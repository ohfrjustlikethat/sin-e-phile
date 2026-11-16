import { useQueryClient } from "@tanstack/react-query";
import { useShallow } from "zustand/react/shallow";
import { useUi, type Destination, type Tab } from "@/lib/store";

/**
 * The way back (Phase 16, the author's answers): top-left, just under the masthead, large, and saying where it
 * goes: "← Search results", "← Home", "← Heat". On every screen that can be left: a title page, Settings, the
 * review queue and taste profile, Downloads, and search. Escape, Backspace and the pad's B take the same step
 * (`back` in src/lib/store.ts).
 */

export const PAGE_NAMES: Record<Destination | "search", string> = {
  home: "Home",
  films: "Films",
  tv: "TV Shows",
  watchlist: "Watchlist",
  live: "Live Channels",
  search: "Search results",
};

interface Where {
  tab: Tab;
  destination: Destination;
  trails: Record<Tab, number[]>;
  settingsOpen: boolean;
  downloadsOpen: boolean;
  reviewOpen: boolean;
  tasteOpen: boolean;
}

/** The page a step back lands on, by name; `null` when there is nowhere to go back to. */
export function backLabel(s: Where, titleOf: (id: number) => string | undefined): string | null {
  const trail = s.trails[s.tab];
  // The page showing under Settings or Downloads: a title, or the tab itself.
  const here = () => {
    const id = trail[trail.length - 1];
    return id !== undefined ? (titleOf(id) ?? "the title") : PAGE_NAMES[s.tab];
  };
  if (s.reviewOpen || s.tasteOpen) return "Settings";
  if (s.settingsOpen || s.downloadsOpen) return here();
  if (trail.length >= 2) return titleOf(trail[trail.length - 2]!) ?? "the title before";
  if (trail.length === 1) return PAGE_NAMES[s.tab];
  if (s.tab === "search") return PAGE_NAMES[s.destination];
  return null;
}

export function BackBar() {
  const client = useQueryClient();
  // Shallow: a new object every render would re-render for ever (zustand 5).
  const where = useUi(
    useShallow((s) => ({
      tab: s.tab,
      destination: s.destination,
      trails: s.trails,
      settingsOpen: s.settingsOpen,
      downloadsOpen: s.downloadsOpen,
      reviewOpen: s.reviewOpen,
      tasteOpen: s.tasteOpen,
    })),
  );
  const back = useUi((s) => s.back);
  // A title already seen is in the query cache (DetailScreen's ["item", id]).
  const label = backLabel(where, (id) => client.getQueryData<{ title: string }>(["item", id])?.title);
  if (!label) return null;
  return (
    <div className="px-16 pt-7">
      <button
        type="button"
        onClick={() => back()}
        aria-label={`Back to ${label}`}
        className="group -ml-2 flex h-11 items-center gap-3 rounded-sm px-2 text-[17px] text-ink-muted transition-colors duration-[var(--dur-standard)] hover:text-ink"
      >
        <svg width="22" height="22" viewBox="0 0 22 22" fill="none" aria-hidden>
          <path d="M13.5 5L7.5 11l6 6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <span className="max-w-[48ch] truncate">{label}</span>
      </button>
    </div>
  );
}
