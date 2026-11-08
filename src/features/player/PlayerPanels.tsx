import { forwardRef, useEffect, useRef, useState } from "react";
import { commands, type StatView, type TrackView } from "@/lib/ipc";
import { SHORTCUTS } from "./intents";
import { SubtitleTools } from "./SubtitleTools";

/**
 * Phase 11's controls, each a panel cut out of the video like the player's bars.
 *
 * `PlayerMenu` is the pad's way in (Menu, ADR-0041): every control the keyboard has, as
 * buttons the D-pad moves between. `StatsPanel` is "statistics for nerds". `ShortcutsPanel`
 * is `?`, generated from the same table the keys are tested against.
 */

export interface Picture {
  brightness: number;
  contrast: number;
  saturation: number;
}

interface MenuProps {
  tracks: TrackView[];
  picture: Picture;
  audioDelayMs: number;
  /** Playback speed, 1 normal. */
  speed: number;
  stats: boolean;
  onSpeed: (faster: boolean) => void;
  onAudio: (id: number) => void;
  onSubtitles: (id: number | null) => void;
  onSubtitleDelay: (ms: number) => void;
  onAudioDelay: (ms: number) => void;
  onPicture: (p: Picture) => void;
  onStats: () => void;
  onMini: () => void;
  onPip: () => void;
  onShortcuts: () => void;
  onClose: () => void;
}

const row = "flex w-full items-center justify-between gap-4 rounded-sm px-2 py-1.5 text-left text-[14px] hover:bg-raised";
const small = "h-8 w-8 rounded-sm border border-line-strong text-[15px] text-ink hover:bg-raised";

export const PlayerMenu = forwardRef<HTMLDivElement, MenuProps>(function PlayerMenu(p, ref) {
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => first.current?.focus(), []);
  const audio = p.tracks.filter((t) => t.kind === "audio");
  const subs = p.tracks.filter((t) => t.kind === "sub");
  const step = (key: keyof Picture, by: number) =>
    p.onPicture({ ...p.picture, [key]: Math.max(-100, Math.min(100, p.picture[key] + by)) });
  const label = (t: TrackView) => [t.lang, t.title].filter(Boolean).join(" · ") || `Track ${t.id}`;

  return (
    <div
      ref={ref}
      data-cutout
      role="dialog"
      aria-label="Player menu"
      className="absolute top-[68px] right-3 bottom-[96px] flex w-[min(420px,38%)] flex-col gap-5 overflow-y-auto rounded-md bg-surface px-5 py-5"
    >
      {audio.length > 1 && (
        <section aria-label="Sound">
          <h2 className="label mb-1.5">Sound</h2>
          {audio.map((t, i) => (
            <button key={t.id} ref={i === 0 ? first : undefined} type="button" className={row} onClick={() => p.onAudio(t.id)}>
              <span className="text-ink">{label(t)}</span>
              {t.selected && <span className="label text-ink-faint">Playing</span>}
            </button>
          ))}
        </section>
      )}
      <section aria-label="Subtitles">
        <h2 className="label mb-1.5">Subtitles</h2>
        <button type="button" ref={audio.length > 1 ? undefined : first} className={row} onClick={() => p.onSubtitles(null)}>
          <span className="text-ink">Off</span>
          {!subs.some((t) => t.selected) && <span className="label text-ink-faint">Now</span>}
        </button>
        {subs.map((t) => (
          <button key={t.id} type="button" className={row} onClick={() => p.onSubtitles(t.id)}>
            <span className="text-ink">{label(t)}</span>
            {t.selected && <span className="label text-ink-faint">Now</span>}
          </button>
        ))}
        <div className="mt-1 flex items-center gap-2 px-2">
          <span className="flex-1 text-[13px] text-ink-muted">Timing</span>
          <button type="button" className={small} aria-label="Subtitles earlier" onClick={() => p.onSubtitleDelay(-100)}>−</button>
          <button type="button" className={small} aria-label="Subtitles later" onClick={() => p.onSubtitleDelay(100)}>+</button>
        </div>
        <SubtitleTools />
      </section>
      <section aria-label="Sound timing">
        <div className="flex items-center gap-2 px-2">
          <span className="flex-1 text-[13px] text-ink-muted">
            Sound timing <span className="tabular-nums text-ink">{p.audioDelayMs > 0 ? "+" : ""}{p.audioDelayMs} ms</span>
          </span>
          <button type="button" className={small} aria-label="Sound earlier" onClick={() => p.onAudioDelay(-100)}>−</button>
          <button type="button" className={small} aria-label="Sound later" onClick={() => p.onAudioDelay(100)}>+</button>
        </div>
      </section>
      <section aria-label="Speed">
        <div className="flex items-center gap-2 px-2">
          <span className="flex-1 text-[13px] text-ink-muted">
            Speed <span className="tabular-nums text-ink">{p.speed === 1 ? "Normal" : `${p.speed}×`}</span>
          </span>
          <button type="button" className={small} aria-label="Slower" onClick={() => p.onSpeed(false)}>−</button>
          <button type="button" className={small} aria-label="Faster" onClick={() => p.onSpeed(true)}>+</button>
        </div>
      </section>
      <section aria-label="Picture">
        <h2 className="label mb-1.5">Picture</h2>
        {(["brightness", "contrast", "saturation"] as const).map((key) => (
          <div key={key} className="flex items-center gap-2 px-2 py-0.5">
            <span className="flex-1 text-[13px] capitalize text-ink-muted">
              {key} <span className="tabular-nums text-ink">{p.picture[key]}</span>
            </span>
            <button type="button" className={small} aria-label={`Less ${key}`} onClick={() => step(key, -10)}>−</button>
            <button type="button" className={small} aria-label={`More ${key}`} onClick={() => step(key, 10)}>+</button>
          </div>
        ))}
        <button type="button" className={`${row} mt-1`} onClick={() => p.onPicture({ brightness: 0, contrast: 0, saturation: 0 })}>
          <span className="text-ink-muted">Reset the picture</span>
        </button>
      </section>
      <section aria-label="More" className="flex flex-col">
        <button type="button" className={row} onClick={p.onStats}>
          <span className="text-ink">Statistics</span>
          {p.stats && <span className="label text-ink-faint">On</span>}
        </button>
        <button type="button" className={row} onClick={p.onMini}>
          <span className="text-ink">Keep watching while browsing</span>
        </button>
        <button type="button" className={row} onClick={p.onPip}>
          <span className="text-ink">Picture in picture</span>
        </button>
        <button type="button" className={row} onClick={p.onShortcuts}>
          <span className="text-ink">Shortcuts</span>
        </button>
        <button type="button" className={row} onClick={p.onClose}>
          <span className="text-ink-muted">Close</span>
        </button>
      </section>
    </div>
  );
});

