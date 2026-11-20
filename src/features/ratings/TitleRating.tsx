import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { StarRating, StarsShown, starWords } from "@/design-system";
import { commands } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { useShowRatings, useTitleRating } from "./useRating";

/**
 * What a title's page shows of the viewer's own rating (ADR-0056, the author's answers): a film's row of stars on its
 * own line under the title and year, "Rate" beside it until rated; a show's average of its rated seasons beside the
 * byline; and a season's stars on its header. Plus "Remove from Continue watching" for what is in that row.
 */

/** A film's stars, under its title and year. */
export function RateRow({ id }: { id: number }) {
  const { halves, set } = useTitleRating(id);
  return (
    <div className="mt-6 flex items-center gap-4">
      <StarRating value={halves} onChange={set} />
      <span className="label">{halves ? starWords(halves) : "Rate"}</span>
    </div>
  );
}

/** "Your average ★★★★" for a show with rated seasons; nothing before one is rated. */
export function ShowAverage({ id }: { id: number }) {
  const { average } = useShowRatings(id);
  if (average === null) return null;
  const said = `Your average: ${(average / 2).toFixed(1).replace(/\.0$/, "")} stars`;
  return (
    <span className="label inline-flex items-center gap-2" aria-label={said}>
      your average <StarsShown halves={average} label={said} />
    </span>
  );
}

/** A season's header: its name, its stars, how many episodes. */
export function SeasonHeader({
  series,
  season,
  name,
  episodes,
}: {
  series: number;
  season: number;
  name: string;
  episodes: number;
}) {
  const { of, set } = useShowRatings(series);
  return (
    <div className="mb-2 flex items-center gap-5 border-t border-line-strong pt-3" aria-label={`${name}, your rating`}>
      <h3 className="label text-ink">{name}</h3>
      <StarRating size="sm" value={of(season)} onChange={(h) => set(season, h)} label={`Your rating of ${name}`} />
      {episodes > 0 && (
        <span className="label text-ink-faint">
          {episodes} {episodes === 1 ? "episode" : "episodes"}
        </span>
      )}
    </div>
  );
}

/** "Remove from Continue watching", while the title is there: hidden, its place kept, back once played again. */
export function ContinueRemoval({ id }: { id: number }) {
  const client = useQueryClient();
  const [removed, setRemoved] = useState(false);
  const there = useQuery({
    queryKey: ["in-continue", id],
    queryFn: async () => {
      const r = await commands.inContinueWatching(id);
      return r.status === "ok" ? r.data : false;
    },
  });
  if (removed) {
    return (
      <span className="label" role="status">
        Removed from Continue watching
      </span>
    );
  }
  if (!there.data) return null;
  const remove = async () => {
    const r = await commands.removeFromContinue(id);
    if (r.status === "error") {
      toast(`Not removed: ${r.error}`, "danger");
      return;
    }
    setRemoved(true);
    void client.invalidateQueries({ queryKey: ["home"] });
    void client.invalidateQueries({ queryKey: ["continue-watching"] });
  };
  return (
    <button type="button" className="label hover:text-ink" onClick={() => void remove()}>
      Remove from Continue watching
    </button>
  );
}
