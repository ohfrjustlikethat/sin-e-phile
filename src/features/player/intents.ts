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
  | { kind: "audio_menu" }
  | { kind: "back" };

/**
 * SPEC.md Phase 8: space, arrows, J/K/L, F, M, `,`/`.` frame step, `[`/`]` speed — plus
 * Escape to leave. Returns `null` for any key the player does not own, so the page's
 * other shortcuts still work.
 */
export function keyIntent(e: Pick<KeyboardEvent, "key" | "shiftKey" | "ctrlKey" | "altKey" | "metaKey">): Intent | null {
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
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
