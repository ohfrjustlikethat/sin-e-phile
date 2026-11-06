/**
 * The Xbox controller as a remote — ADR-0041, SPEC.md §9.4.
 *
 * Windows presents an Xbox controller (Bluetooth, the wireless adapter or USB) to the
 * WebView through the standard Gamepad API, with the W3C "standard" button layout. This
 * module turns presses into one of two things:
 *
 * - **Player intents**, when the player is on screen — the same intents the keyboard
 *   sends (`features/player/intents.ts`), so the two can never offer different things.
 * - **Focus navigation** everywhere else: the D-pad and left stick move focus to the
 *   nearest focusable thing in that direction, A activates it, B goes back.
 *
 * Everything that decides something is a pure function, tested in `gamepad.test.ts`;
 * `startGamepad` is the thin loop that reads the hardware.
 */
import type { Intent } from "@/features/player/intents";

/** W3C standard gamepad layout. */
export const BUTTON = {
  A: 0, B: 1, X: 2, Y: 3, LB: 4, RB: 5, LT: 6, RT: 7, VIEW: 8, MENU: 9,
  UP: 12, DOWN: 13, LEFT: 14, RIGHT: 15,
} as const;

export type Direction = "up" | "down" | "left" | "right";

export type NavAction =
  | { kind: "move"; direction: Direction }
  | { kind: "activate" }
  | { kind: "back" }
  | { kind: "tab"; next: boolean }
  | { kind: "search" }
  | { kind: "settings" };

/** One frame of a pad: which buttons are down, and the left stick. */
export interface PadFrame {
  pressed: boolean[];
  stickX: number;
  stickY: number;
}

const DEADZONE = 0.5;

/** Where a resting thumb starts to tilt the focused tile (ADR-0042); below it is noise. */
export const TILT_FROM = 0.12;
/** The most a tile tilts, in degrees. */
export const TILT_MAX = 6;

/**
 * The tilt for a thumb resting on the left stick: inside the band between noise and the
 * navigation deadzone, up to 6° toward the thumb; outside it, flat. Past the deadzone the
 * stick moves focus instead, so the two never fight.
 */
export function tiltFor(x: number, y: number): { rx: number; ry: number } {
  const m = Math.hypot(x, y);
  if (m <= TILT_FROM || m >= DEADZONE) return { rx: 0, ry: 0 };
  const deg = (TILT_MAX * (m - TILT_FROM)) / (DEADZONE - TILT_FROM);
  // Stick up (y < 0) tips the top away; stick right turns the right edge away.
  // `|| 0` turns a -0 (from -y when y is 0) into a plain 0.
  return { rx: (-y / m) * deg || 0, ry: (x / m) * deg || 0 };
}

/** The four directions a frame is holding, from the D-pad or the left stick. */
export function heldDirections(f: PadFrame): Set<Direction> {
  const d = new Set<Direction>();
  if (f.pressed[BUTTON.UP] || f.stickY < -DEADZONE) d.add("up");
  if (f.pressed[BUTTON.DOWN] || f.stickY > DEADZONE) d.add("down");
  if (f.pressed[BUTTON.LEFT] || f.stickX < -DEADZONE) d.add("left");
  if (f.pressed[BUTTON.RIGHT] || f.stickX > DEADZONE) d.add("right");
  return d;
}

/** Buttons that went down this frame. A held button fires once, not every frame. */
export function newlyPressed(prev: PadFrame | null, now: PadFrame): number[] {
  return now.pressed.flatMap((down, i) => (down && !prev?.pressed[i] ? [i] : []));
}

/** The player's mapping (ADR-0041's table). */
export function playerIntent(button: number): Intent | null {
  switch (button) {
    case BUTTON.A:
    case BUTTON.MENU:
      return { kind: "toggle_pause" };
    case BUTTON.B:
      return { kind: "back" };
    case BUTTON.X:
      return { kind: "subtitles_menu" };
    case BUTTON.Y:
      return { kind: "audio_menu" };
    case BUTTON.LT:
      return { kind: "seek", seconds: -30 };
    case BUTTON.RT:
      return { kind: "seek", seconds: 30 };
    case BUTTON.LEFT:
      return { kind: "seek", seconds: -10 };
    case BUTTON.RIGHT:
      return { kind: "seek", seconds: 10 };
    case BUTTON.UP:
      return { kind: "volume", delta: 5 };
    case BUTTON.DOWN:
      return { kind: "volume", delta: -5 };
    default:
      return null;
  }
}

