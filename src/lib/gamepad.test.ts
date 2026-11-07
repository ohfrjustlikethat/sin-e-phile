import { afterEach, describe, expect, it, vi } from "vitest";
import {
  BUTTON, heldDirections, navAction, nearest, nearestInRegion, newlyPressed, playerIntent, shouldRepeat,
  startGamepad, tiltFor, TILT_MAX, type Box, type PadFrame,
} from "./gamepad";

/** ADR-0041: an Xbox controller drives the app like a remote. */

const frame = (down: number[] = [], stickX = 0, stickY = 0): PadFrame => {
  const pressed = Array(17).fill(false) as boolean[];
  down.forEach((b) => (pressed[b] = true));
  return { pressed, stickX, stickY };
};

describe("gamepad decisions", () => {
  it("a held button fires once, on the frame it went down", () => {
    expect(newlyPressed(null, frame([BUTTON.A]))).toEqual([BUTTON.A]);
    expect(newlyPressed(frame([BUTTON.A]), frame([BUTTON.A]))).toEqual([]);
    expect(newlyPressed(frame([BUTTON.A]), frame([BUTTON.A, BUTTON.B]))).toEqual([BUTTON.B]);
  });

  it("the D-pad and the stick both steer, past a deadzone", () => {
    expect([...heldDirections(frame([BUTTON.DOWN]))]).toEqual(["down"]);
    expect([...heldDirections(frame([], 0.9, 0))]).toEqual(["right"]);
    expect(heldDirections(frame([], 0.3, -0.2)).size).toBe(0);
  });

  it("maps the player buttons as ADR-0041's table says", () => {
    expect(playerIntent(BUTTON.A)).toEqual({ kind: "toggle_pause" });
    expect(playerIntent(BUTTON.B)).toEqual({ kind: "back" });
    expect(playerIntent(BUTTON.X)).toEqual({ kind: "subtitles_menu" });
    expect(playerIntent(BUTTON.RT)).toEqual({ kind: "seek", seconds: 30 });
    expect(playerIntent(BUTTON.LEFT)).toEqual({ kind: "seek", seconds: -10 });
    // Phase 10: the bumpers nudge the subtitles, as G and H do.
    expect(playerIntent(BUTTON.LB)).toEqual({ kind: "subtitle_delay", ms: -100 });
    expect(playerIntent(BUTTON.RB)).toEqual({ kind: "subtitle_delay", ms: 100 });
  });

  it("maps browsing buttons to navigation", () => {
    expect(navAction(BUTTON.A)).toEqual({ kind: "activate" });
    expect(navAction(BUTTON.RB)).toEqual({ kind: "tab", next: true });
    expect(navAction(BUTTON.Y)).toEqual({ kind: "search" });
  });

  it("repeats a held direction only after a delay", () => {
    expect(shouldRepeat(0, 0, 300)).toBe(false);
    expect(shouldRepeat(0, 0, 450)).toBe(true);
    expect(shouldRepeat(0, 450, 500)).toBe(false);
  });

  it("spatial navigation prefers what is directly in the direction asked", () => {
    const from: Box = { x: 100, y: 100, width: 100, height: 40 };
    const below: Box = { x: 100, y: 200, width: 100, height: 40 };
    const belowButFarSide: Box = { x: 600, y: 160, width: 100, height: 40 };
    const above: Box = { x: 100, y: 20, width: 100, height: 40 };
    expect(nearest(from, [belowButFarSide, below, above], "down")).toBe(1);
    expect(nearest(from, [belowButFarSide, below, above], "up")).toBe(2);
    expect(nearest(from, [below, above], "left")).toBe(-1);
  });
});

describe("stick tilt (ADR-0042)", () => {
  it("a resting thumb tilts toward itself; noise and navigation do not", () => {
    expect(tiltFor(0.05, 0)).toEqual({ rx: 0, ry: 0 }); // noise
    expect(tiltFor(0.9, 0)).toEqual({ rx: 0, ry: 0 }); // that is a move, not a tilt
    const right = tiltFor(0.4, 0);
    expect(right.ry).toBeGreaterThan(0);
    expect(right.rx).toBeCloseTo(0);
    const up = tiltFor(0, -0.4);
    expect(up.rx).toBeGreaterThan(0);
  });

  it("never tilts past the maximum", () => {
    for (const [x, y] of [[0.49, 0], [0.35, 0.35], [-0.2, 0.45]] as const) {
      const t = tiltFor(x, y);
      expect(Math.hypot(t.rx, t.ry)).toBeLessThanOrEqual(TILT_MAX + 1e-9);
    }
  });
});

