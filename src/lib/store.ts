import { create } from "zustand";
import { persist } from "zustand/middleware";
import { forgetScroll } from "./scrollMemory";

/**
 * The five top-level surfaces from SPEC.md §3.1. Live Channels exists from
 * Phase 18 with an honest empty state and becomes real in Phase 24; it is here
 * from the start so the navigation shape never changes under the user.
 */
export const DESTINATIONS = ["home", "films", "tv", "watchlist", "live"] as const;
export type Destination = (typeof DESTINATIONS)[number];

/**
 * Where the main area is: one of the five destinations, or search. Search stays a mode in the masthead
 * (§3.1: the navigation shape never changes), but like each destination it keeps its own trail
 * (Phase 16, the author: "each tab remembers its place").
 */
export type Tab = Destination | "search";

/** Settings' sections, in the side list's order (Phase 16, the author's answer). */
export const SETTINGS_SECTIONS = [
  "profile",
  "library",
  "addons",
  "playback",
  "subtitles",
  "downloads",
  "network",
  "controller",
  "about",
] as const;
export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];
const TABS: readonly Tab[] = [...DESTINATIONS, "search"];
const NO_TRAILS = Object.fromEntries(TABS.map((t) => [t, [] as number[]])) as unknown as Record<Tab, number[]>;

const last = (trail: number[]): number | null => (trail.length > 0 ? trail[trail.length - 1]! : null);

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
  /** The Settings section shown beside the side list. */
  settingsSection: SettingsSection;
  setSettingsSection: (section: SettingsSection) => void;
  /** The last destination shown; what the masthead marks, and what leaving search returns to. */
  destination: Destination;
  /** Where the main area is now. */
  tab: Tab;
  /** The title pages opened from each tab, oldest first: Back walks it, the tab keeps it while away. */
  trails: Record<Tab, number[]>;
  /** Search's words, kept while away from it. */
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  /** Bumped by every move between pages (a page's scroll position is put back on each). */
  navSeq: number;
  /**
   * A panel open over the page (the streams panel), closed as the first step of `back`: one way back, so Escape,
   * Backspace and the pad's B never both close it and leave the page (Phase 16, found by the pad audit).
   */
  panelBack: (() => void) | null;
  /** Go to a tab, as its masthead entry does: where it was left, or its start if it is already showing. */
  goTab: (tab: Tab) => void;
  /**
   * The search box itself, with its words: what "/", Ctrl+F and the pad's Y mean ("I want to type"). The masthead's
   * Search button is a tab and returns to where search was left; these go to its start.
   */
  searchNow: () => void;
  /**
   * One step back: an open sub-screen or overlay first (the review queue or taste profile to Settings; Settings
   * or Downloads to the page under them), then the tab's previous page, then from search to the destination.
   * `false` when there is nowhere to go back to.
   */
  back: () => boolean;
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
  /** Rate films one after another (ADR-0056), over the page it was opened from (Home, or Your taste). */
  rateOpen: boolean;
  setRateOpen: (open: boolean) => void;
  /** Who's watching? (Phase 14): `null` until launch has decided whether to ask. */
  pickerOpen: boolean | null;
  /**
   * Whether launch asked (ADR-0043), rather than the masthead's avatar: launch's question has no way back but an
   * answer; the avatar's says where back goes (round 10).
   */
  pickerAtLaunch: boolean;
  setPickerOpen: (open: boolean, atLaunch?: boolean) => void;
  playing: Playing | null;
  /** The in-app mini player (ADR-0043): watching in a corner while browsing. */
  mini: boolean;
  setMini: (mini: boolean) => void;
  /** The catalogue item whose detail page is open: the end of the current tab's trail. */
  detail: number | null;
  /** The same as `goTab` for a destination. */
  setDestination: (d: Destination) => void;
  setSettingsOpen: (open: boolean) => void;
  play: (p: Playing) => void;
  /** Open a title's page on top of the current tab's trail. */
  openDetail: (id: number) => void;
  /** Leave the title page that is showing, back to the page before it. */
  closeDetail: () => void;
  stopPlaying: () => void;
}

