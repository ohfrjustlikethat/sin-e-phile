import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useShallow } from "zustand/react/shallow";
import { BackLink } from "@/design-system";
import { useUi, type Destination, type Tab } from "@/lib/store";

/**
 * The way back (Phase 16, the author's answers): top-left, just under the masthead, large, and saying where it
 * goes: "← Search results", "← Home", "← Heat". On every screen that can be left: a title page, Settings, the
 * review queue and taste profile, Downloads, and search. Escape, Backspace and the pad's B take the same step
 * (`back` in src/lib/store.ts).
 *
 * Who's watching?, opened from the masthead's avatar, covers the masthead and this bar, so it carries the same
 * link to the page under it (`hereLabel`, round 10).
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

type TitleOf = (id: number) => string | undefined;

/** The tab's own page as it stands: the title open in it, or the tab itself. */
function tabPage(s: Where, titleOf: TitleOf): string {
  const id = s.trails[s.tab].at(-1);
  return id !== undefined ? (titleOf(id) ?? "the title") : PAGE_NAMES[s.tab];
}

/** The page a step back lands on, by name; `null` when there is nowhere to go back to. */
export function backLabel(s: Where, titleOf: TitleOf): string | null {
  const trail = s.trails[s.tab];
  if (s.reviewOpen || s.tasteOpen) return "Settings";
  // The page showing under Settings or Downloads: a title, or the tab itself.
  if (s.settingsOpen || s.downloadsOpen) return tabPage(s, titleOf);
  if (trail.length >= 2) return titleOf(trail[trail.length - 2]!) ?? "the title before";
  if (trail.length === 1) return PAGE_NAMES[s.tab];
  if (s.tab === "search") return PAGE_NAMES[s.destination];
  return null;
}

/** The page showing now, by name: where a screen opened over it (Who's watching?) goes back to. */
export function hereLabel(s: Where, titleOf: TitleOf): string {
  if (s.settingsOpen && s.reviewOpen) return "Check your library";
  if (s.settingsOpen && s.tasteOpen) return "Your taste";
  if (s.settingsOpen) return "Settings";
  if (s.downloadsOpen) return "Downloads";
  return tabPage(s, titleOf);
}

/** A title already seen is in the query cache (DetailScreen's ["item", id]). */
const cachedTitle =
  (client: QueryClient): TitleOf =>
  (id) =>
    client.getQueryData<{ title: string }>(["item", id])?.title;

function useWhere(): Where {
  // Shallow: a new object every render would re-render for ever (zustand 5).
  return useUi(
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
}

/** `hereLabel` for the app as it is now. */
export function usePageName(): string {
  return hereLabel(useWhere(), cachedTitle(useQueryClient()));
}

export function BackBar() {
  const client = useQueryClient();
  const where = useWhere();
  const back = useUi((s) => s.back);
  const label = backLabel(where, cachedTitle(client));
  if (!label) return null;
  return (
    <div className="px-16 pt-7">
      <BackLink label={label} onClick={() => back()} />
    </div>
  );
}
