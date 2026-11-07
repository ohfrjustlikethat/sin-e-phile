import { listen } from "@tauri-apps/api/event";

/**
 * Listen to an event from the backend; returns the function that stops listening.
 *
 * Outside the Tauri shell (a plain browser: `tools/uiaudit`, a design review) `listen`
 * throws at once, before it returns a promise, because the shell's internals are missing.
 * A component mounted on every page (the masthead's download count) took the whole app
 * down with it there. Here that is nothing to listen to, not an error.
 */
export function onEvent<T>(name: string, handler: (payload: T) => void): () => void {
  let stop: (() => void) | undefined;
  let stopped = false;
  try {
    listen<T>(name, (e) => handler(e.payload))
      .then((off) => {
        if (stopped) off();
        else stop = off;
      })
      .catch(() => {});
  } catch {
    // Not in the shell.
  }
  return () => {
    stopped = true;
    stop?.();
  };
}
