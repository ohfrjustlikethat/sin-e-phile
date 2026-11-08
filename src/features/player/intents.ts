/**
 * What a viewer can ask the player to do, independent of how they asked.
 *
 * The keyboard (SPEC.md Phase 8) and the Xbox controller (ADR-0041) both map onto these,
 * so the two can never drift into offering different things.
 */
export type Intent =
  | { kind: "toggle_pause" }
  | { kind: "seek"; seconds: number }
  | { kind: "volume"; delta: number }
  | { kind: "mute" }
  | { kind: "fullscreen" }
  | { kind: "frame"; forward: boolean }
  | { kind: "speed"; faster: boolean }
  | { kind: "subtitles_menu" }
  /** Move the subtitles: negative earlier, positive later (SPEC.md Phase 10's nudge). */
  | { kind: "subtitle_delay"; ms: number }
  /** The previous or next episode of a series (SPEC.md Phase 11). */
  | { kind: "episode"; next: boolean }
  /** Phase 11's controls: the pad's player menu, stats, the shortcuts list, picture-in-
   * picture, the in-app mini player (ADR-0043), and the sound's delay. */
  | { kind: "player_menu" }
  | { kind: "stats" }
  | { kind: "shortcuts" }
  | { kind: "pip" }
  | { kind: "mini" }
  | { kind: "audio_delay"; ms: number }
  | { kind: "audio_menu" }
  | { kind: "back" };

/**
 * SPEC.md Phase 8: space, arrows, J/K/L, F, M, `,`/`.` frame step, `[`/`]` speed — plus
 * Escape to leave; Phase 10: G/H move the subtitles earlier and later. Returns `null` for any key the player does not own, so the page's
 * other shortcuts still work.
 */
export function keyIntent(e: Pick<KeyboardEvent, "key" | "shiftKey" | "ctrlKey" | "altKey" | "metaKey">): Intent | null {
  if (e.altKey || e.metaKey) return null;
  if (e.ctrlKey) {
    // Phase 11: the few chords the player owns.
    switch (e.key.toLowerCase()) {
      case "p":
        return { kind: "pip" };
      case "m":
        return { kind: "mini" };
      case "[":
        return { kind: "audio_delay", ms: -100 };
      case "]":
        return { kind: "audio_delay", ms: 100 };
      default:
        return null;
    }
  }
  switch (e.key) {
    case " ":
    case "k":
    case "K":
      return { kind: "toggle_pause" };
    case "j":
    case "J":
      return { kind: "seek", seconds: -10 };
    case "l":
    case "L":
      return { kind: "seek", seconds: 10 };
    case "ArrowLeft":
      return { kind: "seek", seconds: e.shiftKey ? -60 : -5 };
    case "ArrowRight":
      return { kind: "seek", seconds: e.shiftKey ? 60 : 5 };
    case "ArrowUp":
      return { kind: "volume", delta: 5 };
    case "ArrowDown":
      return { kind: "volume", delta: -5 };
    case "m":
    case "M":
      return { kind: "mute" };
    case "f":
    case "F":
      return { kind: "fullscreen" };
    case ",":
      return { kind: "frame", forward: false };
    case ".":
      return { kind: "frame", forward: true };
    case "[":
      return { kind: "speed", faster: false };
    case "]":
      return { kind: "speed", faster: true };
    case "c":
    case "C":
      return { kind: "subtitles_menu" };
    // SPEC.md Phase 10: G and H nudge the subtitles a tenth of a second.
    case "g":
    case "G":
      return { kind: "subtitle_delay", ms: -100 };
    case "h":
    case "H":
      return { kind: "subtitle_delay", ms: 100 };
    // Phase 11: N and P for the next and previous episode.
    case "n":
    case "N":
      return { kind: "episode", next: true };
    case "p":
    case "P":
      return { kind: "episode", next: false };
    case "i":
    case "I":
      return { kind: "stats" };
    case "?":
      return { kind: "shortcuts" };
    case "Escape":
      return { kind: "back" };
    default:
      return null;
  }
}

/** The speeds `[` and `]` step through. */
export const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];

export function nextSpeed(current: number, faster: boolean): number {
  const i = SPEEDS.findIndex((s) => Math.abs(s - current) < 0.01);
  const at = i === -1 ? SPEEDS.indexOf(1) : i;
  const next = SPEEDS[Math.min(Math.max(at + (faster ? 1 : -1), 0), SPEEDS.length - 1)];
  return next ?? 1;
}

