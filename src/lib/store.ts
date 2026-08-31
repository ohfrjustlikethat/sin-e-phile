import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * The five top-level surfaces from SPEC.md §3.1. Live Channels exists from
 * Phase 18 with an honest empty state and becomes real in Phase 24; it is here
 * from the start so the navigation shape never changes under the user.
 */
export const DESTINATIONS = ["home", "films", "tv", "watchlist", "live"] as const;
export type Destination = (typeof DESTINATIONS)[number];

interface UiState {
  destination: Destination;
  /** §9.4: the rail remembers whether it is collapsed. */
  railExpanded: boolean;
  settingsOpen: boolean;
  setDestination: (d: Destination) => void;
  toggleRail: () => void;
  setSettingsOpen: (open: boolean) => void;
}

export const useUi = create<UiState>()(
  persist(
    (set) => ({
      destination: "home",
      railExpanded: true,
      settingsOpen: false,
      setDestination: (destination) => set({ destination }),
      toggleRail: () => set((s) => ({ railExpanded: !s.railExpanded })),
      setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
    }),
    {
      name: "sin-e-phile.ui",
      // Only persist what should survive a restart. `settingsOpen` deliberately
      // does not: reopening the app into a settings modal would be surprising.
      partialize: (s) => ({ destination: s.destination, railExpanded: s.railExpanded }),
    },
  ),
);
