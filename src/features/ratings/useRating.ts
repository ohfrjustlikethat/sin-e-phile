import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/ipc";
import { toast } from "@/lib/toast";

/**
 * Ratings for any title (ADR-0056): a film's own half stars, and a show's, season by season. The stars move at once;
 * the rating is saved once the stars stop moving (stepping by key or pad is one save, not five), and the taste model,
 * Home and the count Home asks by follow.
 */

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: string };

async function ok<T>(p: Promise<Result<T>>): Promise<T> {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const RATING = (id: number) => ["rating", id] as const;
export const SHOW_RATINGS = (id: number) => ["show-ratings", id] as const;
export const RATED_COUNT = ["rated-count"] as const;

const SETTLE_MS = 350;
const pending = new Map<string, ReturnType<typeof setTimeout>>();

/** Save after the stars have stopped moving; a later change to the same rating replaces an earlier one. */
function settle(key: string, client: QueryClient, save: () => Promise<Result<null>>) {
  const waiting = pending.get(key);
  if (waiting) clearTimeout(waiting);
  pending.set(
    key,
    setTimeout(() => {
      pending.delete(key);
      void save().then((r) => {
        if (r.status === "error") {
          toast(`Rating not saved: ${r.error}`, "danger");
          return;
        }
        void client.invalidateQueries({ queryKey: RATED_COUNT });
        // What is recommended follows the taste the rating changed.
        void client.invalidateQueries({ queryKey: ["home"] });
      });
    }, SETTLE_MS),
  );
}

/** A film's (or anything rated as a whole) own rating, in halves. */
export function useTitleRating(id: number) {
  const client = useQueryClient();
  const q = useQuery({ queryKey: RATING(id), queryFn: () => ok(commands.titleRating(id)) });
  const set = (halves: number | null) => {
    // A load still on its way would put back what was there before.
    void client.cancelQueries({ queryKey: RATING(id) }, { revert: false });
    client.setQueryData(RATING(id), halves);
    settle(`title-${id}`, client, () => commands.rateTitle(id, halves));
  };
  return { halves: q.data ?? null, set, ready: q.isSuccess };
}

/** A show's rated seasons and their average, in halves. */
export function useShowRatings(series: number) {
  const client = useQueryClient();
  const q = useQuery({ queryKey: SHOW_RATINGS(series), queryFn: () => ok(commands.showRatings(series)) });
  const seasons = q.data?.seasons ?? [];
  const of = (season: number) => seasons.find((s) => s.season === season)?.halves ?? null;
  const set = (season: number, halves: number | null) => {
    const others = seasons.filter((s) => s.season !== season);
    const next = halves === null ? others : [...others, { season, halves }].sort((a, b) => a.season - b.season);
    const average = next.length ? next.reduce((n, s) => n + s.halves, 0) / next.length : null;
    void client.cancelQueries({ queryKey: SHOW_RATINGS(series) }, { revert: false });
    client.setQueryData(SHOW_RATINGS(series), { seasons: next, average });
    settle(`season-${series}-${season}`, client, () => commands.rateSeason(series, season, halves));
  };
  return { average: q.data?.average ?? null, of, set };
}

/** How many titles this profile has rated. */
export function useRatedCount() {
  return useQuery({ queryKey: RATED_COUNT, queryFn: () => ok(commands.ratedCount()) });
}
