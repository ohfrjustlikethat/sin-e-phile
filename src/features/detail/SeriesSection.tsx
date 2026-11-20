import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/design-system";
import { commands, type ShelvedView, type UpNextView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { usePlaybackChoice } from "@/lib/playbackChoice";
import { shortPlace } from "@/features/player/place";
import { airsLine } from "./airing";
import { ChoiceRow } from "./ChoiceRow";
import { SeriesMarks } from "./Watched";
import { SeasonHeader } from "@/features/ratings/TitleRating";

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

/** The show, its seasons and the episode to resume: one key, one shape, for every part of its page. */
export function useSeries(id: number) {
  return useQuery({ queryKey: ["series", id], queryFn: () => ok(commands.seriesDetail(id)) });
}

/**
 * The show's page, ready to resume (Phase 16, the author's round 8): Continue watching opens the show, and this wide
 * button plays the episode they are on, or the one after the last they finished, from where it was left.
 */
export function UpNext({ id, show }: { id: number; show: string }) {
  const series = useSeries(id);
  const up = series.data?.up_next ?? null;
  // Once it is there, it takes focus unless something else has it, as Play does on a film's page (ADR-0041).
  useEffect(() => {
    if (!up) return;
    const active = document.activeElement;
    if (!active || active === document.body) document.getElementById("series-resume")?.focus();
  }, [up]);
  if (!up) return null;
  return <ResumeEpisode up={up} show={show} />;
}

/** Under Resume: the episode's name when it has one; otherwise what the button cannot say (seen on Twin Peaks, whose
 * episodes the catalogue leaves unnamed: "S1 E2" under "Resume S1 E2"). */
function said(up: UpNextView, place: string): string {
  const line = up.line.replace(/^Next: /, "");
  if (line && line !== place) return up.line;
  return up.progress !== null ? "From where you left it" : "The episode after the last one you finished";
}

function ResumeEpisode({ up, show }: { up: UpNextView; show: string }) {
  const play = useUi((s) => s.play);
  const { audio, quality, preferred } = usePlaybackChoice();
  const [status, setStatus] = useState<{ finding: boolean; error: string | null }>({ finding: false, error: null });
  const place = shortPlace(up.season, up.episode);
  const resume = up.progress !== null;
  const go = async () => {
    setStatus({ finding: true, error: null });
    const r = await commands.preparePlay(up.id, audio, quality, preferred);
    if (r.status !== "ok") {
      setStatus({ finding: false, error: r.error });
      return;
    }
    setStatus({ finding: false, error: null });
    play({ target: r.data.open, title: [show, place].filter(Boolean).join(" · "), mediaItemId: up.id, start: r.data.start });
  };
  return (
    <div className="mt-10 max-w-[560px]">
      <Button
        id="series-resume"
        variant="primary"
        size="lg"
        className="h-16 w-full text-[17px]"
        onClick={() => void go()}
        disabled={status.finding}
        aria-describedby="series-resume-line"
      >
        {resume ? `Resume ${place}` : `Play ${place}`.trim()}
      </Button>
      {up.progress !== null && (
        <div className="mt-2 h-[3px] w-full rounded-full bg-line" aria-hidden>
          <div className="h-full rounded-full bg-oxblood-bright" style={{ width: `${Math.min(1, up.progress) * 100}%` }} />
        </div>
      )}
      <p id="series-resume-line" className="mt-3 text-sm text-ink-muted" role="status">
        {status.finding ? "Asking your sources…" : (status.error ?? said(up, place))}
      </p>
    </div>
  );
}

export function SeriesSection({ id }: { id: number }) {
  const openDetail = useUi((s) => s.openDetail);
  const play = useUi((s) => s.play);
  const series = useSeries(id);
  const seasons = series.data?.seasons ?? [];
  // The first real season, not the specials.
  const first = seasons.find((s) => (s.number ?? 0) >= 1) ?? seasons[0];
  const [season, setSeason] = useState<number | null | undefined>(undefined);
  useEffect(() => setSeason(undefined), [id]);
  // The season of the episode to resume, when there is one (Phase 16); else the first real season.
  const up = series.data?.up_next;
  const chosen = season === undefined ? (up ? up.season : (first?.number ?? null)) : season;
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
      {/* The season's own header, with its stars (ADR-0056: a show is rated season by season). */}
      {chosen !== null && seasons.length > 0 && (
        <SeasonHeader
          series={id}
          season={chosen}
          name={seasonLabel(chosen)}
          episodes={episodes.data?.length ?? 0}
        />
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
              {up?.id === e.id && <span className="label text-ink">{up.progress !== null ? "Resume" : "Up next"}</span>}
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
