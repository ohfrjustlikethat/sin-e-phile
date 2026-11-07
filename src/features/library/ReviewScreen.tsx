import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input } from "@/design-system";
import { useHints } from "@/design-system/focus/FocusLayer";
import { claimButtons, BUTTON } from "@/lib/gamepad";
import { commands, type ReviewItemView, type ReviewOptionView, type SearchHit } from "@/lib/ipc";
import { useUi } from "@/lib/store";

/**
 * The library's review queue — SPEC.md Phase 12 (E5: twenty answers in under a minute).
 *
 * One file at a time: what the name says on the left, what it could be on the right as
 * typographic cards (title, year, who made it, how long, a line of what it is). One
 * press answers, and the answer is remembered for every file named the same way, so a
 * series is settled by its first episode.
 *
 * Keys: 1–5 choose · S search · N not a film · L later · Esc back.
 * Pad: A chooses the focused card · Y search · X not a film · RB later · B back.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const QUEUE = ["library-review"];

const KIND: Record<string, string> = {
  film: "Film",
  anime_film: "Animated film",
  series: "Series",
  anime_series: "Anime series",
};

/** "1995 · Film · 170 min · Michael Mann" */
export function facts(o: Pick<ReviewOptionView, "year" | "kind" | "runtime_minutes" | "made_by">): string {
  return [o.year?.toString(), KIND[o.kind] ?? o.kind, o.runtime_minutes ? `${o.runtime_minutes} min` : null, o.made_by]
    .filter(Boolean)
    .join(" · ");
}

/** "1:52 · 1080p" for the file. */
export function fileFacts(item: Pick<ReviewItemView, "minutes" | "height">): string {
  const parts: string[] = [];
  if (item.minutes !== null) {
    const h = Math.floor(item.minutes / 60);
    parts.push(h > 0 ? `${h}:${String(item.minutes % 60).padStart(2, "0")}` : `${item.minutes} min`);
  }
  if (item.height !== null) parts.push(`${item.height}p`);
  return parts.join(" · ");
}