describe("startGamepad, with a simulated pad", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("sends player intents when the player takes them, navigation otherwise", () => {
    let pad = { connected: true, buttons: Array(17).fill({ pressed: false, value: 0 }), axes: [0, 0] };
    vi.stubGlobal("navigator", { getGamepads: () => [pad] });
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    const player = vi.fn(() => true);
    const nav = vi.fn();
    const stop = startGamepad({ player, nav });

    const press = (b: number) => {
      const buttons = Array(17).fill({ pressed: false, value: 0 });
      buttons[b] = { pressed: true, value: 1 };
      pad = { ...pad, buttons };
    };
    press(BUTTON.A);
    frames.shift()?.(16);
    expect(player).toHaveBeenCalledWith({ kind: "toggle_pause" });
    expect(document.documentElement.dataset.input).toBe("pad");

    player.mockReturnValue(false); // the player is not on screen
    press(BUTTON.RB);
    frames.shift()?.(32);
    expect(nav).toHaveBeenCalledWith({ kind: "tab", next: true });
    stop();
    expect(document.documentElement.dataset.input).toBeUndefined();
  });

  it("reports pick-up once, A down and up for the press, View, and a resting tilt", () => {
    let pad = { connected: true, buttons: Array(17).fill({ pressed: false, value: 0 }), axes: [0, 0] };
    vi.stubGlobal("navigator", { getGamepads: () => [pad] });
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    const h = { player: vi.fn(() => false), nav: vi.fn(), tilt: vi.fn(), press: vi.fn(), view: vi.fn(), pickup: vi.fn() };
    const stop = startGamepad(h);
    const set = (down: number[], axes = [0, 0]) => {
      const buttons = Array(17).fill({ pressed: false, value: 0 });
      down.forEach((b) => (buttons[b] = { pressed: true, value: 1 }));
      pad = { ...pad, buttons, axes };
      frames.shift()?.(performance.now());
    };
    set([BUTTON.A]);
    set([BUTTON.A]);
    set([]);
    expect(h.press.mock.calls).toEqual([[true], [false]]);
    set([BUTTON.VIEW]);
    expect(h.view).toHaveBeenCalledTimes(1);
    set([], [0.3, 0]);
    expect(h.tilt).toHaveBeenLastCalledWith(0, expect.any(Number));
    expect(h.tilt.mock.lastCall?.[1]).toBeGreaterThan(0);
    set([], [0, 0]);
    expect(h.tilt).toHaveBeenLastCalledWith(0, 0);
    expect(h.pickup).toHaveBeenCalledTimes(1);
    stop();
  });
});

describe("nearest, wide targets (Phase 9)", () => {
  it("goes up from a small option to the wide button it sits under, not past it", () => {
    // Play spans 100-400; "German" sits under its right end; a masthead link is far above
    // but centred over the option.
    const option = { x: 380, y: 300, width: 60, height: 20 };
    const play = { x: 100, y: 230, width: 300, height: 50 };
    const masthead = { x: 390, y: 20, width: 60, height: 20 };
    expect(nearest(option, [masthead, play], "up")).toBe(1);
  });

  it("among options under a wide button, goes down to the nearest one", () => {
    const play = { x: 100, y: 230, width: 300, height: 50 };
    const options = [80, 160, 240].map((x) => ({ x, y: 300, width: 60, height: 20 }));
    expect(nearest(play, options, "down")).toBe(2);
  });
});


describe("nearestInRegion (Phase 14)", () => {
  it("climbs the page to a button off screen above before it leaves for the masthead", () => {
    // A field near the top of the view; the masthead just above it; the page's previous
    // button scrolled 1,300 px up. Plain nearest takes the masthead.
    const field = { x: 64, y: 249, width: 640, height: 40 };
    const masthead = { x: 680, y: 49, width: 40, height: 20 };
    const offScreen = { x: 64, y: -1293, width: 120, height: 32 };
    expect(nearest(field, [masthead, offScreen], "up")).toBe(0);
    expect(nearestInRegion(field, [masthead, offScreen], [false, true], "up")).toBe(1);
  });

  it("leaves the region when nothing more lies that way in it", () => {
    const top = { x: 64, y: 120, width: 640, height: 40 };
    const masthead = { x: 680, y: 49, width: 40, height: 20 };
    const below = { x: 64, y: 300, width: 640, height: 40 };
    expect(nearestInRegion(top, [masthead, below], [false, true], "up")).toBe(0);
    expect(nearestInRegion(top, [masthead, below], [false, true], "down")).toBe(1);
  });
});

describe("nearest, the beam (Phase 14)", () => {
  it("moves along the row to a far target rather than to a nearer one off it", () => {
    // Back, Skip and Continue on one row; a wide button above-right of Back, closer.
    const back = { x: 449, y: 932, width: 104, height: 48 };
    const more = { x: 449, y: 790, width: 328, height: 50 };
    const skip = { x: 1210, y: 932, width: 72, height: 48 };
    const next = { x: 1307, y: 932, width: 142, height: 48 };
    expect(nearest(back, [more, skip, next], "right")).toBe(1);
    // With nothing on the row, the general rule still finds the button off it.
    expect(nearest(back, [more], "right")).toBe(0);
  });
});
