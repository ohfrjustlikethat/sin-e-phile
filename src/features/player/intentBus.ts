import type { Intent } from "./intents";

/**
 * A tiny bus so inputs other than the keyboard — the Xbox controller (ADR-0041) — can
 * drive the player without knowing about it. The player subscribes while it is on
 * screen; nothing is delivered when it is not.
 */
/** Returns `false` to decline an intent (the player's menu is open, or it is the mini
 * player), so the caller falls back to ordinary navigation. */
type Listener = (intent: Intent) => boolean | void;
const listeners = new Set<Listener>();

export function onPlayerIntent(l: Listener): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Returns whether the player took it, so a caller can fall back otherwise. */
export function sendPlayerIntent(intent: Intent): boolean {
  let taken = false;
  listeners.forEach((l) => {
    if (l(intent) !== false) taken = true;
  });
  return taken;
}
