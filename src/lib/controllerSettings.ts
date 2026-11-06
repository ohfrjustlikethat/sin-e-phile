import { create } from "zustand";

/**
 * Controller preferences — ADR-0042, Settings → Controller.
 *
 * A per-machine convenience, so it lives in the WebView's own storage rather than the
 * database. Storage can be missing or throw (a private profile, cleared data); every
 * read and write is guarded, and the defaults below are always a working state.
 */

export interface ControllerSettings {
  sounds: boolean;
  /** 0–1. Very quiet by default: the author chose "on, very subtle". */
  volume: number;
  rumble: boolean;
  /** Enter full screen the first time the pad is used after launch. */
  fullscreenOnPickup: boolean;
}

export const DEFAULTS: ControllerSettings = {
  sounds: true,
  volume: 0.15,
  rumble: true,
  fullscreenOnPickup: true,
};

const KEY = "sinephile.controller";

function load(): ControllerSettings {
  try {
    const raw = window.localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<ControllerSettings>) } : DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

interface Store extends ControllerSettings {
  set: (patch: Partial<ControllerSettings>) => void;
}

export const useControllerSettings = create<Store>((set, get) => ({
  ...load(),
  set: (patch) => {
    set(patch);
    try {
      const { sounds, volume, rumble, fullscreenOnPickup } = get();
      window.localStorage.setItem(KEY, JSON.stringify({ sounds, volume, rumble, fullscreenOnPickup }));
    } catch {
      /* storage unavailable: the setting still holds for this run */
    }
  },
}));
