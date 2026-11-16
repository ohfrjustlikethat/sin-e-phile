import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type ShelvedView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { airsLine } from "./airing";
import { ChoiceRow } from "./ChoiceRow";
import { SeriesMarks } from "./Watched";

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

/** "S1 · E14", or as much of it as the file's name gave. */
export const shelfNumber = (f: ShelvedView) =>
  [f.season !== null ? `S${f.season}` : null, f.episode !== null ? `E${f.episode}` : null].filter(Boolean).join(" · ");

/** The file's own name, without its folders or extension: the fallback title. */
const fileName = (path: string) => (path.split(/[\\/]/).pop() ?? path).replace(/\.[^.]+$/, "");

export function SeriesSection({ id }: { id: number }) {
  const openDetail = useUi((s) => s.openDetail);
  const play = useUi((s) => s.play);
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
      {series.data && (
        <SeriesMarks
          series={id}
          season={chosen}
          seasonSeen={(episodes.data ?? []).filter((e) => e.watched).length}
          seasonTotal={episodes.data?.length ?? 0}
          showSeen={series.data.watched}
          showTotal={seasons.reduce((n, s) => n + s.episodes, 0)}
        />
      )}
      {series.data && series.data.on_this_pc + series.data.also_here.length > 0 && (
        <p className="mb-4 text-[13px] text-ink-muted" aria-label="On this PC">
          {plural(series.data.on_this_pc + series.data.also_here.length, "episode", "episodes")} on this PC
        </p>
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
              <span className={`flex-1 text-[15px] ${e.watched ? "text-ink-muted" : "text-ink"}`}>{e.title}</span>
              {e.on_this_pc && <span className="label text-ink-muted">On this PC</span>}
              {e.watched && <span className="label text-ink-faint">Watched</span>}
              {e.air_date && <span className="label text-ink-faint">{e.air_date}</span>}
            </button>
          </li>
        ))}
      </ol>
      {(series.data?.also_here.length ?? 0) > 0 && (
        // Files of this show the catalogue has no episode for (a season it lists as another
        // series, a numbering past its season), in the order their names give (Phase 16).
        <section aria-label="Also on this PC" className="mt-10">
          <h3 className="label mb-2 text-ink">Also on this PC</h3>
          <ol className="flex flex-col">
            {series.data?.also_here.map((f) => (
              <li key={f.path}>
                <button
                  type="button"
                  onClick={() => play({ target: f.path, title: f.title ?? fileName(f.path), mediaItemId: null, start: null })}
                  className="flex w-full items-baseline gap-5 border-t border-line-subtle py-3 text-left hover:bg-raised"
                >
                  <span className="w-20 shrink-0 text-right tabular-nums text-ink-faint">{shelfNumber(f) || "·"}</span>
                  <span className="flex-1 text-[15px] text-ink">{f.title ?? fileName(f.path)}</span>
                  <span className="label text-ink-muted">Play</span>
                </button>
              </li>
            ))}
          </ol>
        </section>
      )}
    </section>
  );
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
