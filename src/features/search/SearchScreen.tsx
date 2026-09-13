import { useEffect, useRef, useState } from "react";
import { commands, type SearchHit, type SearchResponse, type Why } from "@/lib/ipc";
import { Input, Badge, Skeleton, EmptyState } from "@/design-system";

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
    <div className="mx-auto flex h-full max-w-3xl flex-col gap-6 p-8">
      <Input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.currentTarget.value)}
        onKeyDown={onKeyDown}
        placeholder="Search — a title, a director, or what a film is about"
        aria-label="Search the catalogue"
      />

      {searching && !response && (
        <div className="flex flex-col gap-2" aria-hidden>
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      )}

      {error && (
        <EmptyState
          title="Search failed"
          body={`The engine returned an error: ${error}`}
        />
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

      <div ref={rowsRef} className="flex flex-col gap-6">
        {groupByKind(hits).map(([kind, group]) => (
          <section key={kind} aria-label={KIND_LABEL[kind] ?? kind}>
            <h2 className="mb-2 text-sm font-medium text-ink-muted">
              {KIND_LABEL[kind] ?? kind}
            </h2>
            <ul className="flex flex-col">
              {group.map((hit) => {
                const index = hits.indexOf(hit);
                return (
                  <li key={hit.id}>
                    <div
                      data-row
                      tabIndex={0}
                      onFocus={() => setFocused(index)}
                      className={`flex items-center justify-between gap-4 rounded px-3 py-2 outline-none ${
                        focused === index ? "bg-surface-raised" : ""
                      } focus-visible:ring-2 focus-visible:ring-accent`}
                    >
                      <span className="min-w-0 truncate">
                        {hit.title}
                        {hit.year !== null && (
                          <span className="ml-2 text-ink-muted">{hit.year}</span>
                        )}
                      </span>
                      <Badge tone={WHY[hit.why].tone}>{WHY[hit.why].label}</Badge>
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>

      {/*
        Said plainly rather than implied. Without the artefact, "films about grief"
        matches the WORD, and a user who does not know that concludes the search is
        bad rather than unavailable (ADR-0014's degraded path, SPEC.md §8).
      */}
      {response?.ready && !response.semantic && hits.length > 0 && (
        <p className="text-sm text-ink-muted">
          Searching titles and people only — the meaning index is not downloaded, so
          describing a film rather than naming it will not find much.
        </p>
      )}
    </div>
  );
}
