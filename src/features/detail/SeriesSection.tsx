import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { airsLine } from "./airing";
import { ChoiceRow } from "./ChoiceRow";

/**
 * A series' seasons and episodes — SPEC.md Phase 11.
 *
 * The airing line reads AniList's schedule, so "Episode 7 airs Thursday" shows before the
 * catalogue has an episode 7 (ADR-0030). Each episode opens its own page, where Play asks
 * the sources for the series, season and number.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const seasonLabel = (n: number | null) => (n === null ? "Other" : n === 0 ? "Specials" : `Season ${n}`);

export function SeriesSection({ id }: { id: number }) {
  const openDetail = useUi((s) => s.openDetail);
  const series = useQuery({ queryKey: ["series", id], queryFn: () => ok(commands.seriesDetail(id)) });
  const seasons = series.data?.seasons ?? [];
  // The first real season, not the specials.
  const first = seasons.find((s) => (s.number ?? 0) >= 1) ?? seasons[0];
  const [season, setSeason] = useState<number | null | undefined>(undefined);
  useEffect(() => setSeason(undefined), [id]);
  const chosen = season === undefined ? (first?.number ?? null) : season;
  const episodes = useQuery({
    queryKey: ["season", id, chosen],
    queryFn: () => ok(commands.seasonEpisodes(id, chosen)),
    enabled: first !== undefined,
  });

  const airing = series.data?.next_airing;
  return (
    <section aria-label="Episodes" className="mt-12">
      {airing && (
        <p className="mb-5 text-[15px] text-ink" aria-label="Next episode">
          {airsLine(airing.episode, airing.airs_at ?? 0, new Date())}
        </p>
      )}
      {seasons.length > 1 && (
        <div className="mb-5">
          <ChoiceRow
            label="Season"
            choices={seasons.map((s) => ({ value: s.number === null ? "other" : String(s.number), label: seasonLabel(s.number) }))}
            value={chosen === null ? "other" : String(chosen)}
            onChange={(v) => setSeason(v === "other" || v === null ? null : Number(v))}
          />
        </div>
      )}
      {seasons.length === 0 && !series.isPending && (
        <p className="text-sm text-ink-muted">The catalogue lists no episodes for this series yet.</p>
      )}
      <ol className="flex flex-col">
        {(episodes.data ?? []).map((e) => (
          <li key={e.id}>
            <button
              type="button"
              onClick={() => openDetail(e.id)}
              className="flex w-full items-baseline gap-5 border-t border-line-subtle py-3 text-left hover:bg-raised"
            >
              <span className="w-8 shrink-0 text-right tabular-nums text-ink-faint">{e.episode ?? "·"}</span>
              <span className="flex-1 text-[15px] text-ink">{e.title}</span>
              {e.air_date && <span className="label text-ink-faint">{e.air_date}</span>}
            </button>
          </li>
        ))}
      </ol>
    </section>
  );
}
