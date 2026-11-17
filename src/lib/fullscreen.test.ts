import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
const win = { full: false, max: false };
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isFullscreen: () => Promise.resolve(win.full),
    isMaximized: () => Promise.resolve(win.max),
    unmaximize: () => {
      calls.push("unmaximize");
      win.max = false;
      return Promise.resolve();
    },
    maximize: () => {
      calls.push("maximize");
      win.max = true;
      return Promise.resolve();
    },
    setFullscreen: (on: boolean) => {
      calls.push(`fullscreen ${on}`);
      win.full = on;
      return Promise.resolve();
    },
  }),
}));
const { setWindowFullscreen } = await import("./fullscreen");

describe("Full screen from any window state (Phase 16)", () => {
  beforeEach(() => {
    calls.length = 0;
    win.full = false;
    win.max = false;
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  });
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it("un-maximises before going full screen, and maximises again after", async () => {
    // A maximised window put straight into full screen kept the maximised height (measured).
    win.max = true;
    expect(await setWindowFullscreen()).toBe(true);
    expect(calls).toEqual(["unmaximize", "fullscreen true"]);
    expect(await setWindowFullscreen(false)).toBe(false);
    expect(calls).toEqual(["unmaximize", "fullscreen true", "fullscreen false", "maximize"]);
  });

  it("leaves a normal window normal", async () => {
    await setWindowFullscreen(true);
    await setWindowFullscreen(false);
    expect(calls).toEqual(["fullscreen true", "fullscreen false"]);
  });

  it("does nothing when asked for the state it is already in", async () => {
    expect(await setWindowFullscreen(false)).toBe(false);
    expect(calls).toEqual([]);
  });
});
