import { useEffect, useRef, useState } from "react";
import { commands, type SearchHit, type SearchResponse, type Why } from "@/lib/ipc";
import { Skeleton, EmptyState } from "@/design-system";
import { TitleCard } from "@/features/home/TitleCard";
import { moveFocus, type Direction } from "@/lib/gamepad";
import { MeaningDownload } from "./MeaningDownload";
import { useUi } from "@/lib/store";

/**
 * Search — `SPEC.md` §15 Phase 5: "results as you type, grouped by kind, keyboard
 * navigable, with a 'why this matched' hint".
 *
 * # What the debounce is protecting
 *
 * E1 budgets 80 ms from keystroke to results, and the engine measures p95 31.3 ms —
 * but that is per QUERY, and typing "kurosawa" is eight of them. The debounce is what
 * makes "as you type" mean "as you pause", which is what a person actually perceives
 * as instant. 120 ms is below the ~150 ms at which a delay starts to feel like lag.
 *
 * # Why a stale response can never overwrite a fresh one
 *
 * Searches are async and do not come back in order: "kur" can outrun "kurosawa" when
 * the shorter query matches more rows. Each request carries a sequence number and
 * anything older than what is already displayed is dropped — otherwise the results
 * visibly flick backwards as you type, which looks like a bug in the engine rather
 * than in the screen.
 *
 * # One grid (the author, round 9)
 *
 * Results were a ranked list grouped by kind, so a search showed films and nothing else until it was scrolled.
 * Now they are one poster grid, about eight across: films first, then series, anime and episodes in the same
 * grid, each marked with its kind, every result the same size. The cards are Home's, so posters, the lettered
 * card and focus memory come with them.
 *
 * "Films first" is among results that matched equally well, not over the whole page: sorted strictly, "peaks"
 * put the Twin Peaks series after forty films that only matched in meaning, off the screen again, which was
 * the complaint (seen in the real window on a copy of the author's catalogue).
 */
const DEBOUNCE_MS = 120;

/** Enough for six rows of eight. */
const LIMIT = 48;

/** The first result's button: where Down from the field goes, by key or by pad. */
const FIRST = "search-first-result";

/**
 * Why a result is here, said only where the title alone does not show it (SPEC.md §15 Phase 5's "why this
 * matched"). An exact title or matching words are visible on the card; a match in meaning or a near spelling is
 * not, and a person wonders why it is there.
 */
const WHY_NOTE: Partial<Record<Why, string>> = {
  semantic: "similar in meaning",
  both: "title and meaning",
  fuzzy: "close spelling",
  filter: "matched your filters",
};

/**
 * A person's work, when the query was their name (ADR-0057: "zendaya" found none of her films): what they did in
 * it, by name.
 */
export function personNote(why: Why, person: string | null): string | undefined {
  if (!person) return undefined;
  if (why === "starring") return `With ${person}`;
  if (why === "directed") return `Directed by ${person}`;
  if (why === "written") return `Written by ${person}`;
  return undefined;
}

/**
 * The order kinds appear in: films first because that is what most searches are for, then shows, then episodes.
 * An anime film is a film and an anime series a show: split, "anime about loneliness" put every live-action film
 * the engine ranked lower ahead of the anime films it ranked higher (seen in the release build).
 */
const KIND_GROUP: Record<string, number> = { film: 0, anime_film: 0, series: 1, anime_series: 1, episode: 2 };

/** The mark on a result that is not a film. */
const KIND_MARK: Record<string, string> = {
  anime_film: "Anime film",
  series: "Series",
  anime_series: "Anime series",
  episode: "Episode",
  live_channel: "Live channel",
};

/** How well a result matched, best first: its exact title, its words, a near spelling, its meaning. */
const TIER: Record<Why, number> = {
  exact_title: 0,
  // A person's work, ahead of words that merely matched: the engine put it there.
  starring: 1,
  directed: 1,
  written: 1,
  both: 2,
  keyword: 2,
  filter: 2,
  fuzzy: 3,
  semantic: 4,
};

/**
 * Films first among results that matched equally well; the engine's rank decides everything else (see above).
 * Within a tier and a kind the rank is kept, and an unknown kind goes last in its tier.
 */
export function filmsFirst(hits: SearchHit[]): SearchHit[] {
  const place = (kind: string) => KIND_GROUP[kind] ?? 3;
  return [...hits].sort((a, b) => TIER[a.why] - TIER[b.why] || place(a.kind) - place(b.kind));
}

