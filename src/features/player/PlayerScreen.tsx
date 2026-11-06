import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type ChapterView, type PlayerStateView, type TrackView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { clock, keyIntent, nextSpeed, type Intent } from "./intents";
import { onPlayerIntent } from "./intentBus";
import { PauseOverlay } from "./PauseOverlay";
import { PlayerMenu, ShortcutsPanel, StatsPanel, type Picture } from "./PlayerPanels";
import { EndCard, NextEpisodeCard } from "./WhatNext";
import { moment } from "./endCard";
import { previewLeft, tileAt } from "./scrubPreview";
import { usePlaybackChoice } from "@/lib/playbackChoice";

/**
 * The player — SPEC.md Phase 8, ADR-0020 (solid chrome through cutouts) and ADR-0021
 * (a still frame under the pause overlay).
 *
 * The video is a native window above this page, so nothing here can be drawn OVER it.
 * Instead each chrome panel reports its outline, and the native window cuts that shape
 * out of itself: the page shows through exactly where a panel is. Panels are therefore
 * solid, with a hard edge and a radius the cutout matches (ADR-0020). When paused, the
 * video window is hidden and the page draws the frame as an ordinary image, so the
 * pause overlay can dim it like any HTML.
 */

const HIDE_AFTER_MS = 2500;

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** A backend event, for as long as the component is mounted. Inert outside the Tauri shell. */
function useTauriEvent<T>(name: string, on: (payload: T) => void) {
  const handler = useRef(on);
  handler.current = on;
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen<T>(name, (e) => handler.current(e.payload)).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [name]);
}

/** `player-state`, four times a second. */
function usePlayerState(): PlayerStateView | null {
  const [state, setState] = useState<PlayerStateView | null>(null);
  useTauriEvent<PlayerStateView>("player-state", setState);
  return state;
}