/** Browsing's mapping. Directions are handled separately, with repeat. */
export function navAction(button: number): NavAction | null {
  switch (button) {
    case BUTTON.A:
      return { kind: "activate" };
    case BUTTON.B:
      return { kind: "back" };
    case BUTTON.LB:
      return { kind: "tab", next: false };
    case BUTTON.RB:
      return { kind: "tab", next: true };
    case BUTTON.Y:
      return { kind: "search" };
    case BUTTON.MENU:
      return { kind: "settings" };
    default:
      return null;
  }
}

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Spatial navigation: the candidate nearest to `from` in `direction`, or -1.
 *
 * A candidate counts only if it lies in that direction (its near edge beyond the
 * current one's centre). Among those, distance along the direction counts fully and
 * sideways drift counts double, so "down" prefers the thing directly below over a
 * slightly nearer thing off to the side — what a remote user expects.
 */
export function nearest(from: Box, candidates: Box[], direction: Direction): number {
  const cx = from.x + from.width / 2;
  const cy = from.y + from.height / 2;
  let best = -1;
  let bestScore = Infinity;
  candidates.forEach((c, i) => {
    const ox = c.x + c.width / 2;
    const oy = c.y + c.height / 2;
    let along: number;
    let across: number;
    switch (direction) {
      case "down":
        if (c.y + c.height / 2 <= cy + 1) return;
        along = c.y - (from.y + from.height);
        across = Math.abs(ox - cx);
        break;
      case "up":
        if (c.y + c.height / 2 >= cy - 1) return;
        along = from.y - (c.y + c.height);
        across = Math.abs(ox - cx);
        break;
      case "right":
        if (c.x + c.width / 2 <= cx + 1) return;
        along = c.x - (from.x + from.width);
        across = Math.abs(oy - cy);
        break;
      case "left":
        if (c.x + c.width / 2 >= cx - 1) return;
        along = from.x - (c.x + c.width);
        across = Math.abs(oy - cy);
        break;
    }
    const score = Math.max(along, 0) + 2 * across;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}

/** How long a direction is held before it repeats, and how fast it then repeats. */
export const REPEAT_DELAY_MS = 400;
export const REPEAT_EVERY_MS = 120;

/** Whether a held direction should fire at `now`, given when it started and last fired. */
export function shouldRepeat(heldSince: number, lastFired: number, now: number): boolean {
  return now - heldSince >= REPEAT_DELAY_MS && now - lastFired >= REPEAT_EVERY_MS;
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"]), [role="switch"], [role="slider"]';

function visibleFocusables(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
  });
}

function box(el: Element): Box {
  const r = el.getBoundingClientRect();
  return { x: r.x, y: r.y, width: r.width, height: r.height };
}

/** Move focus one step in `direction` from whatever has it. Returns false at an edge. */
export function moveFocus(direction: Direction): boolean {
  const all = visibleFocusables();
  const current = document.activeElement as HTMLElement | null;
  if (!current || current === document.body || !all.includes(current)) {
    all[0]?.focus();
    return !!all[0];
  }
  const others = all.filter((el) => el !== current);
  const i = nearest(box(current), others.map(box), direction);
  const target = others[i];
  if (!target) return false;
  target.focus({ preventScroll: true });
  // Rows glide to keep the focused tile in view (ADR-0042), unless motion is reduced.
  const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  target.scrollIntoView({ block: "nearest", inline: "nearest", behavior: reduce ? "auto" : "smooth" });
  return true;
}

/** Mark the document as driven by a pad (or not), for the ten-foot focus treatment. */
export function setPadMode(on: boolean): void {
  if (on) {
    document.documentElement.dataset.input = "pad";
    document.documentElement.dataset["focusVisible"] = "true";
  } else delete document.documentElement.dataset.input;
}

export interface GamepadHandlers {
  /** Returns true if the player took the intent (it is on screen). */
  player: (intent: Intent) => boolean;
  nav: (action: NavAction) => void;
  /** A resting thumb's tilt for the focused tile, in degrees (ADR-0042). */
  tilt?: (rx: number, ry: number) => void;
  /** A went down or up: the focused tile presses in and springs back. */
  press?: (down: boolean) => void;
  /** The View button: toggle full screen. */
  view?: () => void;
  /** The first pad input since launch. */
  pickup?: () => void;
}

function frameOf(pad: Gamepad): PadFrame {
  return {
    pressed: pad.buttons.map((b) => b.pressed || b.value > 0.6),
    stickX: pad.axes[0] ?? 0,
    stickY: pad.axes[1] ?? 0,
  };
}

/**
 * Read the first connected pad every animation frame and dispatch. Returns a stop
 * function. Mouse or keyboard input switches the ten-foot focus treatment off again.
 */
export function startGamepad(handlers: GamepadHandlers): () => void {
  let prev: PadFrame | null = null;
  const held = new Map<Direction, { since: number; last: number }>();
  let raf = 0;
  let pickedUp = false;
  let lastTilt = { rx: 0, ry: 0 };

  const fireDirection = (d: Direction) => {
    const asButton = { up: BUTTON.UP, down: BUTTON.DOWN, left: BUTTON.LEFT, right: BUTTON.RIGHT }[d];
    const intent = playerIntent(asButton);
    if (!(intent && handlers.player(intent))) handlers.nav({ kind: "move", direction: d });
  };

  const tick = (now: number) => {
    const pad = navigator.getGamepads?.().find((p): p is Gamepad => !!p && p.connected);
    if (pad) {
      const frame = frameOf(pad);
      const pressed = newlyPressed(prev, frame).filter((b) => b < BUTTON.UP || b > BUTTON.RIGHT);
      const dirs = heldDirections(frame);
      const tilt = tiltFor(frame.stickX, frame.stickY);
      const active = pressed.length > 0 || dirs.size > 0 || tilt.rx !== 0 || tilt.ry !== 0;
      if (active) {
        travel = 0;
        setPadMode(true);
        if (!pickedUp) {
          pickedUp = true;
          handlers.pickup?.();
        }
      }
      if (tilt.rx !== lastTilt.rx || tilt.ry !== lastTilt.ry) {
        lastTilt = tilt;
        handlers.tilt?.(tilt.rx, tilt.ry);
      }
      const aNow = !!frame.pressed[BUTTON.A];
      if (aNow !== !!prev?.pressed[BUTTON.A] && (aNow || prev)) handlers.press?.(aNow);
      if (pressed.includes(BUTTON.VIEW)) handlers.view?.();
      for (const b of pressed) {
        const intent = playerIntent(b);
        if (intent && handlers.player(intent)) continue;
        const action = navAction(b);
        if (action) handlers.nav(action);
      }
      for (const d of ["up", "down", "left", "right"] as Direction[]) {
        if (!dirs.has(d)) {
          held.delete(d);
          continue;
        }
        const h = held.get(d);
        if (!h) {
          held.set(d, { since: now, last: now });
          fireDirection(d);
        } else if (shouldRepeat(h.since, h.last, now)) {
          h.last = now;
          fireDirection(d);
        }
      }
      prev = frame;
    }
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);

  // Leaving TV mode re-lays the page out at 100%, so a nudged desk mouse must not do it:
  // it takes a deliberate 32px of movement, or a key.
  let travel = 0;
  const moved = (e: MouseEvent) => {
    travel += Math.abs(e.movementX) + Math.abs(e.movementY);
    if (travel > 32) {
      travel = 0;
      setPadMode(false);
    }
  };
  const off = () => setPadMode(false);
  const resetTravel = () => (travel = 0);
  window.addEventListener("mousemove", moved);
  window.addEventListener("keydown", off);
  window.addEventListener("gamepadconnected", resetTravel);
  return () => {
    cancelAnimationFrame(raf);
    window.removeEventListener("mousemove", moved);
    window.removeEventListener("keydown", off);
    window.removeEventListener("gamepadconnected", resetTravel);
    setPadMode(false);
  };
}
