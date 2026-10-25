import { useEffect, useRef, useState } from "react";
import { commands, type SearchHit, type SearchResponse, type Why } from "@/lib/ipc";
import { Skeleton, EmptyState } from "@/design-system";
import { MeaningDownload } from "./MeaningDownload";

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
 */
const DEBOUNCE_MS = 120;

/** What each match reason should say to a person, and how loudly. */
const WHY: Record<Why, { label: string; tone: "accent" | "neutral" }> = {
  exact_title: { label: "exact title", tone: "accent" },
  both: { label: "title and meaning", tone: "accent" },
  semantic: { label: "similar in meaning", tone: "neutral" },
  keyword: { label: "matched words", tone: "neutral" },
  fuzzy: { label: "close spelling", tone: "neutral" },
  filter: { label: "matched your filters", tone: "accent" },
};

/** The order kinds appear in. Films first because that is what most searches are for. */
const KIND_ORDER = ["film", "anime_film", "series", "anime_series", "episode"] as const;

const KIND_LABEL: Record<string, string> = {
  film: "Films",
  anime_film: "Anime films",
  series: "TV series",
  anime_series: "Anime series",
  episode: "Episodes",
  live_channel: "Live channels",
};

function groupByKind(hits: SearchHit[]): [string, SearchHit[]][] {
  const groups = new Map<string, SearchHit[]>();
  for (const hit of hits) {
    const existing = groups.get(hit.kind);
    if (existing) existing.push(hit);
    else groups.set(hit.kind, [hit]);
  }
  // Ranked order is preserved INSIDE each group; only the groups themselves are
  // ordered, and an unknown kind sorts last rather than vanishing.
  return [...groups.entries()].sort(
    ([a], [b]) =>
      (KIND_ORDER.indexOf(a as never) + 1 || 99) - (KIND_ORDER.indexOf(b as never) + 1 || 99),
  );
}

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

/**
 * One result, in one of three weights.
 *
 * §9.0 bans uniform sizing, and a ranked list has a natural hierarchy to show: the
 * lead result is set large with its synopsis, the next two a step down, and the rest
 * as a quiet list. The page reads as an edited answer with a lead and a body, not as a
 * table of equals.
 *
 * The match reason is quiet text, not a box on every row. It is only coloured when
 * it says something strong ("exact title", "matched your filters").
 */
function Entry({
  hit,
  number,
  weight,
  focused,
  onFocus,
}: {
  hit: SearchHit;
  number: number;
  weight: "lead" | "second" | "list";
  focused: boolean;
  onFocus: () => void;
}) {
  const why = WHY[hit.why];
  const byline = [
    hit.made_by,
    hit.year,
    hit.runtime_minutes !== null ? `${hit.runtime_minutes} min` : null,
  ]
    .filter((part) => part !== null && part !== undefined)
    .join(" · ");

  const title = {
    lead: "text-[46px] leading-[1.02] tracking-[-0.01em]",
    second: "text-[26px] leading-tight",
    list: "text-[20px] leading-snug",
  }[weight];

  return (
    <div
      data-row
      tabIndex={0}
      onFocus={onFocus}
      className={`grid grid-cols-[3.5rem_1fr_auto] items-baseline gap-x-4 border-b border-line-subtle ${
        weight === "lead" ? "pb-8" : weight === "second" ? "py-5" : "py-3"
      } ${focused ? "bg-raised" : ""}`}
    >
      <span className="spine">{String(number).padStart(2, "0")}</span>
      <div className="min-w-0">
        <h3 className={`font-serif text-ink ${title} ${weight === "list" ? "truncate" : ""}`}>
          {hit.title}
          {weight === "list" && byline && (
            <span className="label ml-4 align-middle">{byline}</span>
          )}
        </h3>
        {weight !== "list" && byline && <p className="label mt-2">{byline}</p>}
        {weight === "lead" && hit.excerpt && (
          <p className="mt-4 max-w-[62ch] text-[15px] leading-relaxed text-ink-muted">
            {hit.excerpt}
          </p>
        )}
      </div>
      <span className={`label ${why.tone === "accent" ? "text-oxblood-text" : ""}`}>
        {why.label}
      </span>
    </div>
  );
}

