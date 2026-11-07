import { createJSONStorage } from "zustand/middleware";

/**
 * Browser storage for what is personal (Phase 14): the playback choice, recent searches.
 * Each profile's values sit under their own key, so a switch never shows one person's
 * language or searches to another. Machine settings (the controller, the last
 * destination) are not personal and do not use this.
 *
 * Before profiles existed these lived under the bare key. The first profile to load
 * claims them, once, so nobody's saved choices vanish on upgrading.
 *
 * Every access is guarded: storage can be missing or refuse (a private window).
 */

let current: number | null = null;

function scoped(key: string): string {
  return current === null ? key : `${key}.profile-${current}`;
}

function safe<T>(f: () => T, fallback: T): T {
  try {
    return f();
  } catch {
    return fallback;
  }
}

export const profileStorage = createJSONStorage(() => ({
  getItem: (key: string) =>
    safe(() => {
      const own = window.localStorage.getItem(scoped(key));
      if (own !== null || current === null) return own;
      // Claim the pre-profile value, once.
      const claimed = `${key}.claimed`;
      const legacy = window.localStorage.getItem(key);
      if (legacy === null || window.localStorage.getItem(claimed) !== null) return null;
      window.localStorage.setItem(claimed, String(current));
      window.localStorage.setItem(scoped(key), legacy);
      return legacy;
    }, null),
  setItem: (key: string, value: string) =>
    safe(() => window.localStorage.setItem(scoped(key), value), undefined),
  removeItem: (key: string) => safe(() => window.localStorage.removeItem(scoped(key)), undefined),
}));

/** Whose values are read and written from now on. */
export function setStorageProfile(id: number): void {
  current = id;
}

/** A store whose persisted values are personal. */
interface Rehydratable {
  getInitialState: () => unknown;
  setState: (s: never, replace: true) => void;
  persist: {
    rehydrate: () => Promise<void> | void;
    getOptions: () => { name?: string; storage?: { getItem: (name: string) => unknown } | undefined };
  };
}

/**
 * Show `id`'s own values: its saved ones if it has any, the defaults if not. Not "reset,
 * then read": the persist middleware writes on every setState, so a reset first would
 * overwrite the profile's saved values with blanks before they were read.
 */
export async function switchStores(id: number, stores: Rehydratable[]): Promise<void> {
  setStorageProfile(id);
  for (const s of stores) {
    const { name, storage } = s.persist.getOptions();
    const saved = name && storage ? await storage.getItem(name) : null;
    if (saved) await s.persist.rehydrate();
    else s.setState(s.getInitialState() as never, true);
  }
}
