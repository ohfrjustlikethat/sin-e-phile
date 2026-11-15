import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type WatchedScope } from "@/lib/ipc";
import { HOME } from "@/features/home/HomeScreen";
import { WATCHLIST } from "@/features/watchlist/WatchlistScreen";

/**
 * Marking things watched by hand (SPEC.md Appendix F, I21): a film, an episode, a season,
 * a whole show. Watched is one completed viewing, as an import records it; not watched is
 * as if it was never played. Either way the place it was left at goes, so it leaves
 * Continue watching.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/** Mark a scope watched or not; every view that shows watched state is asked again. */
export function useSetWatched(onDone?: () => void) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (v: { scope: WatchedScope; watched: boolean }) => ok(commands.setWatched(v.scope, v.watched)),
    onSuccess: () => {
      for (const key of [["watched"], ["series"], ["season"], HOME, WATCHLIST]) {
        void client.invalidateQueries({ queryKey: key });
      }
      onDone?.();
    },
  });
}

const quiet = "label self-start rounded-sm py-1 text-ink-muted hover:text-ink disabled:opacity-50";

/** A film's or an episode's own mark. `onChange` lets the page drop a stale "Resume from". */
export function WatchedToggle({ id, onChange }: { id: number; onChange?: () => void }) {
  const watched = useQuery({ queryKey: ["watched", id], queryFn: () => ok(commands.titleWatched(id)) });
  const set = useSetWatched(onChange);
  if (watched.data === undefined) return null;
  const seen = watched.data;
  return (
    <button
      type="button"
      className={quiet}
      disabled={set.isPending}
      aria-pressed={seen}
      onClick={() => set.mutate({ scope: { kind: "title", id }, watched: !seen })}
    >
      {seen ? "Watched · mark as not watched" : "Mark as watched"}
    </button>
  );
}

/** Save a film or a show for later, or take it off the list (Appendix F, I39). */
export function WatchlistToggle({ id }: { id: number }) {
  const client = useQueryClient();
  const saved = useQuery({ queryKey: ["watchlisted", id], queryFn: () => ok(commands.titleWatchlisted(id)) });
  const set = useMutation({
    mutationFn: (on: boolean) => ok(commands.setWatchlisted(id, on)),
    onSuccess: () => {
      for (const key of [["watchlisted", id], WATCHLIST, HOME]) {
        void client.invalidateQueries({ queryKey: key });
      }
    },
  });
  if (saved.data === undefined) return null;
  const on = saved.data;
  return (
    <button type="button" className={quiet} disabled={set.isPending} aria-pressed={on} onClick={() => set.mutate(!on)}>
      {on ? "On your watchlist · remove" : "Add to watchlist"}
    </button>
  );
}

/** A season's and a show's marks, under the season chooser. */
export function SeriesMarks({
  series,
  season,
  seasonSeen,
  seasonTotal,
  showSeen,
  showTotal,
}: {
  series: number;
  season: number | null;
  seasonSeen: number;
  seasonTotal: number;
  showSeen: number;
  showTotal: number;
}) {
  const set = useSetWatched();
  if (showTotal === 0) return null;
  const seasonDone = seasonTotal > 0 && seasonSeen === seasonTotal;
  const showDone = showSeen === showTotal;
  return (
    <div className="mb-4 flex flex-wrap items-baseline gap-x-6 gap-y-1" aria-label="Watched">
      <span className="label text-ink-faint">
        Seen {showSeen} of {showTotal}
      </span>
      {seasonTotal > 0 && (
        <button
          type="button"
          className={quiet}
          disabled={set.isPending}
          onClick={() => set.mutate({ scope: { kind: "season", series, season }, watched: !seasonDone })}
        >
          {seasonDone ? "Mark this season as not watched" : "Mark this season as watched"}
        </button>
      )}
      <button
        type="button"
        className={quiet}
        disabled={set.isPending}
        onClick={() => set.mutate({ scope: { kind: "series", id: series }, watched: !showDone })}
      >
        {showDone ? "Mark the whole show as not watched" : "Mark the whole show as watched"}
      </button>
    </div>
  );
}