async function setFullscreen(on: boolean) {
  if (!inTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().setFullscreen(on);
}

export function PlayerScreen({ mini = false }: { mini?: boolean } = {}) {
  const playing = useUi((s) => s.playing);
  const stopPlaying = useUi((s) => s.stopPlaying);
  const openDetail = useUi((s) => s.openDetail);
  const state = usePlayerState();
  // The pause overlay's data, fetched as playback starts so pausing only paints.
  const details = useQuery({
    queryKey: ["pause-details", playing?.mediaItemId],
    queryFn: async () => {
      const r = await commands.pauseDetails(playing?.mediaItemId ?? 0);
      if (r.status === "error") throw new Error(r.error);
      return r.data;
    },
    enabled: playing?.mediaItemId != null,
    staleTime: Infinity,
  });
  // Chapters, once the file is open: their titles for the overlay, their marks for the bar.
  const [chapters, setChapters] = useState<ChapterView[]>([]);
  useEffect(() => {
    if (!state?.started) return;
    void commands.playerChapters().then((r) => setChapters(r.status === "ok" ? r.data : []));
  }, [state?.started]);
  const chapterTitle = state?.chapter != null ? (chapters[state.chapter]?.title ?? null) : null;

  // The episodes either side (SPEC.md Phase 11): N/P and the stick clicks, and the card.
  const play = useUi((s) => s.play);
  const neighbours = useQuery({
    queryKey: ["neighbours", playing?.mediaItemId],
    queryFn: async () => {
      const r = await commands.episodeNeighbours(playing?.mediaItemId ?? 0);
      return r.status === "ok" ? r.data : { previous: null, next: null };
    },
    enabled: playing?.mediaItemId != null,
    staleTime: Infinity,
  });
  const { autoplay, audio, quality } = usePlaybackChoice();
  const [dismissed, setDismissed] = useState(false);
  const [finished, setFinished] = useState(false);
  useEffect(() => {
    setDismissed(false);
    setFinished(false);
  }, [playing]);
  const next = neighbours.data?.next ?? null;
  const now = moment({
    hasNext: !!next,
    position: state?.position ?? null,
    duration: state?.duration ?? null,
    ended: !!state?.ended,
    chapterTitle,
  });
  const showNextCard = now === "next-episode" && !dismissed && !!next;
  useEffect(() => {
    // A film's credits have played: stop the video, and the end card takes the screen.
    if (now === "end-card" && !finished) {
      setFinished(true);
      void commands.playerClose();
    }
  }, [now, finished]);
  const goTo = useCallback(
    async (id: number) => {
      const r = await commands.preparePlay(id, audio, quality);
      if (r.status === "ok") play({ target: r.data.open, title: r.data.title, mediaItemId: id, start: r.data.start });
      else setError(r.error);
    },
    [audio, quality, play],
  );
  const card = useRef<HTMLDivElement>(null);

  // Scrub thumbnails (Phase 11): made in the background for a file on this machine; asked
  // for until they exist. A stream has none, and the preview shows the time alone.
  const [hover, setHover] = useState<{ time: number; x: number; barLeft: number; barWidth: number } | null>(null);
  const sheets = useQuery({
    queryKey: ["scrub-sheets", playing?.target],
    queryFn: async () => {
      const r = await commands.scrubThumbs();
      return r.status === "ok" ? r.data : null;
    },
    enabled: !!state?.started,
    refetchInterval: (q) => (q.state.data ? false : 10_000),
  });
  const [sheetImages, setSheetImages] = useState<Record<number, string>>({});
  const asked = useRef(new Set<number>());
  useEffect(() => {
    setSheetImages({});
    asked.current = new Set();
  }, [playing]);
  const tile = hover && sheets.data ? tileAt(hover.time, sheets.data) : null;
  const wanted = tile?.sheet;
  useEffect(() => {
    if (wanted === undefined || asked.current.has(wanted)) return;
    asked.current.add(wanted);
    void commands.scrubSheet(wanted).then((r) => {
      if (r.status === "ok") setSheetImages((m) => ({ ...m, [wanted]: r.data }));
    });
  }, [wanted]);
  const preview = useRef<HTMLDivElement>(null);

  // Phase 11's controls: the pad's menu, the shortcuts list, statistics, picture,
  // the sound's timing, picture-in-picture and the mini player.
  const setMini = useUi((s) => s.setMini);
  const [panel, setPanel] = useState<"menu" | "shortcuts" | null>(null);
  const [stats, setStats] = useState(false);
  const [picture, setPicture] = useState<Picture>({ brightness: 0, contrast: 0, saturation: 0 });
  const [audioDelayMs, setAudioDelayMs] = useState(0);
  const menuPanel = useRef<HTMLDivElement>(null);
  const statsPanel = useRef<HTMLDivElement>(null);
  const shortcutsPanel = useRef<HTMLDivElement>(null);
  const pipFrom = useRef<{ x: number; y: number; w: number; h: number } | null>(null);
  const [pip, setPip] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Every copy was tried and none would start (SPEC.md Phase 9: fallback is silent
  // until there is nothing left to fall back to).
  useTauriEvent<string>("player-failed", setError);
  const [still, setStill] = useState<string | null>(null);
  // Paused, as the page shows it: true the moment pause is pressed, before the still
  // frame arrives (Phase 11: the overlay in < 200 ms).
  const [paused, setPaused] = useState(false);
  const [chrome, setChrome] = useState(true);
  const [menu, setMenu] = useState<"audio" | "sub" | null>(null);
  // The last nudge, shown for a moment in the controls (G/H, LB/RB).
  const [nudge, setNudge] = useState<string | null>(null);
  useEffect(() => {
    if (!nudge) return;
    const t = window.setTimeout(() => setNudge(null), 2500);
    return () => window.clearTimeout(t);
  }, [nudge]);
  const [fullscreen, setFs] = useState(false);

  const stage = useRef<HTMLDivElement>(null);
  const top = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const hideTimer = useRef<number | undefined>(undefined);

  // The latest position, for saving on the way out (Phase 8 E4).
  const last = useRef<{ position: number; duration: number | null }>({ position: 0, duration: null });
  useEffect(() => {
    if (state?.position != null) last.current = { position: state.position, duration: state.duration };
  }, [state]);

  // Open on mount; save the position and close on unmount.
  useEffect(() => {
    if (!playing) return;
    void commands.playerOpen(playing.target, playing.start).then((r) => {
      if (r.status === "error") setError(r.error);
    });
    const save = () => {
      if (playing.mediaItemId !== null && last.current.position > 0) {
        void commands.savePosition(playing.mediaItemId, last.current.position, last.current.duration);
      }
    };
    // Also every 15 s, so a crash or a closed window loses little.
    const timer = window.setInterval(save, 15_000);
    return () => {
      window.clearInterval(timer);
      save();
      void commands.playerClose();
      void setFullscreen(false);
    };
  }, [playing]);

  // Tell the native window where to sit and which shapes to cut out (ADR-0020).
  const layout = useCallback(() => {
    const s = stage.current?.getBoundingClientRect();
    if (!s) return;
    const panels = [
      ...(chrome && !paused ? [top.current, bottom.current] : []),
      ...(showNextCard ? [card.current] : []),
      ...(hover && chrome ? [preview.current] : []),
      ...(panel === "menu" ? [menuPanel.current] : []),
      ...(panel === "shortcuts" ? [shortcutsPanel.current] : []),
      ...(stats ? [statsPanel.current] : []),
    ];
    const cutouts = panels
      .filter((el): el is HTMLDivElement => !!el)
      .map((el) => {
        const r = el.getBoundingClientRect();
        return {
          x: r.x,
          y: r.y,
          width: r.width,
          height: r.height,
          radius: Number.parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0,
        };
      });
    void commands.playerLayout(s.x, s.y, s.width, s.height, window.devicePixelRatio, cutouts);
  }, [chrome, paused, showNextCard, hover, panel, stats, mini]);

  useLayoutEffect(() => {
    layout();
    const ro = new ResizeObserver(layout);
    if (stage.current) ro.observe(stage.current);
    window.addEventListener("resize", layout);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", layout);
    };
  }, [layout, menu]);

  // Chrome auto-hide while playing.
  const wake = useCallback(() => {
    setChrome(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!menu) setChrome(false);
    }, HIDE_AFTER_MS);
  }, [menu]);
  useEffect(() => {
    wake();
    return () => window.clearTimeout(hideTimer.current);
  }, [wake]);

  const act = useCallback((a: Parameters<typeof commands.playerAction>[0]) => {
    void commands.playerAction(a);
  }, []);

  const pause = useCallback(async () => {
    (window as { __pausedAt?: number }).__pausedAt = performance.now();
    setPaused(true);
    const r = await commands.playerPauseStill();
    if (r.status === "ok") setStill(r.data);
    else act({ kind: "pause", paused: true });
  }, [act]);

  const resume = useCallback(async () => {
    setStill(null);
    setPaused(false);
    await commands.playerResumeVideo();
  }, []);

  const leave = useCallback(() => {
    stopPlaying();
  }, [stopPlaying]);

  /** Picture-in-picture (Ctrl+P): the window small, on top, in the corner; again restores. */
  const togglePip = useCallback(async () => {
    if (!inTauri()) return;
    const { getCurrentWindow, currentMonitor, PhysicalPosition, PhysicalSize, LogicalSize } = await import(
      "@tauri-apps/api/window"
    );
    const w = getCurrentWindow();
    const from = pipFrom.current;
    if (from) {
      pipFrom.current = null;
      setPip(false);
      await w.setAlwaysOnTop(false);
      await w.setMinSize(new LogicalSize(1024, 640));
      await w.setSize(new PhysicalSize(from.w, from.h));
      await w.setPosition(new PhysicalPosition(from.x, from.y));
      return;
    }
    if (fullscreen) {
      setFs(false);
      await setFullscreen(false);
    }
    const pos = await w.outerPosition();
    const size = await w.outerSize();
    pipFrom.current = { x: pos.x, y: pos.y, w: size.width, h: size.height };
    setPip(true);
    const m = await currentMonitor();
    const sf = m?.scaleFactor ?? 1;
    const width = Math.round(480 * sf);
    const height = Math.round(270 * sf);
    await w.setMinSize(null);
    await w.setAlwaysOnTop(true);
    await w.setSize(new PhysicalSize(width, height));
    if (m) {
      await w.setPosition(
        new PhysicalPosition(
          m.position.x + m.size.width - width - Math.round(24 * sf),
          m.position.y + m.size.height - height - Math.round(72 * sf),
        ),
      );
    }
  }, [fullscreen]);

  /** Returns false to decline an intent: the mini player declines everything so the
   * app can be browsed; with a panel open, the D-pad and A move focus within it. */
  const run = useCallback(
    (intent: Intent): boolean => {
      if (mini) return false;
      if (panel) {
        if (intent.kind === "back" || intent.kind === "player_menu" || intent.kind === "shortcuts") {
          setPanel(null);
          return true;
        }
        if (intent.kind === "seek" || intent.kind === "volume" || intent.kind === "toggle_pause") return false;
      }
      wake();
      const s = state;
      switch (intent.kind) {
        case "player_menu":
          setPanel("menu");
          break;
        case "shortcuts":
          setPanel("shortcuts");
          break;
        case "stats":
          setStats((v) => !v);
          break;
        case "pip":
          void togglePip();
          break;
        case "mini":
          if (paused) void resume();
          setMini(true);
          break;
        case "audio_delay": {
          const next = audioDelayMs + intent.ms;
          setAudioDelayMs(next);
          act({ kind: "audio_delay", seconds: next / 1000 });
          break;
        }
        case "toggle_pause":
          // With the next-episode card up, A plays it.
          if (showNextCard && next) {
            void goTo(next.id);
            break;
          }
          if (paused || s?.paused) void resume();
          else void pause();
          break;
        case "seek":
          if (paused) void resume();
          act({ kind: "seek_by", seconds: intent.seconds });
          break;
        case "volume":
          act({ kind: "volume", percent: Math.min(130, Math.max(0, (s?.volume ?? 100) + intent.delta)) });
          break;
        case "mute":
          act({ kind: "mute", muted: !(s?.muted ?? false) });
          break;
        case "fullscreen":
          setFs((f) => {
            void setFullscreen(!f);
            return !f;
          });
          break;
        case "frame":
          act({ kind: "frame_step", forward: intent.forward });
          break;
        case "speed":
          act({ kind: "speed", speed: nextSpeed(s?.speed ?? 1, intent.faster) });
          break;
        case "subtitles_menu":
          setMenu((m) => (m === "sub" ? null : "sub"));
          break;
        case "episode": {
          const target = intent.next ? neighbours.data?.next : neighbours.data?.previous;
          if (target) void goTo(target.id);
          break;
        }
        case "subtitle_delay":
          void commands.subtitleNudge(intent.ms).then((r) => {
            const ms = r.status === "ok" ? r.data : null;
            setNudge(
              ms === null
                ? "No subtitles showing"
                : ms === 0
                  ? "Subtitles as timed"
                  : `Subtitles ${ms > 0 ? "+" : "−"}${(Math.abs(ms) / 1000).toFixed(1)} s`,
            );
          });
          break;
        case "audio_menu":
          setMenu((m) => (m === "audio" ? null : "audio"));
          break;
        case "back":
          if (showNextCard) setDismissed(true);
          else if (menu) setMenu(null);
          else if (fullscreen) {
            setFs(false);
            void setFullscreen(false);
          } else leave();
          break;
      }
      return true;
    },
    [
      act, audioDelayMs, fullscreen, goTo, leave, menu, mini, neighbours.data, next, panel, pause, paused,
      resume, setMini, showNextCard, state, togglePip, wake,
    ],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (mini) return; // the app is being browsed; its keys are its own
      const intent = keyIntent(e);
      if (!intent) return;
      if (run(intent)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    const off = onPlayerIntent(run);
    return () => {
      window.removeEventListener("keydown", onKey);
      off();
    };
  }, [run, mini]);

  if (!playing) return null;
  const duration = state?.duration ?? 0;
  const position = state?.position ?? 0;
  const tracks = state?.tracks ?? [];
  const showChrome = !mini && (chrome || paused || !!menu || !!panel);

  if (mini) {
    // The mini player (ADR-0043): the video in a corner, a slim bar under it.
    return (
      <div
        className="flex h-full w-full flex-col overflow-hidden rounded-md bg-black text-ink"
        aria-label={`Playing ${playing.title}`}
      >
        <div ref={stage} className="relative min-h-0 flex-1" />
        <div className="flex items-center gap-2 bg-surface px-3 py-2 text-[12px]">
          <span className="flex-1 truncate text-ink">{playing.title}</span>
          <button
            type="button"
            className="px-2 py-1 text-ink-muted hover:text-ink"
            onClick={() => act({ kind: "pause", paused: !state?.paused })}
          >
            {state?.paused ? "Play" : "Pause"}
          </button>
          <button type="button" className="px-2 py-1 text-ink-muted hover:text-ink" onClick={() => setMini(false)}>
            Expand
          </button>
          <button type="button" className="px-2 py-1 text-ink-muted hover:text-ink" onClick={leave} aria-label="Stop">
            ✕
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={stage}
      onMouseMove={wake}
      className="relative h-full w-full overflow-hidden bg-black text-ink"
      style={{ cursor: showChrome ? "default" : "none" }}
      aria-label={`Playing ${playing.title}`}
    >
      {paused && (
        <PauseOverlay
          title={playing.title}
          still={still}
          position={position}
          duration={duration}
          chapter={chapterTitle}
          details={details.data}
          onResume={() => void resume()}
          onOpen={(id) => {
            stopPlaying();
            openDetail(id);
          }}
        />
      )}

      {panel === "menu" && (
        <PlayerMenu
          ref={menuPanel}
          tracks={tracks}
          picture={picture}
          audioDelayMs={audioDelayMs}
          stats={stats}
          onAudio={(id) => act({ kind: "audio", id })}
          onSubtitles={(id) => act({ kind: "subtitles", id })}
          onSubtitleDelay={(ms) => run({ kind: "subtitle_delay", ms })}
          onAudioDelay={(ms) => run({ kind: "audio_delay", ms })}
          onPicture={(p) => {
            setPicture(p);
            act({ kind: "picture", ...p });
          }}
          onStats={() => setStats((v) => !v)}
          onMini={() => {
            setPanel(null);
            setMini(true);
          }}
          onPip={() => {
            setPanel(null);
            void togglePip();
          }}
          onShortcuts={() => setPanel("shortcuts")}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === "shortcuts" && <ShortcutsPanel ref={shortcutsPanel} onClose={() => setPanel(null)} />}
      {stats && <StatsPanel ref={statsPanel} />}

      {hover && showChrome && (() => {
        const w = sheets.data?.tile_width ?? 0;
        const h = sheets.data?.tile_height ?? 0;
        const img = tile ? sheetImages[tile.sheet] : undefined;
        const width = img ? w : 72;
        return (
          <div
            ref={preview}
            aria-hidden
            className="absolute bottom-[96px] overflow-hidden rounded-sm bg-surface"
            style={{ left: previewLeft(hover.x, width + 8, hover.barLeft, hover.barWidth) }}
          >
            {img && tile && (
              <div
                className="m-1"
                style={{
                  width: w,
                  height: h,
                  backgroundImage: `url(${img})`,
                  backgroundPosition: `-${tile.col * w}px -${tile.row * h}px`,
                }}
              />
            )}
            <p className="px-2 pb-1 text-center text-[12px] tabular-nums text-ink">{clock(hover.time)}</p>
          </div>
        );
      })()}

      {showNextCard && next && (
        <NextEpisodeCard
          ref={card}
          next={next}
          autoplay={autoplay}
          onPlay={() => void goTo(next.id)}
          onDismiss={() => setDismissed(true)}
        />
      )}
      {finished && playing.mediaItemId !== null && (
        <EndCard
          mediaItemId={playing.mediaItemId}
          title={playing.title}
          moreLikeThis={details.data?.more_like_this ?? []}
          onOpen={(id) => {
            stopPlaying();
            openDetail(id);
          }}
          onLeave={() => {
            const id = playing.mediaItemId;
            stopPlaying();
            if (id !== null) openDetail(id);
          }}
        />
      )}

      {/* Until the first frame: what is opening, not a black rectangle. The video
          window stays hidden until then (src-tauri player_open). */}
      {!paused && !error && !state?.started && (
        <div className="absolute inset-x-0 top-[36%] flex flex-col items-center gap-4 text-center">
          <p className="label text-ink-muted">
            {playing.start ? `Resuming from ${clock(playing.start)}` : "Opening"}
          </p>
          <h1 className="max-w-[70%] font-display text-[44px] leading-[1.05] font-semibold tracking-[-0.03em]">
            {playing.title}
          </h1>
        </div>
      )}

      {error && (
        <div className="absolute inset-0 grid place-items-center">
          <div className="max-w-md text-center">
            <p className="font-display text-[26px]">This could not be played</p>
            <p className="mt-2 text-sm text-ink-muted">{error}</p>
            <button type="button" onClick={leave} className="mt-5 text-sm underline">
              Back
            </button>
          </div>
        </div>
      )}

      {showChrome && (
        <>
          {!pip && (
          <div
            ref={top}
            data-tauri-drag-region
            className="absolute inset-x-3 top-3 flex h-12 items-center gap-4 rounded-md bg-surface px-3"
          >
            <button type="button" onClick={leave} aria-label="Back" className="grid h-8 w-8 place-items-center text-ink-muted hover:text-ink">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
                <path d="M10 3L5 8l5 5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </button>
            <span data-tauri-drag-region className="truncate font-display text-[17px] font-semibold tracking-[-0.02em]">
              {playing.title}
            </span>
          </div>
          )}

          <div ref={bottom} className="absolute inset-x-3 bottom-3 rounded-md bg-surface px-4 pt-3 pb-2.5">
            <Scrub
              position={position}
              duration={duration}
              buffered={state?.buffered_until ?? null}
              chapters={chapters}
              onSeek={(t) => act({ kind: "seek_to", seconds: t })}
              onHover={setHover}
            />
            <div className="mt-2 flex items-center gap-3 text-[12px]">
              <button
                type="button"
                onClick={() => run({ kind: "toggle_pause" })}
                aria-label={paused || state?.paused ? "Play" : "Pause"}
                className="grid h-8 w-8 place-items-center rounded-sm hover:bg-raised"
              >
                {paused || state?.paused ? <PlayGlyph /> : <PauseGlyph />}
              </button>
              <span className="tabular-nums text-ink-muted">
                {clock(position)} / {clock(duration)}
              </span>
              <div className="flex-1" />
              {pip ? (
                // Picture-in-picture is 480 px wide: play, the time, and the way back.
                <button type="button" onClick={() => void togglePip()} className="px-2 py-1 text-ink-muted hover:text-ink">
                  Back to the window
                </button>
              ) : (
              <>
              {state?.hwdec && <span className="label text-ink-faint">{state.hwdec}</span>}
              {(state?.speed ?? 1) !== 1 && <span className="tabular-nums">{state?.speed}×</span>}
              {nudge && (
                <span role="status" className="tabular-nums text-ink">
                  {nudge}
                </span>
              )}
              <TrackMenu label="Audio" kind="audio" open={menu === "audio"} tracks={tracks}
                onToggle={() => run({ kind: "audio_menu" })}
                onPick={(id) => { act({ kind: "audio", id: id ?? 1 }); setMenu(null); }} />
              <TrackMenu label="Subtitles" kind="sub" open={menu === "sub"} tracks={tracks}
                onToggle={() => run({ kind: "subtitles_menu" })}
                onPick={(id) => { act({ kind: "subtitles", id }); setMenu(null); }} />
              <button type="button" onClick={() => run({ kind: "mute" })} className="px-2 py-1 text-ink-muted hover:text-ink">
                {state?.muted ? "Unmute" : `Volume ${Math.round(state?.volume ?? 100)}`}
              </button>
              <button type="button" onClick={() => run({ kind: "fullscreen" })} className="px-2 py-1 text-ink-muted hover:text-ink">
                {fullscreen ? "Exit full screen" : "Full screen"}
              </button>
              </>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function Scrub({
  position, duration, buffered, chapters, onSeek, onHover,
}: {
  position: number;
  duration: number;
  buffered: number | null;
  chapters: ChapterView[];
  onSeek: (t: number) => void;
  /** The time under the pointer, and where the pointer is; null when it leaves. */
  onHover: (h: { time: number; x: number; barLeft: number; barWidth: number } | null) => void;
}) {
  const pct = (v: number) => (duration > 0 ? `${Math.min(100, (v / duration) * 100)}%` : "0%");
  const at = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    return { time: ((e.clientX - r.left) / r.width) * duration, x: e.clientX, barLeft: r.left, barWidth: r.width };
  };
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label="Position"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(position)}
      aria-valuetext={`${clock(position)} of ${clock(duration)}`}
      onClick={(e) => onSeek(at(e).time)}
      onMouseMove={(e) => onHover(at(e))}
      onMouseLeave={() => onHover(null)}
      className="relative h-1.5 cursor-pointer rounded-sm bg-line"
    >
      {buffered != null && (
        <div className="absolute inset-y-0 left-0 rounded-sm bg-line-strong" style={{ width: pct(buffered) }} />
      )}
      <div className="absolute inset-y-0 left-0 rounded-sm bg-oxblood-bright" style={{ width: pct(position) }} />
      {/* Chapter marks (Phase 11): a gap in the bar where each chapter begins. */}
      {chapters
        .filter((c) => (c.time ?? 0) > 0)
        .map((c) => (
          <div
            key={c.time}
            data-chapter={c.title ?? ""}
            className="absolute inset-y-0 w-[3px] -translate-x-1/2 bg-surface"
            style={{ left: pct(c.time ?? 0) }}
          />
        ))}
    </div>
  );
}

function TrackMenu({
  label, kind, open, tracks, onToggle, onPick,
}: {
  label: string;
  kind: "audio" | "sub";
  open: boolean;
  tracks: TrackView[];
  onToggle: () => void;
  onPick: (id: number | null) => void;
}) {
  const list = tracks.filter((t) => t.kind === kind);
  if (kind === "audio" && list.length < 2) return null;
  return (
    <div className="relative">
      <button type="button" onClick={onToggle} aria-expanded={open} className="px-2 py-1 text-ink-muted hover:text-ink">
        {label}
      </button>
      {open && (
        <ul role="menu" className="absolute right-0 bottom-9 min-w-44 rounded-md border border-line bg-raised py-1">
          {kind === "sub" && (
            <li>
              <button type="button" role="menuitem" onClick={() => onPick(null)} className="w-full px-3 py-1.5 text-left hover:bg-surface">
                Off
              </button>
            </li>
          )}
          {list.map((t) => (
            <li key={t.id}>
              <button type="button" role="menuitem" onClick={() => onPick(t.id)}
                className={`w-full px-3 py-1.5 text-left hover:bg-surface ${t.selected ? "text-ink" : "text-ink-muted"}`}>
                {t.title ?? t.lang ?? `Track ${t.id}`}
                {t.lang && t.title ? <span className="text-ink-faint"> · {t.lang}</span> : null}
              </button>
            </li>
          ))}
          {list.length === 0 && <li className="px-3 py-1.5 text-ink-faint">None in this file</li>}
        </ul>
      )}
    </div>
  );
}

function PlayGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden>
      <path d="M3 1.5v11l9-5.5z" fill="currentColor" />
    </svg>
  );
}

function PauseGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden>
      <path d="M3 1.5h3v11H3zM8 1.5h3v11H8z" fill="currentColor" />
    </svg>
  );
}
