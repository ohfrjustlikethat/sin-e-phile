import type { Intent } from "./intents";

/**
 * A tiny bus so inputs other than the keyboard — the Xbox controller (ADR-0041) — can
 * drive the player without knowing about it. The player subscribes while it is on
 * screen; nothing is delivered when it is not.
 */
type Listener = (intent: Intent) => void;
const listeners = new Set<Listener>();

export function onPlayerIntent(l: Listener): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Returns whether anything was listening, so a caller can fall back otherwise. */
export function sendPlayerIntent(intent: Intent): boolean {
  listeners.forEach((l) => l(intent));
  return listeners.size > 0;
}
