import { useQuery } from "@tanstack/react-query";
import { EmptyState, Skeleton } from "@/design-system";
import { TitleCard } from "@/features/home/TitleCard";
import { commands } from "@/lib/ipc";
import { useUi } from "@/lib/store";

/**
 * The Watchlist page, first version (SPEC.md Appendix F, I39): what this profile saved,
 * newest first, as a grid. Phase 20 adds named lists, order, notes and the smart views;
 * until then this replaces a placeholder with the thing itself.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const WATCHLIST = ["watchlist"];

export function WatchlistScreen() {
  const openDetail = useUi((s) => s.openDetail);
  const saved = useQuery({ queryKey: WATCHLIST, queryFn: () => ok(commands.watchlist()) });

  if (saved.isPending) {
    return (
      <div className="flex flex-col gap-10 px-16 pt-12">
        <Skeleton className="h-6 w-56" />
        <Skeleton className="h-[260px] w-full" />
      </div>
    );
  }
  if (saved.isError) {
    return <p className="px-16 pt-12 text-sm text-ink-muted">{saved.error.message}</p>;
  }
  if (saved.data.length === 0) {
    return (
      <EmptyState
        title="Nothing saved yet"
        body="Open any film or show and choose “Add to watchlist”. What you save waits here, newest first."
        whatInstead="A history imported from Letterboxd, Trakt, IMDb or MyAnimeList brings its watchlist with it: Settings, Profiles."
      />
    );
  }
  const count = saved.data.length;
  return (
    // The same 96px | 1fr grid as Home's rails (ADR-0024), so the spine runs on.
    <section aria-label="Watchlist" className="grid min-h-full" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
      <div className="border-r border-line-subtle" />
      <div className="min-w-0 pt-10 pr-14 pb-24 pl-8">
        <header className="mb-8">
          <div className="flex items-baseline gap-4">
            <h1 className="font-display text-[15px] font-extrabold uppercase tracking-[-0.01em] text-ink">Watchlist</h1>
            <span className="h-px flex-1 bg-line-subtle" />
          </div>
          <p className="label mt-2">
            What you saved, newest first · {count} {count === 1 ? "title" : "titles"}
          </p>
        </header>
        <ul className="grid gap-x-6 gap-y-10" style={{ gridTemplateColumns: "repeat(auto-fill, 194px)" }}>
          {saved.data.map((c, i) => (
            <li key={c.id}>
              <TitleCard
                id={c.id}
                size="md"
                film={{ id: String(c.id), title: c.title, year: c.year ?? 0, director: c.made_by ?? "", spine: i + 1 }}
                onOpen={() => openDetail(c.id)}
              />
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
