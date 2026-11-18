import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Skeleton } from "@/design-system";
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

const WHY: Record<string, string> = {
  continue: "Where you left off",
  new_episodes: "New in the shows you follow",
  library: "On this PC, newest first",
  new: "Out this year, by your taste",
  rising: "Gaining the most ratings since the last update",
  taste: "From what you liked and what you said you love",
  watchlist: "What you saved",
  loved: "The films most people have seen and rated",
};

export function HomeScreen() {
  const home = useQuery({ queryKey: HOME, queryFn: () => ok(commands.home()) });
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

