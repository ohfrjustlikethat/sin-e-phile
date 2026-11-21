import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Skeleton, StarRating, starWords } from "@/design-system";
import { commands } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { toast } from "@/lib/toast";
import { useRatedCount, useTitleRating } from "./useRating";

/**
 * Rate films, one after another (ADR-0056, the author's answers): well-known films, most voted first, never one already
 * rated or passed over. Stars, then Next; or "Haven't seen it", which is no signal of taste and is not asked again.
 * Opened from Your taste and from Home while a profile has fewer than ten ratings.
 */

const BATCH = 20;

export function RateScreen() {
  const client = useQueryClient();
  const batch = useQuery({
    queryKey: ["to-rate"],
    queryFn: async () => {
      const r = await commands.toRate(BATCH);
      if (r.status === "error") throw new Error(r.error);
      return r.data;
    },
    staleTime: Infinity,
  });
  const count = useRatedCount();
  const [at, setAt] = useState(0);
  const films = batch.data ?? [];
  const film = films[at];

  const next = () => {
    if (at + 1 < films.length) {
      setAt(at + 1);
      return;
    }
    // The end of this batch: the next one leaves out everything just rated or passed over.
    setAt(0);
    void client.invalidateQueries({ queryKey: ["to-rate"] });
  };
  const notSeen = async () => {
    if (!film) return;
    const r = await commands.skipRating(film.id);
    if (r.status === "error") toast(`Not saved: ${r.error}`, "danger");
    next();
  };

  return (
    <section aria-label="Rate films" className="grid" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
      <div className="border-r border-line-subtle" />
      <div className="min-w-0 pt-10 pr-14 pb-24 pl-8">
        <header className="mb-14 max-w-[68ch]">
          <div className="flex items-baseline gap-4">
            <h1 className="font-display text-[15px] font-extrabold uppercase tracking-[-0.01em] text-ink">Rate films</h1>
            <span className="h-px flex-1 bg-line-subtle" />
            {count.data !== undefined && (
              <span className="label" aria-label="Rated so far">
                {count.data} rated
              </span>
            )}
          </div>
          <p className="mt-4 text-[15px] leading-[1.6] text-ink-muted">
            Rate what you have seen. Every rating sharpens what Home recommends; pass over what you haven&rsquo;t seen.
          </p>
        </header>
        {batch.isPending ? (
          <Skeleton className="h-40 w-2/3" />
        ) : batch.isError ? (
          <p className="text-sm text-ink-muted">{batch.error.message}</p>
        ) : !film ? (
          <p className="text-[15px] text-ink-muted">
            Nothing more to ask about: every well-known film in the catalogue is rated or passed over.
          </p>
        ) : (
          <RateOne
            key={film.id}
            id={film.id}
            title={film.title}
            byline={[film.made_by, film.year, film.runtime_minutes ? `${film.runtime_minutes} min` : null]
              .filter(Boolean)
              .join(" · ")}
            place={`${at + 1} of ${films.length}`}
            onNext={next}
            onNotSeen={() => void notSeen()}
            onDone={() => useUi.getState().back()}
          />
        )}
      </div>
    </section>
  );
}

function RateOne({
  id,
  title,
  byline,
  place,
  onNext,
  onNotSeen,
  onDone,
}: {
  id: number;
  title: string;
  byline: string;
  place: string;
  onNext: () => void;
  onNotSeen: () => void;
  onDone: () => void;
}) {
  const { halves, set } = useTitleRating(id);
  const stars = useRef<HTMLDivElement>(null);
  // Each film starts on its stars, so a pad's Left and Right rate it straight away.
  useEffect(() => {
    stars.current?.querySelector<HTMLInputElement>("input")?.focus();
  }, [id]);
  return (
    <article aria-label={title} className="max-w-[900px]">
      <p className="label text-ink-faint">{place}</p>
      <h2 className="mt-3 font-serif text-[56px] leading-[1.0] tracking-[-0.015em] text-ink">{title}</h2>
      {byline && <p className="label mt-4">{byline}</p>}
      <div ref={stars} className="mt-10 flex items-center gap-4">
        <StarRating value={halves} onChange={set} />
        <span className="label">{halves ? starWords(halves) : "Rate"}</span>
      </div>
      <div className="mt-10 flex items-center gap-4">
        <Button variant="primary" onClick={onNext} disabled={halves === null}>
          Next
        </Button>
        <Button variant="secondary" onClick={onNotSeen}>
          Haven&rsquo;t seen it
        </Button>
        <span className="flex-1" />
        <button type="button" className="label hover:text-ink" onClick={onDone}>
          Done
        </button>
      </div>
    </article>
  );
}