export function ReviewScreen() {
  const client = useQueryClient();
  const setReviewOpen = useUi((s) => s.setReviewOpen);
  const queue = useQuery({ queryKey: QUEUE, queryFn: () => ok(commands.libraryReview(50)) });
  const [skipped, setSkipped] = useState<number[]>([]);
  const [searching, setSearching] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [answered, setAnswered] = useState(0);

  const items = useMemo(() => queue.data ?? [], [queue.data]);
  // Skipped files go to the back, in the order they were skipped.
  const order = useMemo(
    () => [...items.filter((i) => !skipped.includes(i.file_id)), ...items.filter((i) => skipped.includes(i.file_id))],
    [items, skipped],
  );
  const item = order[0];

  async function after(settled: number) {
    setAnswered((n) => n + 1 + settled);
    setNote(settled > 0 ? `Settled ${settled} more named the same way.` : null);
    setSearching(false);
    await client.invalidateQueries({ queryKey: QUEUE });
    void client.invalidateQueries({ queryKey: ["library-review-count"] });
    void client.invalidateQueries({ queryKey: ["library-folders"] });
  }

  async function choose(mediaItemId: number) {
    if (!item || busy) return;
    setBusy(true);
    try {
      await after(await ok(commands.resolveReview(item.file_id, mediaItemId)));
    } finally {
      setBusy(false);
    }
  }

  async function notATitle() {
    if (!item || busy) return;
    setBusy(true);
    try {
      await ok(commands.notATitle(item.file_id));
      await after(0);
    } finally {
      setBusy(false);
    }
  }

  function later() {
    if (!item) return;
    setSkipped((s) => [...s.filter((id) => id !== item.file_id), item.file_id]);
    setNote(null);
  }

  // Keys and pad buttons, while no text field has them.
  const handlers = useRef({ choose, notATitle, later });
  handlers.current = { choose, notATitle, later };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement;
      if (e.key === "Escape") {
        if (searching) setSearching(false);
        else setReviewOpen(false);
        e.preventDefault();
        return;
      }
      if (typing || !item) return;
      const n = Number.parseInt(e.key, 10);
      if (n >= 1 && n <= item.options.length) void handlers.current.choose(item.options[n - 1]!.media_item_id);
      else if (e.key === "s" || e.key === "S") setSearching(true);
      else if (e.key === "n" || e.key === "N") void handlers.current.notATitle();
      else if (e.key === "l" || e.key === "L") handlers.current.later();
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    const release = claimButtons((b) => {
      if (!item || searching) return false;
      if (b === BUTTON.Y) setSearching(true);
      else if (b === BUTTON.X) void handlers.current.notATitle();
      else if (b === BUTTON.RB) handlers.current.later();
      else return false;
      return true;
    });
    return () => {
      window.removeEventListener("keydown", onKey);
      release();
    };
  }, [item, searching, setReviewOpen]);

  // TV mode's hint bar says what the buttons do here.
  useEffect(
    () =>
      useHints.getState().push([
        ["A", "Choose"],
        ["Y", "Search"],
        ["X", "Not a film"],
        ["RB", "Later"],
        ["B", "Back"],
      ]),
    [],
  );

  // The first card has focus, so A answers at once on a pad and Enter on a keyboard.
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!searching) first.current?.focus();
  }, [item?.file_id, searching]);

  return (
    <div className="mx-auto max-w-5xl px-10 py-10" aria-label="Check your library">
      <header className="mb-8 flex items-start justify-between">
        <div>
          <h1 className="text-[26px] font-semibold tracking-[-0.02em]">Check your library</h1>
          <p className="mt-1 text-sm text-ink-muted" aria-live="polite">
            {item
              ? `${items.length} waiting${answered > 0 ? ` · ${answered} settled` : ""}`
              : queue.isPending
                ? "Reading the queue…"
                : "Nothing waiting."}
          </p>
        </div>
        <Button variant="secondary" onClick={() => setReviewOpen(false)}>
          Done
        </Button>
      </header>

      {note && <p className="mb-6 text-sm text-ink" role="status">{note}</p>}

      {!item && !queue.isPending && (
        <p className="text-[15px] text-ink-muted">
          Every file in your library is identified. New files are checked as they arrive.
        </p>
      )}

      {item && (
        <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-10">
          <section aria-label="The file">
            <p className="label mb-2 text-ink-faint">The file</p>
            <p className="font-ui text-[17px] leading-snug break-all text-ink">{item.name}</p>
            {item.folders && <p className="mt-1 text-sm break-all text-ink-muted">in {item.folders}</p>}
            {fileFacts(item) && <p className="mt-3 text-sm text-ink-muted">{fileFacts(item)}</p>}
            <p className="label mt-6 mb-1 text-ink-faint">Read as</p>
            <p className="text-[15px] text-ink">
              {item.read_title || "No title in the name"}
              {item.read_year ? ` (${item.read_year})` : ""}
            </p>
            {item.read_episode && <p className="text-sm text-ink-muted">{item.read_episode}</p>}
            {item.same_name > 0 && (
              <p className="mt-6 text-sm text-ink-muted">
                Your answer also settles {item.same_name} more named the same way.
              </p>
            )}
          </section>

          <section aria-label="What it could be">
            {searching ? (
              <ManualSearch initial={item.read_title} onChoose={(id) => void choose(id)} onCancel={() => setSearching(false)} />
            ) : (
              <>
                <p className="label mb-2 text-ink-faint">Which is it?</p>
                <ol className="flex flex-col gap-2">
                  {item.options.map((o, i) => (
                    <li key={o.media_item_id}>
                      <button
                        ref={i === 0 ? first : undefined}
                        type="button"
                        disabled={busy}
                        onClick={() => void choose(o.media_item_id)}
                        className="flex w-full items-baseline gap-4 rounded-md border border-line px-5 py-4 text-left hover:bg-raised"
                      >
                        <span className="w-4 shrink-0 tabular-nums text-ink-faint">{i + 1}</span>
                        <span className="min-w-0">
                          <span className="block font-display text-[22px] leading-tight font-semibold">{o.title}</span>
                          <span className="mt-1 block text-sm text-ink-muted">{facts(o)}</span>
                          {o.synopsis && <span className="mt-1 block text-sm text-ink-faint">{o.synopsis}</span>}
                        </span>
                      </button>
                    </li>
                  ))}
                </ol>
                {item.options.length === 0 && (
                  <p className="text-sm text-ink-muted">Nothing in the catalogue looks like it. Search for it below.</p>
                )}
                <div className="mt-5 flex flex-wrap gap-2">
                  <Button variant="ghost" onClick={() => setSearching(true)}>
                    Search the catalogue (S)
                  </Button>
                  <Button variant="ghost" onClick={() => void notATitle()} disabled={busy}>
                    Not a film or episode (N)
                  </Button>
                  <Button variant="ghost" onClick={later}>
                    Later (L)
                  </Button>
                </div>
              </>
            )}
          </section>
        </div>
      )}
    </div>
  );
}

function ManualSearch(p: { initial: string; onChoose: (id: number) => void; onCancel: () => void }) {
  const [q, setQ] = useState(p.initial);
  const [hits, setHits] = useState<SearchHit[]>([]);
  useEffect(() => {
    if (q.trim().length < 2) {
      setHits([]);
      return;
    }
    let live = true;
    const t = window.setTimeout(() => {
      void commands.search(q, 8).then((r) => {
        if (live && r.status === "ok") setHits(r.data.hits);
      });
    }, 150);
    return () => {
      live = false;
      window.clearTimeout(t);
    };
  }, [q]);
  return (
    <div>
      <Input label="Search the catalogue" value={q} onChange={(e) => setQ(e.target.value)} autoFocus />
      <ol className="mt-3 flex flex-col">
        {hits.map((h) => (
          <li key={h.id}>
            <button
              type="button"
              onClick={() => p.onChoose(h.id)}
              className="flex w-full items-baseline justify-between gap-4 border-t border-line-subtle py-2.5 text-left hover:bg-raised"
            >
              <span className="text-[15px] text-ink">{h.title}</span>
              <span className="text-sm text-ink-muted">{facts({ year: h.year, kind: h.kind, runtime_minutes: h.runtime_minutes, made_by: h.made_by })}</span>
            </button>
          </li>
        ))}
      </ol>
      <Button variant="ghost" className="mt-3" onClick={p.onCancel}>
        Back to the suggestions
      </Button>
    </div>
  );
}
