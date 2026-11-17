import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, EmptyState, Skeleton, TypographicCard, type PosterFilm } from "@/design-system";
import { commands, type CandidateView, type PlayPlanView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { openSettingsAt } from "@/lib/openSettings";
import { clock } from "@/features/player/intents";
import { languageName, qualityLabel, usePlaybackChoice } from "@/lib/playbackChoice";
import { type Choice } from "./ChoiceRow";
import { SourcePanel } from "./SourcePanel";
import { SeriesSection, UpNext } from "./SeriesSection";
import { useSetWatched } from "./Watched";
import { ICONS, RoundAction } from "./RoundAction";
import { StreamsPanel } from "./StreamsPanel";
import { DOWNLOADS, useDownloads } from "@/features/downloads/useDownloads";
import { WATCHLIST } from "@/features/watchlist/WatchlistScreen";
import { HOME } from "@/features/home/HomeScreen";
import { useArtworkOn } from "@/features/home/TitleCard";

/**
 * The minimal detail page — SPEC.md Phase 8: title, synopsis, cast, and a Play button
 * that goes through the resolver.
 *
 * Designed for the artwork-free default (ADR-0013, CLAUDE.md): typography carries it.
 * It asks the sources **as soon as it opens** (`prepare_play`), so by the time Play is
 * pressed the source is chosen and a torrent is already finding peers (Phase 7 E1).
 *
 * Phase 16 (the author's answers): one wide Play, and under it big round buttons with a word under each:
 * Download (the best stream), Watchlist and Watched (filled when on), and Streams, which slides every stream in
 * from the right to play or download exactly one. Play and the round Download take the best stream: the
 * healthiest at the preferred quality (Settings → Playback), then the nearest healthy one. The "stream or
 * download" advice is gone. The audio and quality choices (SPEC.md Phase 9) are at the top of Streams.
 *
 * Round 9 (the author): a wide still faded behind the top of the page, and the poster on the right, from the
 * poster sources the person chose (ADR-0048, ADR-0052). Without a poster, the lettered card the rails use.
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
  const play = useUi((s) => s.play);
  // No source can play this: Settings, at the place to add one, with focus there for a pad.
  const addSource = () => openSettingsAt("Add a source");
  const [plan, setPlan] = useState<PlanState>({ phase: "finding" });
  const [offers, setOffers] = useState<Offers | null>(null);
  const { audio, quality, preferred, setAudio, setQuality, details } = usePlaybackChoice();
  const [streams, setStreams] = useState(false);
  // The last list of streams: the panel keeps showing it while a new choice re-plans, rather than
  // vanishing under the D-pad (found by the pad audit).
  const [listed, setListed] = useState<PlayPlanView["candidates"]>([]);
  const streamsButton = useRef<HTMLButtonElement>(null);
  // Ctrl+Shift+S shows or hides the expert panel on this page; the setting keeps it on.
  const [panel, setPanel] = useState<boolean | null>(null);
  // The stream being resolved for Play or Download from the panel.
  const [busy, setBusy] = useState<number | null>(null);
  const client = useQueryClient();
  const getOne = useMutation({
    mutationFn: async (c: CandidateView) => {
      setBusy(c.index);
      try {
        return await ok(commands.downloadTitle(id ?? 0, false, null, c.index));
      } finally {
        setBusy(null);
      }
    },
    onSuccess: () => void client.invalidateQueries({ queryKey: DOWNLOADS }),
  });
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
    void commands.preparePlay(id, audio, quality, preferred).then((r) => {
      if (!live) return;
      setPlan(r.status === "ok" ? { phase: "ready", plan: r.data } : { phase: "none", reason: r.error });
      if (r.status === "ok") {
        setOffers(r.data);
        setListed(r.data.candidates);
      }
    });
    return () => {
      live = false;
    };
  }, [id, audio, quality, preferred, kind, isSeries]);

  // A different film offers different things.
  useEffect(() => {
    setOffers(null);
    setListed([]);
    setStreams(false);
  }, [id]);

  // Once there is something to play, Play takes focus unless something else has it —
  // so a controller or keyboard user can press it at once (ADR-0041).
  useEffect(() => {
    if (plan.phase !== "ready") return;
    const active = document.activeElement;
    if (!active || active === document.body) document.getElementById("detail-play")?.focus();
  }, [plan.phase, item.data]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Escape and Backspace are the app's one way back (App.tsx, `back` in src/lib/store.ts).
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        setPanel((p) => !(p ?? usePlaybackChoice.getState().details));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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

  /** One stream from the panel: resolved (a torrent starts), then played from where it was left. */
  const playOne = async (c: CandidateView) => {
    setBusy(c.index);
    const r = await commands.playCandidate(d.id, c.index);
    setBusy(null);
    if (r.status !== "ok") {
      setPlan({ phase: "none", reason: r.error });
      return;
    }
    setStreams(false);
    play({ target: r.data, title: d.title, mediaItemId: d.id, start: plan.phase === "ready" ? plan.plan.start : null });
  };

  /** Play from where it was left, or (Appendix F, I01) from the beginning. */
  const start = (over = false) => {
    if (plan.phase !== "ready") return;
    play({ target: plan.plan.open, title: d.title, mediaItemId: d.id, start: over ? null : plan.plan.start });
  };

  const film: Omit<PosterFilm, "artwork"> = {
    id: String(d.id),
    title: d.title,
    year: d.year ?? 0,
    director: makers.map((m) => m.name).join(", "),
  };

  return (
    <article className="relative">
      <Backdrop id={d.id} />
      <div className="relative flex items-start gap-14 px-16 pt-6 pb-24">
      <div className="min-w-0 max-w-[1100px] flex-1">
      <h1 className="font-serif text-[64px] leading-[0.98] tracking-[-0.015em] text-ink">{d.title}</h1>
      {byline && <p className="label mt-5">{byline}</p>}

      {isSeries ? (
        <>
          <UpNext id={d.id} show={d.title} />
          <div className="mt-8 flex">
            <WatchlistAction id={d.id} />
          </div>
          <SeriesSection id={d.id} />
        </>
      ) : (
      <>
      <div className="mt-10 max-w-[560px]">
        <Button
          id="detail-play"
          // Down goes to the first round button, not the one under Play's middle (the nearest box).
          data-pad-down="detail-download"
          variant="primary"
          size="lg"
          className="h-16 w-full text-[17px]"
          onClick={() => start()}
          disabled={plan.phase !== "ready"}
          aria-describedby="play-status"
        >
          {plan.phase === "ready" && plan.plan.start ? `Resume from ${clock(plan.plan.start)}` : "Play"}
        </Button>
        <div className="mt-3 flex items-baseline gap-5">
          <p id="play-status" className="flex-1 text-sm text-ink-muted" role="status">
            {plan.phase === "finding" && "Asking your sources…"}
            {plan.phase === "ready" && (unmetText(plan.plan, audio, quality) ?? `From ${plan.plan.from}`)}
            {plan.phase === "none" && plan.reason}
          </p>
          {plan.phase === "ready" && plan.plan.start ? (
            <button type="button" className="label hover:text-ink" onClick={() => start(true)}>
              Start over
            </button>
          ) : null}
        </div>
        {plan.phase === "ready" && plan.plan.why && (
          <p className="mt-1 text-[13px] text-ink-faint" aria-label="Why this source">
            {plan.plan.why}
          </p>
        )}
      </div>
      {plan.phase === "none" && (
        // Phase 16: the author could not download or stream anything and nothing said why.
        <div className="mt-5 flex max-w-[62ch] flex-col items-start gap-3">
          <p className="text-[14px] text-ink-muted">
            Nothing you have set up can play or download this yet. sin·e·phile comes with no sources of its own:
            add one you have chosen, by its address, and films like this one can play.
          </p>
          <Button variant="secondary" onClick={addSource}>
            Add a source
          </Button>
        </div>
      )}

      <TitleActions
        id={d.id}
        plan={plan.phase === "ready" ? plan.plan : null}
        onStreams={() => setStreams(true)}
        streamsButton={streamsButton}
        onWatchedChange={() =>
          // Marked either way, the place it was left at is gone: "Resume from" would lie.
          setPlan((p) => (p.phase === "ready" ? { phase: "ready", plan: { ...p.plan, start: null } } : p))
        }
      />

      {streams && listed.length > 0 && (
        <StreamsPanel
          title={d.title}
          candidates={listed}
          audioChoices={offers ? languageChoices(offers, audio) : []}
          qualityChoices={offers ? qualityChoices(offers, quality) : []}
          audio={audio}
          quality={quality}
          onAudio={setAudio}
          onQuality={setQuality}
          busy={busy}
          onPlay={(c) => void playOne(c)}
          onDownload={(c) => getOne.mutate(c)}
          onClose={() => {
            setStreams(false);
            streamsButton.current?.focus();
          }}
        />
      )}
      {getOne.isError && (
        <p className="mt-3 text-sm text-danger" role="alert">
          {getOne.error.message}
        </p>
      )}
      {getOne.isSuccess && (
        <p className="mt-3 text-sm text-ink-muted" role="status">
          Downloading that stream. It is on the Downloads page.
        </p>
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
      </div>
      <TitlePoster id={d.id} film={film} />
      </div>
    </article>
  );
}

/** One of a title's images from the person's chosen sources, or null: none chosen, or none found. */
function useTitleImage(kind: "poster" | "backdrop", id: number, on: boolean) {
  return useQuery({
    // The same key and shape as the cards' poster (TitleCard), so a poster seen in a rail is not fetched again.
    queryKey: [kind, id],
    queryFn: async () => {
      const r = await (kind === "poster" ? commands.poster(id) : commands.backdrop(id));
      return r.status === "ok" ? r.data : null;
    },
    enabled: on,
    staleTime: Infinity,
  });
}

/**
 * The wide still behind the top of the page, faded into it (round 9, ADR-0052). Two scrims keep the text on it
 * legible, as on a card's artwork: §9.0 bans shadow for depth, not a scrim for legibility. Nothing without one.
 */
function Backdrop({ id }: { id: number }) {
  const on = useArtworkOn();
  const still = useTitleImage("backdrop", id, on);
  if (!on || !still.data) return null;
  return (
    <div
      aria-hidden
      data-backdrop
      className="pointer-events-none absolute inset-x-0 top-0 h-[640px] overflow-hidden"
      // Its top edge fades in too: a hard line under the back arrow read as a band laid over the page.
      style={{ maskImage: "linear-gradient(to bottom, transparent, black 160px)" }}
    >
      <img src={still.data} alt="" className="h-full w-full object-cover object-top opacity-45" />
      <span className="absolute inset-0 bg-gradient-to-b from-base/10 via-base/55 to-base" />
      <span className="absolute inset-0 bg-gradient-to-r from-base/90 via-base/45 to-transparent" />
    </div>
  );
}

/** The poster on the right of the page (round 9); the lettered card when there is none. Hidden on narrow windows. */
function TitlePoster({ id, film }: { id: number; film: Omit<PosterFilm, "artwork"> }) {
  const on = useArtworkOn();
  const poster = useTitleImage("poster", id, on);
  return (
    // Decorative: the heading names the title, and a focus stop that does nothing would be in the D-pad's way.
    <aside aria-hidden data-title-poster className="hidden w-[300px] shrink-0 pt-2 lg:block">
      <div className="aspect-[2/3] overflow-hidden rounded-sm border border-line-subtle bg-surface">
        {on && poster.data ? (
          <img src={poster.data} alt="" className="h-full w-full object-cover" />
        ) : (
          <TypographicCard film={film} size="lead" />
        )}
      </div>
    </aside>
  );
}

/** Download, Watchlist, Watched and Streams, as big round buttons (Phase 16). */
function TitleActions({
  id,
  plan,
  onStreams,
  streamsButton,
  onWatchedChange,
}: {
  id: number;
  plan: PlayPlanView | null;
  onStreams: () => void;
  streamsButton: React.RefObject<HTMLButtonElement | null>;
  onWatchedChange: () => void;
}) {
  const client = useQueryClient();
  const setDownloadsOpen = useUi((s) => s.setDownloadsOpen);
  const downloads = useDownloads();
  const mine = (downloads.data ?? []).find((r) => r.download.media_item_id === id && r.download.state !== "cancelled");
  const get = useMutation({
    mutationFn: () => ok(commands.downloadTitle(id, false, null, null)),
    onSuccess: () => void client.invalidateQueries({ queryKey: DOWNLOADS }),
  });
  const watched = useQuery({ queryKey: ["watched", id], queryFn: () => ok(commands.titleWatched(id)) });
  const setWatched = useSetWatched(onWatchedChange);
  const seen = watched.data === true;

  const d = mine?.download;
  const percent = d?.total_bytes ? Math.floor(((d.done_bytes ?? 0) / d.total_bytes) * 100) : null;
  const downloadLabel = !d
    ? "Download"
    : d.state === "done"
      ? "Downloaded"
      : d.state === "paused"
        ? "Download paused"
        : percent !== null
          ? `Downloading ${percent}%`
          : "Downloading";
  return (
    <div className="mt-8 flex flex-wrap gap-6" aria-label="Actions">
      <RoundAction
        id="detail-download"
        label={downloadLabel}
        icon={ICONS.download}
        on={d?.state === "done"}
        // Already queued: the button goes to Downloads, where it can be watched or paused.
        onClick={() => (d ? setDownloadsOpen(true) : get.mutate())}
        disabled={!d && (plan === null || get.isPending)}
      />
      <WatchlistAction id={id} />
      <RoundAction
        label={seen ? "Watched" : "Mark watched"}
        icon={ICONS.watched}
        on={seen}
        pressed={seen}
        disabled={watched.data === undefined || setWatched.isPending}
        onClick={() => setWatched.mutate({ scope: { kind: "title", id }, watched: !seen })}
      />
      <RoundAction
        ref={streamsButton}
        label={plan ? `Streams · ${plan.candidates.length}` : "Streams"}
        icon={ICONS.streams}
        disabled={plan === null || plan.candidates.length === 0}
        aria-haspopup="dialog"
        onClick={onStreams}
      />
      {get.isError && (
        <p className="basis-full text-sm text-danger" role="alert">
          {get.error.message}
        </p>
      )}
    </div>
  );
}

/** Save a film or a show for later, or take it off the list (Appendix F, I39), as a round button. */
function WatchlistAction({ id }: { id: number }) {
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
  const on = saved.data === true;
  return (
    <RoundAction
      label={on ? "On watchlist" : "Watchlist"}
      icon={ICONS.watchlist(on)}
      on={on}
      pressed={on}
      disabled={saved.data === undefined || set.isPending}
      onClick={() => set.mutate(!on)}
    />
  );
}
