import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, EmptyState, Skeleton } from "@/design-system";
import { commands, type PlayPlanView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { clock } from "@/features/player/intents";
import { languageName, qualityLabel, usePlaybackChoice } from "@/lib/playbackChoice";
import { ChoiceRow, type Choice } from "./ChoiceRow";
import { SourcePanel } from "./SourcePanel";
import { DownloadAdvice } from "./DownloadAdvice";
import { SeriesSection } from "./SeriesSection";

/**
 * The minimal detail page — SPEC.md Phase 8: title, synopsis, cast, and a Play button
 * that goes through the resolver.
 *
 * Designed for the artwork-free default (ADR-0013, CLAUDE.md): typography carries it.
 * It asks the sources **as soon as it opens** (`prepare_play`), so by the time Play is
 * pressed the source is chosen and a torrent is already finding peers (Phase 7 E1).
 *
 * Under Play: the audio languages and qualities the copies actually carry (SPEC.md
 * Phase 9). Choosing one plans again, and if no copy has it the page says so.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

type PlanState =
  | { phase: "finding" }
  | { phase: "ready"; plan: PlayPlanView }
  | { phase: "none"; reason: string };

/** What the copies offer, kept while a new choice is being planned. */
type Offers = Pick<PlayPlanView, "languages" | "qualities" | "original_language">;

function languageChoices(o: Offers, chosen: string | null): Choice[] {
  const codes = chosen && !o.languages.includes(chosen) ? [...o.languages, chosen] : o.languages;
  const role = (code: string) =>
    o.original_language ? (code === o.original_language ? " · original" : " · dub") : "";
  return [{ value: null, label: "Auto" }, ...codes.map((c) => ({ value: c, label: languageName(c) + role(c) }))];
}

function qualityChoices(o: Offers, chosen: string | null): Choice[] {
  const codes = chosen && !o.qualities.includes(chosen) ? [...o.qualities, chosen] : o.qualities;
  return [{ value: null, label: "Best" }, ...codes.map((c) => ({ value: c, label: qualityLabel(c) }))];
}

/** In words, when the copy that will play is not what was chosen. */
function unmetText(plan: PlayPlanView, audio: string | null, quality: string | null): string | null {
  if (!plan.unmet) return null;
  if (audio && !plan.languages.includes(audio)) return `None of your sources has ${languageName(audio)} audio.`;
  if (quality && !plan.qualities.includes(quality)) return `None of your sources has this in ${qualityLabel(quality)}.`;
  return `The copy that will play: ${plan.unmet}.`;
}

export function DetailScreen() {
  const id = useUi((s) => s.detail);
  const closeDetail = useUi((s) => s.closeDetail);
  const play = useUi((s) => s.play);
  const [plan, setPlan] = useState<PlanState>({ phase: "finding" });
  const [offers, setOffers] = useState<Offers | null>(null);
  const { audio, quality, setAudio, setQuality, details } = usePlaybackChoice();
  // Ctrl+Shift+S shows or hides the expert panel on this page; the setting keeps it on.
  const [panel, setPanel] = useState<boolean | null>(null);
  const showPanel = panel ?? details;

  const item = useQuery({
    queryKey: ["item", id],
    queryFn: () => ok(commands.itemDetail(id ?? 0)),
    enabled: id !== null,
  });

  // A series is not played itself; its episodes are (SeriesSection). Its sources are not
  // asked, so the plan waits until the kind is known — one local query.
  const kind = item.data?.kind;
  const isSeries = kind === "series" || kind === "anime_series";

  useEffect(() => {
    if (id === null || kind === undefined || isSeries) return;
    let live = true;
    setPlan({ phase: "finding" });
    void commands.preparePlay(id, audio, quality).then((r) => {
      if (!live) return;
      setPlan(r.status === "ok" ? { phase: "ready", plan: r.data } : { phase: "none", reason: r.error });
      if (r.status === "ok") setOffers(r.data);
    });
    return () => {
      live = false;
    };
  }, [id, audio, quality, kind, isSeries]);

  // A different film offers different things.
  useEffect(() => setOffers(null), [id]);

  // Once there is something to play, Play takes focus unless something else has it —
  // so a controller or keyboard user can press it at once (ADR-0041).
  useEffect(() => {
    if (plan.phase !== "ready") return;
    const active = document.activeElement;
    if (!active || active === document.body) document.getElementById("detail-play")?.focus();
  }, [plan.phase, item.data]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Backspace") closeDetail();
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        setPanel((p) => !(p ?? usePlaybackChoice.getState().details));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closeDetail]);

  if (id === null) return null;
  if (item.isPending) {
    return (
      <div className="px-16 pt-14">
        <Skeleton className="h-14 w-2/3" />
      </div>
    );
  }
  const d = item.data;
  if (!d) {
    return <EmptyState title="Not found" body="This item is no longer in the catalogue." />;
  }

  const makers = d.credits.filter((c) => c.role === "director" || c.role === "creator");
  const cast = d.credits.filter((c) => c.role === "actor" || c.role === "actress" || c.role === "self");
  const byline = [
    makers.map((m) => m.name).join(", ") || null,
    d.year,
    d.runtime_minutes ? `${d.runtime_minutes} min` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  const start = () => {
    if (plan.phase !== "ready") return;
    play({ target: plan.plan.open, title: d.title, mediaItemId: d.id, start: plan.plan.start });
  };

  return (
    <article className="max-w-[1100px] px-16 pt-12 pb-24">
      <button type="button" onClick={closeDetail} className="label mb-10 hover:text-ink">
        ← Back
      </button>
      <h1 className="font-serif text-[64px] leading-[0.98] tracking-[-0.015em] text-ink">{d.title}</h1>
      {byline && <p className="label mt-5">{byline}</p>}

      {isSeries ? (
        <SeriesSection id={d.id} />
      ) : (
      <>
      <div className="mt-10 flex items-center gap-5">
        <Button
          id="detail-play"
          variant="primary"
          size="lg"
          onClick={start}
          disabled={plan.phase !== "ready"}
          aria-describedby="play-status"
        >
          {plan.phase === "ready" && plan.plan.start ? `Resume from ${clock(plan.plan.start)}` : "Play"}
        </Button>
        <p id="play-status" className="text-sm text-ink-muted" role="status">
          {plan.phase === "finding" && "Asking your sources…"}
          {plan.phase === "ready" && (unmetText(plan.plan, audio, quality) ?? `From ${plan.plan.from}`)}
          {plan.phase === "none" && plan.reason}
        </p>
      </div>
      {plan.phase === "ready" && plan.plan.why && (
        <p className="mt-3 text-[13px] text-ink-faint" aria-label="Why this source">
          {plan.plan.why}
        </p>
      )}
      {plan.phase === "ready" && <DownloadAdvice id={d.id} plan={plan.plan.open} />}

      {offers && (offers.languages.length > 0 || audio || offers.qualities.length > 1 || quality) && (
        <div className="mt-8 flex flex-col gap-3">
          {(offers.languages.length > 0 || audio) && (
            <ChoiceRow label="Audio" choices={languageChoices(offers, audio)} value={audio} onChange={setAudio} />
          )}
          {(offers.qualities.length > 1 || quality) && (
            <ChoiceRow label="Quality" choices={qualityChoices(offers, quality)} value={quality} onChange={setQuality} />
          )}
        </div>
      )}

      {showPanel && plan.phase === "ready" && <SourcePanel candidates={plan.plan.candidates} />}
      </>
      )}

      {d.synopsis && (
        <p className="mt-12 max-w-[68ch] text-[17px] leading-[1.65] text-ink-muted">{d.synopsis}</p>
      )}

      {cast.length > 0 && (
        <section className="mt-14" aria-label="Cast">
          <h2 className="label mb-4 border-t border-line-strong pt-3">Cast</h2>
          <ul className="grid grid-cols-2 gap-x-10 gap-y-2.5 text-[15px] md:grid-cols-3">
            {cast.map((c) => (
              <li key={`${c.name}-${c.character ?? ""}`}>
                <span className="text-ink">{c.name}</span>
                {c.character && <span className="block text-[13px] text-ink-faint">{c.character}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}
