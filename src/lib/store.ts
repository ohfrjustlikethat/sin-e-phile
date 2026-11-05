import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * The five top-level surfaces from SPEC.md §3.1. Live Channels exists from
 * Phase 18 with an honest empty state and becomes real in Phase 24; it is here
 * from the start so the navigation shape never changes under the user.
 */
export const DESTINATIONS = ["home", "films", "tv", "watchlist", "live"] as const;
export type Destination = (typeof DESTINATIONS)[number];

/** What the player is showing. Not persisted: a restart resumes from the saved position. */
export interface Playing {
  /** An http(s) address or a file path, as the player opens it. */
  target: string;
  title: string;
  /** The catalogue item, for saving the position. */
  mediaItemId: number | null;
  /** Seconds to start from (position resume). */
  start: number | null;
}

interface UiState {
  destination: Destination;
  settingsOpen: boolean;
  playing: Playing | null;
  /** The catalogue item whose detail page is open. */
  detail: number | null;
  setDestination: (d: Destination) => void;
  setSettingsOpen: (open: boolean) => void;
  play: (p: Playing) => void;
  openDetail: (id: number) => void;
  closeDetail: () => void;
  stopPlaying: () => void;
}

export const useUi = create<UiState>()(
  persist(
    (set) => ({
      destination: "home",
      settingsOpen: false,
      playing: null,
      detail: null,
      setDestination: (destination) => set({ destination, detail: null }),
      setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
      play: (playing) => set({ playing }),
      openDetail: (detail) => set({ detail, settingsOpen: false }),
      closeDetail: () => set({ detail: null }),
      stopPlaying: () => set({ playing: null }),
    }),
    {
      name: "sin-e-phile.ui",
      // Only persist what should survive a restart. `settingsOpen` deliberately
      // does not: reopening the app into a settings modal would be surprising.
      partialize: (s) => ({ destination: s.destination }),
    },
  ),
);
