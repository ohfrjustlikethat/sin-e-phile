import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Skeleton } from "@/design-system";
import { useScreenHints, type Hint } from "@/design-system/focus/FocusLayer";
import { commands } from "@/lib/ipc";
import { onEvent } from "@/lib/events";
import { SetupCard } from "./SetupCard";
import { TitleRail } from "./TitleRail";

/**
 * Home, first version (SPEC.md Phase 14; Phase 18 overhauls it). Continue watching, where
 * your taste points, your watchlist, and what is widely loved: the last needs only the
 * offline catalogue, so Home is never empty. Typographic title cards: a default install has
 * no artwork, and §9.4 makes that state the designed one. The first rail leads with a
 * larger card, so the page is not a grid of identical tiles (§9.0).
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const HOME = ["home"];

/** Home's pad hints: X on a recommended card says "not for me" (Phase 18). */
const HINTS: Hint[] = [
  ["A", "Select"],
  ["B", "Back"],
  ["X", "Not for me…"],
  ["Y", "Search"],
  ["Menu", "Settings"],
];

const WHY: Record<string, string> = {
  continue: "Where you left off",
  new_episodes: "New in the shows you follow",
  library: "On this PC, newest first",
  new: "Out this year, by your taste",
  rising: "Gaining the most ratings since the last update",
  taste: "From what you liked and what you said you love",
  // Phase 18's discovery rails: each card says why it is there.
  stretch: "Close to one of your tastes, one step outside it",
  bridge: "Close to two of your tastes at once",
  blind_spot: "Acclaimed, from somewhere you have never been",
  canon: "Rated 7.8 or more by 50,000 people, close to your taste",
  deep_cut: "Rated 7.5 or more by at most 15,000 people",
  watchlist: "What you saved",
  loved: "The films most people have seen and rated",
};

export function HomeScreen() {
  // The hour: the part of the day the discovery engine learns and chooses in (Phase 18).
  const home = useQuery({ queryKey: HOME, queryFn: () => ok(commands.home(new Date().getHours())) });
  useScreenHints(HINTS);
  // New titles from the catalogue, and anything new in the library, show at once.
  const client = useQueryClient();
  useEffect(() => {
    const again = () => void client.invalidateQueries({ queryKey: HOME });
    const offs = [
      onEvent<null>("catalogue-refreshed", again),
      onEvent<number>("library-changed", again),
      onEvent<{ phase: string }>("library-scan", (e) => {
        if (e.phase === "done") again();
      }),
    ];
    return () => offs.forEach((off) => off());
  }, [client]);
  if (home.isPending) {
    return (
      <div className="flex flex-col gap-10 px-16 pt-12">
        <Skeleton className="h-6 w-56" />
        <Skeleton className="h-[260px] w-full" />
      </div>
    );
  }
  if (home.isError) {
    return <p className="px-16 pt-12 text-sm text-ink-muted">{home.error.message}</p>;
  }
  return (
    <div className="flex flex-col gap-12 pt-10 pb-24">
      <SetupCard />
      {home.data.map((r, i) => (
        <TitleRail key={r.key} rail={r} lead={i === 0} why={WHY[r.key] ?? ""} />
      ))}
    </div>
  );
}