/** 0:07, 12:03, 1:02:09. */
export function clock(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "0:00";
  const s = Math.floor(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/**
 * The shortcuts overlay's table (`?`, Phase 11 E4). Each row names the keys and the pad
 * button as a person reads them, and carries one real key event: a test checks that
 * `keyIntent` turns that event into the row's intent, so this list cannot drift from
 * what the keys do.
 */
export interface Shortcut {
  keys: string;
  pad: string | null;
  does: string;
  sample: Pick<KeyboardEvent, "key" | "shiftKey" | "ctrlKey" | "altKey" | "metaKey">;
  intent: Intent;
}

const k = (key: string, mods: Partial<Pick<KeyboardEvent, "shiftKey" | "ctrlKey">> = {}) => ({
  key,
  shiftKey: false,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  ...mods,
});

export const SHORTCUTS: Shortcut[] = [
  { keys: "Space or K", pad: "A", does: "Play or pause", sample: k(" "), intent: { kind: "toggle_pause" } },
  { keys: "← / →", pad: "D-pad ← / →", does: "Back or forward 5 s (10 s on the pad)", sample: k("ArrowLeft"), intent: { kind: "seek", seconds: -5 } },
  { keys: "Shift + ← / →", pad: null, does: "Back or forward a minute", sample: k("ArrowRight", { shiftKey: true }), intent: { kind: "seek", seconds: 60 } },
  { keys: "J / L", pad: "LT / RT", does: "Back or forward 10 s (30 s on the pad)", sample: k("j"), intent: { kind: "seek", seconds: -10 } },
  { keys: "↑ / ↓", pad: "D-pad ↑ / ↓", does: "Volume", sample: k("ArrowUp"), intent: { kind: "volume", delta: 5 } },
  { keys: "M", pad: null, does: "Mute", sample: k("m"), intent: { kind: "mute" } },
  { keys: "F", pad: "View", does: "Full screen", sample: k("f"), intent: { kind: "fullscreen" } },
  { keys: ", / .", pad: null, does: "One frame back or forward, paused", sample: k(","), intent: { kind: "frame", forward: false } },
  { keys: "[ / ]", pad: null, does: "Slower or faster", sample: k("]"), intent: { kind: "speed", faster: true } },
  { keys: "C", pad: "X", does: "Subtitles", sample: k("c"), intent: { kind: "subtitles_menu" } },
  { keys: "G / H", pad: "LB / RB", does: "Subtitles earlier or later, remembered", sample: k("g"), intent: { kind: "subtitle_delay", ms: -100 } },
  { keys: "Ctrl + [ / ]", pad: null, does: "Sound earlier or later", sample: k("[", { ctrlKey: true }), intent: { kind: "audio_delay", ms: -100 } },
  { keys: "N / P", pad: "Click right / left stick", does: "Next or previous episode", sample: k("n"), intent: { kind: "episode", next: true } },
  { keys: "I", pad: null, does: "Statistics", sample: k("i"), intent: { kind: "stats" } },
  { keys: "Ctrl + M", pad: null, does: "Keep watching in a corner while you browse", sample: k("m", { ctrlKey: true }), intent: { kind: "mini" } },
  { keys: "Ctrl + P", pad: null, does: "Picture in picture, over other windows", sample: k("p", { ctrlKey: true }), intent: { kind: "pip" } },
  { keys: "?", pad: null, does: "This list", sample: k("?"), intent: { kind: "shortcuts" } },
  { keys: "Esc", pad: "B", does: "Back", sample: k("Escape"), intent: { kind: "back" } },
  { keys: "", pad: "Menu", does: "The player menu: everything above, by pad", sample: k(""), intent: { kind: "player_menu" } },
];

/** What the player last reported, and when (ms on the page's clock). */
export interface Reported {
  position: number;
  at: number;
  speed: number;
  playing: boolean;
  duration: number | null;
}

/** The longest the bar runs ahead of a report: a stall then shows as a stall. */
export const GLIDE_MAX_S = 0.5;

/**
 * Where playback is now, between reports (Phase 16): the backend reports four times a
 * second, and a bar drawn from the reports alone moves in quarter-second jumps. Playing,
 * the position runs on at the playback speed from the last report, at most `GLIDE_MAX_S`
 * past it and never past the end; paused, it is the report.
 */
export function glide(r: Reported, now: number): number {
  if (!r.playing) return r.position;
  const ahead = Math.min(GLIDE_MAX_S, Math.max(0, (now - r.at) / 1000) * r.speed);
  const p = r.position + ahead;
  return r.duration != null && r.duration > 0 ? Math.min(p, r.duration) : p;
}