export const useUi = create<UiState>()(
  persist(
    (set, get) => ({
      settingsSection: "profile",
      setSettingsSection: (settingsSection) => set({ settingsSection }),
      destination: "home",
      tab: "home",
      trails: NO_TRAILS,
      searchQuery: "",
      setSearchQuery: (searchQuery) => set({ searchQuery }),
      navSeq: 0,
      panelBack: null,
      goTab: (tab) =>
        set((s) => {
          const overlay = s.settingsOpen || s.downloadsOpen || s.reviewOpen || s.tasteOpen;
          // Pressed while it is already showing: back to its start.
          const again = s.tab === tab && !overlay;
          if (again) forgetScroll(`${tab}|`);
          const trails = again ? { ...s.trails, [tab]: [] } : s.trails;
          return {
            tab,
            destination: tab === "search" ? s.destination : tab,
            trails,
            detail: last(trails[tab]),
            settingsOpen: false,
            downloadsOpen: false,
            reviewOpen: false,
            tasteOpen: false,
            rateOpen: false,
            navSeq: s.navSeq + 1,
          };
        }),
      searchNow: () =>
        set((s) => ({
          tab: "search",
          trails: { ...s.trails, search: [] },
          detail: null,
          settingsOpen: false,
          downloadsOpen: false,
          reviewOpen: false,
          tasteOpen: false,
          rateOpen: false,
          navSeq: s.navSeq + 1,
        })),
      back: () => {
        const s = get();
        const step = { navSeq: s.navSeq + 1 };
        if (s.panelBack) s.panelBack();
        else if (s.rateOpen) set({ ...step, rateOpen: false });
        else if (s.reviewOpen) set({ ...step, reviewOpen: false });
        else if (s.tasteOpen) set({ ...step, tasteOpen: false });
        else if (s.settingsOpen) set({ ...step, settingsOpen: false });
        else if (s.downloadsOpen) set({ ...step, downloadsOpen: false });
        else if (s.trails[s.tab].length > 0) {
          const trail = s.trails[s.tab].slice(0, -1);
          set({ ...step, trails: { ...s.trails, [s.tab]: trail }, detail: last(trail) });
        } else if (s.tab === "search") {
          set({ ...step, tab: s.destination, detail: last(s.trails[s.destination]) });
        } else return false;
        return true;
      },
      settingsOpen: false,
      reviewOpen: false,
      setReviewOpen: (reviewOpen) => set({ reviewOpen }),
      tasteOpen: false,
      setTasteOpen: (tasteOpen) =>
        set(tasteOpen ? { tasteOpen, settingsOpen: true, reviewOpen: false, downloadsOpen: false, detail: null } : { tasteOpen }),
      downloadsOpen: false,
      rateOpen: false,
      setRateOpen: (rateOpen) => set({ rateOpen }),
      pickerOpen: null,
      pickerAtLaunch: false,
      setPickerOpen: (pickerOpen, atLaunch = false) => set({ pickerOpen, pickerAtLaunch: pickerOpen && atLaunch }),
      setDownloadsOpen: (downloadsOpen) =>
        set(
          downloadsOpen
            ? { downloadsOpen, settingsOpen: false, reviewOpen: false, tasteOpen: false, rateOpen: false, detail: null }
            : { downloadsOpen },
        ),
      playing: null,
      mini: false,
      setMini: (mini) => set({ mini }),
      detail: null,
      setDestination: (destination) => get().goTab(destination),
      setSettingsOpen: (settingsOpen) =>
        set(
          settingsOpen
            ? { settingsOpen, reviewOpen: false, tasteOpen: false, downloadsOpen: false, rateOpen: false }
            : { settingsOpen, reviewOpen: false, tasteOpen: false },
        ),
      play: (playing) => set({ playing }),
      openDetail: (detail) =>
        set((s) => {
          const trail = s.trails[s.tab];
          const next = last(trail) === detail ? trail : [...trail, detail];
          return {
            trails: { ...s.trails, [s.tab]: next },
            detail,
            settingsOpen: false,
            downloadsOpen: false,
            reviewOpen: false,
            tasteOpen: false,
            rateOpen: false,
            navSeq: s.navSeq + 1,
          };
        }),
      closeDetail: () =>
        set((s) => {
          if (s.trails[s.tab].length === 0) return {};
          const trail = s.trails[s.tab].slice(0, -1);
          return { trails: { ...s.trails, [s.tab]: trail }, detail: last(trail), navSeq: s.navSeq + 1 };
        }),
      stopPlaying: () => set({ playing: null, mini: false }),
    }),
    {
      name: "sin-e-phile.ui",
      // Only persist what should survive a restart. `settingsOpen` deliberately
      // does not: reopening the app into a settings modal would be surprising.
      partialize: (s) => ({ destination: s.destination }),
      // The app opens on the destination it was left on, never on search or a title page.
      merge: (saved, current) => {
        const destination = (saved as { destination?: Destination } | undefined)?.destination ?? current.destination;
        return { ...current, destination, tab: destination };
      },
    },
  ),
);
