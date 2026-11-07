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
  /** The library's review queue (Phase 12), opened from Settings → Library. */
  reviewOpen: boolean;
  setReviewOpen: (open: boolean) => void;
  /** The taste profile (Phase 15), a settings sub-screen like the review queue. */
  tasteOpen: boolean;
  setTasteOpen: (open: boolean) => void;
  /** The Downloads page (Phase 13), opened from the masthead. */
  downloadsOpen: boolean;
  setDownloadsOpen: (open: boolean) => void;
  /** Who's watching? (Phase 14): `null` until launch has decided whether to ask. */
  pickerOpen: boolean | null;
  setPickerOpen: (open: boolean) => void;
  playing: Playing | null;
  /** The in-app mini player (ADR-0043): watching in a corner while browsing. */
  mini: boolean;
  setMini: (mini: boolean) => void;
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
      reviewOpen: false,
      setReviewOpen: (reviewOpen) => set({ reviewOpen }),
      tasteOpen: false,
      setTasteOpen: (tasteOpen) =>
        set(tasteOpen ? { tasteOpen, settingsOpen: true, reviewOpen: false, downloadsOpen: false, detail: null } : { tasteOpen }),
      downloadsOpen: false,
      pickerOpen: null,
      setPickerOpen: (pickerOpen) => set({ pickerOpen }),
      setDownloadsOpen: (downloadsOpen) =>
        set(
          downloadsOpen
            ? { downloadsOpen, settingsOpen: false, reviewOpen: false, tasteOpen: false, detail: null }
            : { downloadsOpen },
        ),
      playing: null,
      mini: false,
      setMini: (mini) => set({ mini }),
      detail: null,
      setDestination: (destination) => set({ destination, detail: null, downloadsOpen: false }),
      setSettingsOpen: (settingsOpen) =>
        set(
          settingsOpen
            ? { settingsOpen, reviewOpen: false, tasteOpen: false, downloadsOpen: false }
            : { settingsOpen, reviewOpen: false, tasteOpen: false },
        ),
      play: (playing) => set({ playing }),
      openDetail: (detail) => set({ detail, settingsOpen: false, downloadsOpen: false }),
      closeDetail: () => set({ detail: null }),
      stopPlaying: () => set({ playing: null, mini: false }),
    }),
    {
      name: "sin-e-phile.ui",
      // Only persist what should survive a restart. `settingsOpen` deliberately
      // does not: reopening the app into a settings modal would be surprising.
      partialize: (s) => ({ destination: s.destination }),
    },
  ),
);
