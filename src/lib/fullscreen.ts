/**
 * Full screen for the app window, from any window state (Phase 16, the author: "buggy in full screen and
 * when maximised").
 *
 * Measured in the real window on 2026-11-17: a MAXIMISED window put into full screen kept its web view at the
 * maximised height (1,528 of 1,600 pixels), so the player's bar floated 80 px above the bottom of the screen
 * with black beneath it. From a normal window the same call filled the screen. So a maximised window is
 * un-maximised first, and maximised again on the way out, which is also what a person expects to get back.
 *
 * Both callers use this: the app (View, or picking the pad up; ADR-0042) and the player's own button.
 */

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Whether the window was maximised when it went full screen, to put it back. */
let wasMaximized = false;

/** Full screen on, off, or (no argument) the other way; returns whether it is now full screen. */
export async function setWindowFullscreen(on?: boolean): Promise<boolean> {
  if (!inTauri()) return false;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  const now = await w.isFullscreen();
  const target = on ?? !now;
  if (target === now) return now;
  if (target) {
    wasMaximized = await w.isMaximized();
    if (wasMaximized) await w.unmaximize();
    await w.setFullscreen(true);
  } else {
    await w.setFullscreen(false);
    if (wasMaximized) {
      wasMaximized = false;
      await w.maximize();
    }
  }
  return target;
}