const ARROWS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};

/** Display size for the query: large while it is a title, smaller as it becomes a sentence. */
function querySize(query: string): string {
  if (query.length <= 26) return "text-[46px]";
  if (query.length <= 44) return "text-[34px]";
  return "text-[26px]";
}

/** The line under the query: what kind of search just ran, in a word or two. */
function status(response: SearchResponse | null, searching: boolean): string {
  if (searching && !response) return "Searching";
  if (!response?.ready || response.hits.length === 0) return "";
  const count = `${response.hits.length} ${response.hits.length === 1 ? "result" : "results"}`;
  return `${count} · ${response.semantic ? "titles, people and meaning" : "titles and people only"}`;
}

export function SearchScreen() {
  const openDetail = useUi((s) => s.openDetail);
  // Opened from a search: a light signal for the taste model (Phase 15). Never in the way.
  const open = (id: number) => {
    void Promise.resolve()
      .then(() => commands.recordSignal(id, "search"))
      .catch(() => {});
    openDetail(id);
  };
  // Kept while away: search keeps its words and its trail like a tab (Phase 16).
  const [query, setQuery] = useState(() => useUi.getState().searchQuery);
  useEffect(() => useUi.getState().setSearchQuery(query), [query]);
  const [response, setResponse] = useState<SearchResponse | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // ADR-0030 layer 3: a miss is put to the sources AFTER it is on screen.
  const [asking, setAsking] = useState(false);
  const [found, setFound] = useState<{ title: string; year: number; found_by: string } | null>(
    null,
  );

  // The sequence number of the most recent response rendered. See the note above.
  const latest = useRef(0);
  const issued = useRef(0);

  useEffect(() => {
    const text = query.trim();
    setFound(null);
    setAsking(false);
    if (!text) {
      setResponse(null);
      setError(null);
      setSearching(false);
      return;
    }
    setSearching(true);
    const timer = setTimeout(() => {
      const sequence = ++issued.current;
      commands
        .search(text, LIMIT)
        .then((result) => {
          if (sequence < latest.current) return; // A slower, older query. Discard it.
          latest.current = sequence;
          // The generated binding is a tagged Result, so a failing search cannot be
          // mistaken for an empty one — which is the distinction this whole screen is
          // careful about everywhere else.
          if (result.status === "ok") {
            setResponse(result.data);
            setError(null);
            if (result.data.ready && result.data.hits.length === 0 && !result.data.catalogue_partial) {
              backfill(text, sequence);
            }
          } else {
            setResponse(null);
            setError(result.error);
          }
        })
        .finally(() => {
          if (sequence >= latest.current) setSearching(false);
        });
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query]);

  /**
   * The catalogue had nothing; ask the sources, and if one has the film it becomes a
   * catalogue entry (ADR-0030 layer 3) — then search again so it appears. Runs only
   * after the empty result is rendered, and is dropped if the query has moved on.
   */
  function backfill(text: string, sequence: number) {
    setAsking(true);
    void commands
      .searchBackfill(text)
      .then(async (result) => {
        if (sequence !== issued.current || result.status !== "ok" || !result.data) return;
        setFound(result.data);
        const again = await commands.search(text, LIMIT);
        if (sequence === issued.current && again.status === "ok") setResponse(again.data);
      })
      .finally(() => {
        if (sequence === issued.current) setAsking(false);
      });
  }

  const hits = filmsFirst(response?.hits ?? []);

  /** Down or Enter in the field goes to the first result; the arrows then move through the grid. */
  function onKeyDown(event: React.KeyboardEvent) {
    if ((event.key === "ArrowDown" || event.key === "Enter") && hits.length > 0) {
      event.preventDefault();
      document.getElementById(FIRST)?.focus();
    }
  }

  /** The arrow keys move through the grid as the D-pad does: to the nearest card that way, up to the field. */
  function onGridKeyDown(event: React.KeyboardEvent) {
    const direction = ARROWS[event.key];
    if (!direction) return;
    event.preventDefault();
    moveFocus(direction);
  }

  return (
    <div className="flex h-full flex-col px-16 pt-14 pb-24">
      {/*
        The query is set in the display face, large, on a single rule rather than in a
        box: what was asked is the headline of this page, and the results are the
        article under it (§9.0, the MUBI reference).
      */}
      <input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.currentTarget.value)}
        onKeyDown={onKeyDown}
        // The pad's Down (and Done on the on-screen keyboard) goes to the first result, not to the card under the
        // middle of this wide field.
        data-pad-down={FIRST}
        placeholder="A title, a director, or what it is about"
        aria-label="Search the catalogue"
        spellCheck={false}
        // The underline IS the focus indicator: it turns oxblood when focused, so the
        // global ring (a box) is replaced rather than removed. The size steps down as
        // the query grows, because a sentence-long query at 46px scrolls out of sight.
        className={`w-full max-w-[1100px] border-0 border-b-2 border-line-strong bg-transparent pb-4 font-display leading-none font-semibold tracking-[-0.035em] text-ink placeholder:text-ink-faint focus:border-oxblood-bright focus-visible:outline-none ${querySize(query)}`}
      />
      <p className="label mt-3 h-4" aria-live="polite">
        {status(response, searching)}
      </p>
      {/* At the top while nothing is typed (the author, round 8: under the results it went unseen); once there is a
          query it goes under the results, so Down from the field still reaches the first result (the pad audit). It
          hides itself once search by meaning works. */}
      {!query.trim() && (
        <div className="mt-6">
          <MeaningDownload />
        </div>
      )}

      <div className="mt-10">
        {searching && !response && (
          <div className="flex flex-col gap-3" aria-hidden>
            <Skeleton className="h-14 w-2/3" />
            <Skeleton className="h-8 w-1/2" />
            <Skeleton className="h-8 w-1/2" />
          </div>
        )}

        {error && (
          <EmptyState title="Search failed" body={`The engine returned an error: ${error}`} />
        )}

        {response && !response.ready && (
          <EmptyState
            title="Still starting up"
            body="The catalogue is opening. This takes a moment on the first search after launch."
          />
        )}

        {/*
          "Nothing found" is only ever said when it is TRUE. At 3% ingested it is a lie,
          and the backend sends `catalogue_partial` precisely so this screen cannot tell
          it — see the note on SearchResponse.
        */}
        {response?.ready && hits.length === 0 && !searching && (
          <EmptyState
            title={
              response.catalogue_partial
                ? "Still building the catalogue"
                : asking
                  ? "Not in the catalogue"
                  : "Nothing found"
            }
            body={
              response.catalogue_partial
                ? `${response.searchable_titles.toLocaleString()} titles are searchable so far, and more are still arriving. It may simply not be here yet.`
                : asking
                  ? "Asking your sources whether any of them has it…"
                  : "No title matches that, and none of your sources has it. Try fewer words, or describe what the film is about."
            }
          />
        )}

        {found && hits.length > 0 && (
          <p className="mb-8 text-sm text-ink-muted" role="status">
            Not in the catalogue until now: {found.found_by} has{" "}
            <span className="text-ink">
              {found.title} ({found.year})
            </span>
            , so it has been added.
          </p>
        )}

        {hits.length > 0 && (
          <ol
            aria-label="Results"
            onKeyDown={onGridKeyDown}
            className="grid grid-cols-3 gap-x-5 gap-y-8 sm:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8"
          >
            {hits.map((hit, i) => (
              <li key={hit.id}>
                <TitleCard
                  id={hit.id}
                  size="fill"
                  buttonId={i === 0 ? FIRST : undefined}
                  film={{
                    id: String(hit.id),
                    title: hit.title,
                    year: hit.year ?? 0,
                    director: hit.made_by ?? "",
                    tag: KIND_MARK[hit.kind],
                    note: personNote(hit.why, response?.person ?? null) ?? WHY_NOTE[hit.why],
                  }}
                  onOpen={() => open(hit.id)}
                />
              </li>
            ))}
          </ol>
        )}
      </div>

      {/*
        Said plainly rather than implied, and followed by the offer. Without the
        artefact, "films about grief" matches the WORD, and a user who does not know
        that concludes the search is bad rather than unavailable (ADR-0014's degraded
        path, SPEC.md §8).
      */}
      {response?.ready && !response.semantic && (
        <div className="mt-14 flex flex-col gap-4">
          {hits.length > 0 && (
            <p className="text-sm text-ink-muted">
              Searching titles and people only — describing a film rather than naming it
              will not find much yet.
            </p>
          )}
          <MeaningDownload />
        </div>
      )}
    </div>
  );
}
