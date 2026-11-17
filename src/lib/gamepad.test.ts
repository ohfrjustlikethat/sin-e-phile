import { afterEach, describe, expect, it, vi } from "vitest";
import {
  adjustValue, BUTTON, FOLLOW_GRACE_MS, noteFocus, rescueFocus, heldDirections, moveFocus, navAction, nearest, nearestBox,
  nearestInRegion, newlyPressed, onScreen, playerIntent, scrollByStick, scrollSpeed, SCROLL_MAX,
  shouldRepeat, startGamepad, tiltFor, TILT_MAX, type Box, type PadFrame,
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

  it("maps the player buttons as ADR-0045's TV-standard table says", () => {
    expect(playerIntent(BUTTON.A)).toEqual({ kind: "toggle_pause" });
    expect(playerIntent(BUTTON.B)).toEqual({ kind: "back" });
    expect(playerIntent(BUTTON.Y)).toEqual({ kind: "subtitles_menu" });
    expect(playerIntent(BUTTON.X)).toEqual({ kind: "audio_menu" });
    expect(playerIntent(BUTTON.MENU)).toEqual({ kind: "player_menu" });
    expect(playerIntent(BUTTON.VIEW)).toEqual({ kind: "fullscreen" });
    expect(playerIntent(BUTTON.LEFT)).toEqual({ kind: "seek", seconds: -10 });
    expect(playerIntent(BUTTON.RIGHT)).toEqual({ kind: "seek", seconds: 10 });
    expect(playerIntent(BUTTON.UP)).toEqual({ kind: "volume", delta: 5 });
    expect(playerIntent(BUTTON.DOWN)).toEqual({ kind: "volume", delta: -5 });
    expect(playerIntent(BUTTON.LB)).toEqual({ kind: "episode", next: false });
    expect(playerIntent(BUTTON.RB)).toEqual({ kind: "episode", next: true });
    expect(playerIntent(BUTTON.RT)).toEqual({ kind: "seek", seconds: 30 });
    // The right stick scrolls; clicking it does nothing in the player.
    expect(playerIntent(BUTTON.RS)).toBeNull();
  });

  it("the right stick scrolls faster the further it is pushed, nothing when resting", () => {
    expect(scrollSpeed(0.15)).toBe(0);
    expect(scrollSpeed(-0.2)).toBe(0);
    expect(scrollSpeed(1)).toBe(SCROLL_MAX);
    expect(scrollSpeed(-1)).toBe(-SCROLL_MAX);
    const half = scrollSpeed(0.6);
    expect(half).toBeGreaterThan(0);
    expect(half).toBeLessThan(SCROLL_MAX / 2);
  });

  it("focus that scrolled out of view goes to the nearest box in view", () => {
    const gone: Box = { x: 100, y: -200, width: 100, height: 40 };
    const top: Box = { x: 100, y: 10, width: 100, height: 40 };
    const far: Box = { x: 900, y: 10, width: 100, height: 40 };
    const lower: Box = { x: 100, y: 400, width: 100, height: 40 };
    expect(nearestBox(gone, [lower, far, top])).toBe(2);
    expect(nearestBox(gone, [])).toBe(-1);
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

  it("View goes to the player's full screen while it plays, the window's otherwise, never both", () => {
    let pad = { connected: true, buttons: Array(17).fill({ pressed: false, value: 0 }), axes: [0, 0] };
    vi.stubGlobal("navigator", { getGamepads: () => [pad] });
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    const player = vi.fn(() => true);
    const view = vi.fn();
    const stop = startGamepad({ player, nav: vi.fn(), view });
    const tap = (at: number) => {
      const buttons = Array(17).fill({ pressed: false, value: 0 });
      buttons[BUTTON.VIEW] = { pressed: true, value: 1 };
      pad = { ...pad, buttons };
      frames.shift()?.(at);
      pad = { ...pad, buttons: Array(17).fill({ pressed: false, value: 0 }) };
      frames.shift()?.(at + 16);
    };
    tap(16);
    expect(player).toHaveBeenCalledWith({ kind: "fullscreen" });
    expect(view).not.toHaveBeenCalled();

    player.mockReturnValue(false); // the player is not on screen
    tap(64);
    expect(view).toHaveBeenCalledTimes(1);
    stop();
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

/**
 * jsdom lays nothing out, so each element is placed by hand: its box, and for a scroller its
 * sizes and a scroll offset that moves what is inside it.
 */
function place(el: Element, x: number, y: number, width: number, height: number): void {
  (el as HTMLElement).getBoundingClientRect = () =>
    ({ x, y, width, height, left: x, top: y, right: x + width, bottom: y + height, toJSON: () => ({}) }) as DOMRect;
}
function scroller(el: HTMLElement, axis: "x" | "y", onScroll: (offset: number) => void): void {
  let offset = 0;
  const [pos, size, view] = axis === "x"
    ? ["scrollLeft", "scrollWidth", "clientWidth"]
    : ["scrollTop", "scrollHeight", "clientHeight"];
  if (axis === "x") el.style.overflowX = "auto";
  else el.style.overflowY = "auto";
  Object.defineProperty(el, size, { value: 4000 });
  Object.defineProperty(el, view, { value: 600 });
  Object.defineProperty(el, pos, {
    get: () => offset,
    set: (v: number) => {
      offset = Math.max(0, Math.min(3400, v));
      onScroll(offset);
    },
  });
}
const byId = (id: string) => document.getElementById(id) as HTMLElement;

describe("focus and the right stick, on a page (Phase 16)", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });
  // jsdom has no scrollIntoView; moveFocus asks for it after every move.
  Element.prototype.scrollIntoView = () => {};

  it("a card clipped by its rail is not on screen, though it is inside the window", () => {
    document.body.innerHTML =
      '<main><div id="rail"><button id="seen">a</button><button id="clipped">b</button>' +
      '<button id="away">c</button></div></main>';
    byId("rail").style.overflowX = "auto";
    place(byId("rail"), 300, 100, 600, 200);
    place(byId("seen"), 320, 120, 200, 150);
    place(byId("clipped"), 100, 120, 150, 150); // in the window, left of the rail
    place(byId("away"), 1100, 120, 200, 150); // past the window's edge
    expect(onScreen(byId("seen"))).toBe(true);
    expect(onScreen(byId("clipped"))).toBe(false);
    expect(onScreen(byId("away"))).toBe(false);
  });

  it("a page can name where a direction goes, when that element can take focus", () => {
    // Down from the title page's wide Play is the first round button, not the one under its middle.
    document.body.innerHTML =
      '<main><button id="play" data-pad-down="first">Play</button>' +
      '<button id="first">Download</button><button id="middle">Watched</button></main>';
    place(byId("play"), 0, 0, 560, 64);
    place(byId("first"), 0, 100, 96, 90);
    place(byId("middle"), 240, 100, 96, 90);
    byId("play").focus();
    expect(moveFocus("down")).toBe(true);
    expect(document.activeElement?.id).toBe("first");
    // Not on screen: the nearest box, as ever.
    byId("play").focus();
    place(byId("first"), 0, 0, 0, 0);
    moveFocus("down");
    expect(document.activeElement?.id).toBe("middle");
  });

  it("a dialog holds the D-pad: Down at its foot stays in it, where a plain group lets go", () => {
    for (const role of ["dialog", "group"]) {
      document.body.innerHTML =
        `<main><div role="${role}"><button id="first">a</button><button id="last">b</button></div>` +
        '<button id="bar">bar</button></main>';
      place(byId("first"), 100, 100, 200, 40);
      place(byId("last"), 100, 200, 200, 40);
      place(byId("bar"), 100, 400, 600, 40);
      byId("last").focus();
      const moved = moveFocus("down");
      if (role === "dialog") {
        expect(moved).toBe(false);
        expect(document.activeElement?.id).toBe("last");
        expect(moveFocus("up")).toBe(true);
        expect(document.activeElement?.id).toBe("first");
      } else {
        expect(document.activeElement?.id).toBe("bar");
      }
    }
  });

  it("a card unmounted under focus gives focus back to a card in view, only within the grace", () => {
    document.body.innerHTML =
      '<main><div id="rail"><button id="gone">a</button><button id="near">b</button>' +
      '<button id="further">d</button><button id="clipped">c</button></div></main>';
    const rail = byId("rail");
    place(rail, 300, 100, 600, 200);
    place(byId("gone"), 320, 120, 200, 150);
    place(byId("near"), 600, 120, 150, 150);
    place(byId("further"), 760, 120, 130, 150); // in view: somewhere focus could go
    place(byId("clipped"), 130, 120, 150, 150); // nearer to where focus was, but cut off
    scroller(rail, "x", () => {});
    byId("gone").focus();
    expect(scrollByStick(1, 0, 16)).toBe(true);
    expect(rail.scrollLeft).toBeGreaterThan(0);
    byId("gone").remove(); // the rail drops the card a frame later
    expect(document.activeElement).toBe(document.body);
    rescueFocus(performance.now());
    expect(document.activeElement?.id).toBe("near");

    // Long after the stick last moved anything, a lost focus is left where it fell.
    byId("near").remove();
    rescueFocus(performance.now() + FOLLOW_GRACE_MS + 1);
    expect(document.activeElement).toBe(document.body);
  });

  it("with the pad driving, a button that disables itself under focus hands it to its neighbour", () => {
    document.body.innerHTML =
      '<main><input id="path"><button id="add">Add</button><button id="far">Continue</button></main>';
    place(byId("path"), 100, 100, 300, 40);
    place(byId("add"), 420, 100, 80, 40);
    place(byId("far"), 100, 600, 200, 40);
    document.documentElement.dataset.input = "pad";
    try {
      byId("add").focus();
      noteFocus();
      // Pressed, its field cleared: it disables itself and the browser drops its focus.
      byId("add").blur();
      (byId("add") as HTMLButtonElement).disabled = true;
      expect(document.activeElement).toBe(document.body);
      rescueFocus(performance.now() + FOLLOW_GRACE_MS * 10); // long after any scrolling
      expect(document.activeElement?.id).toBe("path");

      // While a film plays the player owns the buttons, and nothing is moved.
      document.documentElement.dataset.playing = "";
      noteFocus();
      byId("path").blur();
      rescueFocus(performance.now() + FOLLOW_GRACE_MS * 10);
      expect(document.activeElement).toBe(document.body);
    } finally {
      delete document.documentElement.dataset.input;
      delete document.documentElement.dataset.playing;
    }
  });

  it("while a film plays, a focus lost inside the open panel is put back within it", () => {
    document.body.innerHTML =
      '<main><button id="bar">Play</button></main>' +
      '<div role="dialog" aria-label="Player menu"><div id="found"><button id="offer">English</button></div>' +
      '<button id="later">Subtitles later</button></div>';
    place(byId("bar"), 100, 900, 100, 40);
    place(byId("offer"), 600, 500, 300, 40);
    place(byId("later"), 600, 300, 300, 40); // jsdom's window is 1024 wide
    document.documentElement.dataset.input = "pad";
    document.documentElement.dataset.playing = "";
    try {
      byId("offer").focus();
      noteFocus();
      byId("found").remove(); // choosing a found subtitle closes the list it was in
      expect(document.activeElement).toBe(document.body);
      rescueFocus(performance.now());
      expect(document.activeElement?.id).toBe("later");
    } finally {
      delete document.documentElement.dataset.input;
      delete document.documentElement.dataset.playing;
    }
  });

  it("a screen that goes takes focus with it: the next screen decides where focus starts", () => {
    document.body.innerHTML = '<main><div id="film"><button id="see">See downloads</button></div></main>';
    place(byId("see"), 100, 800, 200, 40);
    document.documentElement.dataset.input = "pad";
    try {
      byId("see").focus();
      noteFocus();
      // A opened the Downloads page: the film page is gone, and a select sits where the button was.
      byId("film").remove();
      document.querySelector("main")!.innerHTML =
        '<div id="downloads"><button id="pause">Pause</button><select id="at-once"></select></div>';
      place(byId("pause"), 100, 200, 100, 40);
      place(byId("at-once"), 100, 800, 200, 40);
      rescueFocus(performance.now());
      expect(document.activeElement).toBe(document.body);
    } finally {
      delete document.documentElement.dataset.input;
    }
  });

  it("scrolling a dialog keeps focus inside it, though the bar beneath is nearer", () => {
    document.body.innerHTML =
      '<header><button id="bar">bar</button></header>' +
      '<div role="dialog" id="panel"><button id="top">a</button><button id="low">b</button></div>';
    const panel = byId("panel");
    place(panel, 100, 50, 600, 600);
    place(byId("bar"), 100, 0, 600, 40);
    place(byId("top"), 120, 60, 200, 40);
    place(byId("low"), 120, 500, 200, 40);
    // Scrolling the panel carries its first control up out of the window.
    scroller(panel, "y", (offset) => place(byId("top"), 120, 60 - offset * 10, 200, 40));
    byId("top").focus();
    expect(scrollByStick(0, 1, 16)).toBe(true);
    expect(document.activeElement?.id).toBe("low");
  });
});

describe("a slider by pad (Phase 16, ADR-0041)", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("Left and Right step a focused slider and say so; Up and Down are left to move focus", () => {
    document.body.innerHTML = '<input id="size" type="range" min="30" max="80" step="2" value="46">';
    const size = byId("size") as HTMLInputElement;
    const seen: string[] = [];
    size.addEventListener("input", () => seen.push(size.value));
    size.focus();
    expect(adjustValue("right")).toBe("changed");
    expect(adjustValue("right")).toBe("changed");
    expect(adjustValue("left")).toBe("changed");
    expect(seen).toEqual(["48", "50", "48"]);
    expect(adjustValue("up")).toBeNull();
    expect(adjustValue("down")).toBeNull();
  });

  it("at either end the slider holds focus and changes nothing", () => {
    document.body.innerHTML = '<input id="h" type="range" min="0" max="10" value="10">';
    const h = byId("h") as HTMLInputElement;
    const changed = vi.fn();
    h.addEventListener("input", changed);
    h.focus();
    expect(adjustValue("right")).toBe("end");
    expect(changed).not.toHaveBeenCalled();
    expect(h.value).toBe("10");
  });

  it("anything that is not a slider is left to moveFocus", () => {
    document.body.innerHTML = '<button id="b">a</button><input id="t" type="text">';
    byId("b").focus();
    expect(adjustValue("right")).toBeNull();
    byId("t").focus();
    expect(adjustValue("left")).toBeNull();
  });
});