export function SearchScreen() {
  const [query, setQuery] = useState("");
  const [response, setResponse] = useState<SearchResponse | null>(null);
  const [searching, setSearching] = useState(false);
  const [focused, setFocused] = useState(-1);
  const [error, setError] = useState<string | null>(null);

  // The sequence number of the most recent response rendered. See the note above.
  const latest = useRef(0);
  const issued = useRef(0);
  const rowsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const text = query.trim();
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
        .search(text, 20)
        .then((result) => {
          if (sequence < latest.current) return; // A slower, older query. Discard it.
          latest.current = sequence;
          // The generated binding is a tagged Result, so a failing search cannot be
          // mistaken for an empty one — which is the distinction this whole screen is
          // careful about everywhere else.
          if (result.status === "ok") {
            setResponse(result.data);
            setError(null);
          } else {
            setResponse(null);
            setError(result.error);
          }
          setFocused(-1);
        })
        .finally(() => {
          if (sequence >= latest.current) setSearching(false);
        });
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query]);

  const hits = response?.hits ?? [];
  const groups = groupByKind(hits);
  let position = 0;

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      const next = Math.min(Math.max(focused + step, 0), hits.length - 1);
      setFocused(next);
      rowsRef.current?.querySelectorAll<HTMLElement>("[data-row]")[next]?.scrollIntoView({
        block: "nearest",
      });
    }
  }

  return (
    <div className="flex h-full max-w-[1100px] flex-col px-16 pt-14 pb-24">
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
        placeholder="A title, a director, or what it is about"
        aria-label="Search the catalogue"
        spellCheck={false}
        // The underline IS the focus indicator: it turns oxblood when focused, so the
        // global ring (a box) is replaced rather than removed. The size steps down as
        // the query grows, because a sentence-long query at 46px scrolls out of sight.
        className={`w-full border-0 border-b-2 border-line-strong bg-transparent pb-4 font-display leading-none font-semibold tracking-[-0.035em] text-ink placeholder:text-ink-faint focus:border-oxblood-bright focus-visible:outline-none ${querySize(query)}`}
      />
      <p className="label mt-3 h-4" aria-live="polite">
        {status(response, searching)}
      </p>

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
            title={response.catalogue_partial ? "Still building the catalogue" : "Nothing found"}
            body={
              response.catalogue_partial
                ? `${response.searchable_titles.toLocaleString()} titles are searchable so far, and more are still arriving. It may simply not be here yet.`
                : "No title matches that. Try fewer words, or describe what the film is about."
            }
          />
        )}

        <div ref={rowsRef} className="flex flex-col gap-14">
          {groups.map(([kind, group]) => (
            <section key={kind} aria-label={KIND_LABEL[kind] ?? kind}>
              <h2 className="label flex items-baseline justify-between border-t border-line-strong pt-3 pb-5">
                <span className="text-ink-muted">{KIND_LABEL[kind] ?? kind}</span>
                <span>{String(group.length).padStart(2, "0")}</span>
              </h2>
              <ol className="flex flex-col">
                {group.map((hit) => {
                  // DISPLAY order, not rank: grouping can put a lower-ranked film above a
                  // higher-ranked series, and the arrow keys walk the DOM. Numbering,
                  // weight and focus all follow what the eye and the keyboard see.
                  const index = position++;
                  return (
                    <li key={hit.id}>
                      <Entry
                        hit={hit}
                        number={index + 1}
                        weight={index === 0 ? "lead" : index < 3 ? "second" : "list"}
                        focused={focused === index}
                        onFocus={() => setFocused(index)}
                      />
                    </li>
                  );
                })}
              </ol>
            </section>
          ))}
        </div>
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
