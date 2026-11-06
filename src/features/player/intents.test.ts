import { describe, expect, it } from "vitest";
import { clock, keyIntent, nextSpeed, SHORTCUTS } from "./intents";

const key = (k: string, mods: Partial<{ shiftKey: boolean; ctrlKey: boolean }> = {}) =>
  keyIntent({ key: k, shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, ...mods });

describe("player keys (SPEC.md Phase 8)", () => {
  it("maps every key the spec names", () => {
    expect(key(" ")).toEqual({ kind: "toggle_pause" });
    expect(key("k")).toEqual({ kind: "toggle_pause" });
    expect(key("j")).toEqual({ kind: "seek", seconds: -10 });
    expect(key("l")).toEqual({ kind: "seek", seconds: 10 });
    expect(key("ArrowRight")).toEqual({ kind: "seek", seconds: 5 });
    expect(key("ArrowRight", { shiftKey: true })).toEqual({ kind: "seek", seconds: 60 });
    expect(key("f")).toEqual({ kind: "fullscreen" });
    expect(key("m")).toEqual({ kind: "mute" });
    expect(key(",")).toEqual({ kind: "frame", forward: false });
    expect(key(".")).toEqual({ kind: "frame", forward: true });
    expect(key("[")).toEqual({ kind: "speed", faster: false });
    expect(key("]")).toEqual({ kind: "speed", faster: true });
  });

  it("G and H nudge the subtitles a tenth of a second (Phase 10)", () => {
    expect(key("g")).toEqual({ kind: "subtitle_delay", ms: -100 });
    expect(key("H")).toEqual({ kind: "subtitle_delay", ms: 100 });
  });

  it("leaves keys it does not own, and every chord, to the rest of the app", () => {
    expect(key("/")).toBeNull();
    expect(key("f", { ctrlKey: true })).toBeNull();
  });

  it("steps speed within its range", () => {
    expect(nextSpeed(1, true)).toBe(1.25);
    expect(nextSpeed(2, true)).toBe(2);
    expect(nextSpeed(0.5, false)).toBe(0.5);
    expect(nextSpeed(1.1, false)).toBe(0.75);
  });

  it("formats time the way a viewer reads it", () => {
    expect(clock(7)).toBe("0:07");
    expect(clock(723)).toBe("12:03");
    expect(clock(3729)).toBe("1:02:09");
    expect(clock(null)).toBe("0:00");
  });
});

describe("the shortcuts list (Phase 11 E4)", () => {
  it("says what each key really does", () => {
    for (const row of SHORTCUTS.filter((r) => r.keys !== "")) {
      expect(keyIntent(row.sample), row.keys).toEqual(row.intent);
    }
  });

  it("names a pad button for everything the keyboard can do, or the player menu", () => {
    // ADR-0041: what the keyboard reaches, the pad reaches; the rest goes through Menu.
    expect(SHORTCUTS.some((r) => r.pad === "Menu")).toBe(true);
  });
});