export const StatsPanel = forwardRef<HTMLDivElement, object>(function StatsPanel(_props, ref) {
  const [stats, setStats] = useState<StatView[]>([]);
  useEffect(() => {
    let live = true;
    const tick = () =>
      void commands.playerStats().then((r) => {
        if (live && r.status === "ok") setStats(r.data);
      });
    tick();
    const t = window.setInterval(tick, 1000);
    return () => {
      live = false;
      window.clearInterval(t);
    };
  }, []);
  return (
    <div ref={ref} data-cutout aria-label="Statistics" className="absolute top-[68px] left-3 rounded-md bg-surface px-4 py-3">
      <table className="text-[12px]">
        <tbody>
          {stats.map((s) => (
            <tr key={s.name}>
              <td className="pr-4 text-ink-faint">{s.name}</td>
              <td className="tabular-nums text-ink">{s.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
});

export const ShortcutsPanel = forwardRef<HTMLDivElement, { onClose: () => void }>(function ShortcutsPanel({ onClose }, ref) {
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => close.current?.focus(), []);
  return (
    <div
      ref={ref}
      data-cutout
      role="dialog"
      aria-label="Shortcuts"
      className="absolute top-1/2 left-1/2 max-h-[80%] w-[min(720px,90%)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-md bg-surface px-6 py-5"
    >
      <div className="mb-3 flex items-baseline justify-between">
        <h2 className="font-display text-[20px] font-semibold">Shortcuts</h2>
        <button ref={close} type="button" onClick={onClose} className="label hover:text-ink">
          Close
        </button>
      </div>
      <table className="w-full text-[13px]">
        <thead>
          <tr className="label text-left text-ink-faint">
            <th className="pb-2 font-normal">Keyboard</th>
            <th className="pb-2 font-normal">Controller</th>
            <th className="pb-2 font-normal" />
          </tr>
        </thead>
        <tbody>
          {SHORTCUTS.map((s) => (
            <tr key={s.does} className="border-t border-line-subtle">
              <td className="py-1.5 pr-4 text-ink">{s.keys || "—"}</td>
              <td className="py-1.5 pr-4 text-ink">{s.pad ?? "—"}</td>
              <td className="py-1.5 text-ink-muted">{s.does}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
});
