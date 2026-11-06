import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { commands, type PlayerStateView, type TrackView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { clock, keyIntent, nextSpeed, type Intent } from "./intents";
import { onPlayerIntent } from "./intentBus";

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

/** `player-state`, four times a second. Inert outside the Tauri shell. */
function usePlayerState(): PlayerStateView | null {
  const [state, setState] = useState<PlayerStateView | null>(null);
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<PlayerStateView>("player-state", (e) => setState(e.payload)).then((u) => {
        if (cancelled) u();
        else unlisten = u;
      }),
    );
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
  return state;
}

async function setFullscreen(on: boolean) {
  if (!inTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().setFullscreen(on);
}

export function PlayerScreen() {
  const playing = useUi((s) => s.playing);
  const stopPlaying = useUi((s) => s.stopPlaying);
  const state = usePlayerState();
  const [error, setError] = useState<string | null>(null);
  const [still, setStill] = useState<string | null>(null);
  const [chrome, setChrome] = useState(true);
  const [menu, setMenu] = useState<"audio" | "sub" | null>(null);
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
    const panels = chrome && !still ? [top.current, bottom.current] : [];
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
  }, [chrome, still]);

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
    const r = await commands.playerPauseStill();
    if (r.status === "ok") setStill(r.data);
    else act({ kind: "pause", paused: true });
  }, [act]);

  const resume = useCallback(async () => {
    setStill(null);
    await commands.playerResumeVideo();
  }, []);

  const leave = useCallback(() => {
    stopPlaying();
  }, [stopPlaying]);

  const run = useCallback(
    (intent: Intent) => {
      wake();
      const s = state;
      switch (intent.kind) {
        case "toggle_pause":
          if (still || s?.paused) void resume();
          else void pause();
          break;
        case "seek":
          if (still) void resume();
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
        case "audio_menu":
          setMenu((m) => (m === "audio" ? null : "audio"));
          break;
        case "back":
          if (menu) setMenu(null);
          else if (fullscreen) {
            setFs(false);
            void setFullscreen(false);
          } else leave();
          break;
      }
    },
    [act, fullscreen, leave, menu, pause, resume, state, still, wake],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const intent = keyIntent(e);
      if (!intent) return;
      e.preventDefault();
      run(intent);
    };
    window.addEventListener("keydown", onKey);
    const off = onPlayerIntent(run);
    return () => {
      window.removeEventListener("keydown", onKey);
      off();
    };
  }, [run]);

  if (!playing) return null;
  const duration = state?.duration ?? 0;
  const position = state?.position ?? 0;
  const tracks = state?.tracks ?? [];
  const showChrome = chrome || !!still || !!menu;

  return (
    <div
      ref={stage}
      onMouseMove={wake}
      className="relative h-full w-full overflow-hidden bg-black text-ink"
      style={{ cursor: showChrome ? "default" : "none" }}
      aria-label={`Playing ${playing.title}`}
    >
      {still && (
        <>
          <img src={still} alt="" className="absolute inset-0 h-full w-full object-contain" />
          <div className="absolute inset-0 bg-black/55" />
          <div className="absolute inset-x-0 top-[34%] flex flex-col items-center gap-5 text-center">
            <p className="label text-ink-muted">Paused · {clock(position)} of {clock(duration)}</p>
            <h1 className="max-w-[70%] font-display text-[44px] leading-[1.05] font-semibold tracking-[-0.03em]">
              {playing.title}
            </h1>
            <button
              type="button"
              onClick={() => void resume()}
              className="mt-2 rounded-sm border border-line-strong px-6 py-2.5 text-[12px] tracking-[0.12em] uppercase hover:bg-raised"
            >
              Resume
            </button>
          </div>
        </>
      )}

      {/* Until the first frame: what is opening, not a black rectangle. The video
          window stays hidden until then (src-tauri player_open). */}
      {!still && !error && !state?.started && (
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

          <div ref={bottom} className="absolute inset-x-3 bottom-3 rounded-md bg-surface px-4 pt-3 pb-2.5">
            <Scrub
              position={position}
              duration={duration}
              buffered={state?.buffered_until ?? null}
              onSeek={(t) => act({ kind: "seek_to", seconds: t })}
            />
            <div className="mt-2 flex items-center gap-3 text-[12px]">
              <button
                type="button"
                onClick={() => run({ kind: "toggle_pause" })}
                aria-label={still || state?.paused ? "Play" : "Pause"}
                className="grid h-8 w-8 place-items-center rounded-sm hover:bg-raised"
              >
                {still || state?.paused ? <PlayGlyph /> : <PauseGlyph />}
              </button>
              <span className="tabular-nums text-ink-muted">
                {clock(position)} / {clock(duration)}
              </span>
              <div className="flex-1" />
              {state?.hwdec && <span className="label text-ink-faint">{state.hwdec}</span>}
              {(state?.speed ?? 1) !== 1 && <span className="tabular-nums">{state?.speed}×</span>}
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
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function Scrub({
  position, duration, buffered, onSeek,
}: { position: number; duration: number; buffered: number | null; onSeek: (t: number) => void }) {
  const pct = (v: number) => (duration > 0 ? `${Math.min(100, (v / duration) * 100)}%` : "0%");
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label="Position"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(position)}
      aria-valuetext={`${clock(position)} of ${clock(duration)}`}
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        onSeek(((e.clientX - r.left) / r.width) * duration);
      }}
      className="relative h-1.5 cursor-pointer rounded-sm bg-line"
    >
      {buffered != null && (
        <div className="absolute inset-y-0 left-0 rounded-sm bg-line-strong" style={{ width: pct(buffered) }} />
      )}
      <div className="absolute inset-y-0 left-0 rounded-sm bg-oxblood-bright" style={{ width: pct(position) }} />
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
