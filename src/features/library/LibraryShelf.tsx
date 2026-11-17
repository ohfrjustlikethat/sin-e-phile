import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Skeleton } from "@/design-system";
import { TitleCard } from "@/features/home/TitleCard";
import { TitleRail } from "@/features/home/TitleRail";
import { onEvent } from "@/lib/events";
import { commands } from "@/lib/ipc";
import { openSettingsAt } from "@/lib/openSettings";
import { useUi } from "@/lib/store";

/**
 * The Films and TV Shows pages (Phase 16): what is on this PC first, by title, a show once however many of its
 * episodes are here; then rails from the whole catalogue (most watched, new this year, the main genres), so neither
 * tab is ever empty and any title opens its page with streams (the author's brief item 9). Phase 19 adds browsing
 * by decade, country and director.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const LIBRARY_TITLES = (shows: boolean) => ["library-titles", shows];
export const CATALOGUE_RAILS = (shows: boolean) => ["catalogue-rails", shows];

/** The small line under each catalogue rail's name. */
function why(key: string, shows: boolean): string {
  if (key === "most") return "the most voted on IMDb";
  if (key === "new") return "out lately, and being watched";
  return `the most watched ${shows ? "shows" : "films"}`;
}

export function LibraryShelf({ shows }: { shows: boolean }) {
  const openDetail = useUi((s) => s.openDetail);
  const titles = useQuery({ queryKey: LIBRARY_TITLES(shows), queryFn: () => ok(commands.libraryTitles(shows)) });
  const rails = useQuery({ queryKey: CATALOGUE_RAILS(shows), queryFn: () => ok(commands.catalogueRails(shows)) });
  const name = shows ? "TV Shows" : "Films";
  // A finished scan, or a file that changed: the shelf shows it at once, as Home does.
  const client = useQueryClient();
  useEffect(() => {
    const again = () => void client.invalidateQueries({ queryKey: ["library-titles"] });
    const offs = [
      onEvent<number>("library-changed", again),
      onEvent<{ phase: string }>("library-scan", (e) => {
        if (e.phase === "done") again();
      }),
    ];
    return () => offs.forEach((off) => off());
  }, [client]);

  const count = titles.data?.length ?? 0;
  return (
    <div className="flex flex-col gap-12 pb-24">
      {/* The same 96px | 1fr grid as Home's rails (ADR-0024), so the spine runs on. */}
      <section aria-label={name} className="grid" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
        <div className="border-r border-line-subtle" />
        <div className="min-w-0 pt-10 pr-14 pl-8">
          <header className="mb-8">
            <div className="flex items-baseline gap-4">
              <h1 className="font-display text-[15px] font-extrabold uppercase tracking-[-0.01em] text-ink">{name}</h1>
              <span className="h-px flex-1 bg-line-subtle" />
            </div>
            <p className="label mt-2">
              On this PC, by title
              {titles.data && ` · ${count} ${shows ? (count === 1 ? "show" : "shows") : count === 1 ? "film" : "films"}`}
            </p>
          </header>
          {titles.isPending && <Skeleton className="h-[260px] w-full" />}
          {titles.isError && <p className="text-sm text-ink-muted">{titles.error.message}</p>}
          {titles.data && count === 0 && (
            // Not a dead end: the catalogue is below, and the way to add a folder is here.
            <div className="flex flex-wrap items-center gap-4">
              <p className="text-sm text-ink-muted">
                {shows ? "No shows on this PC yet." : "No films on this PC yet."} Add the folder they are in, and they
                appear here as they are found.
              </p>
              <Button variant="secondary" size="sm" onClick={() => openSettingsAt("Library")}>
                Add a folder
              </Button>
            </div>
          )}
          {count > 0 && (
            <ul className="grid gap-x-6 gap-y-10" style={{ gridTemplateColumns: "repeat(auto-fill, 194px)" }}>
              {titles.data!.map((c, i) => (
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
          )}
        </div>
      </section>

      {(rails.data ?? []).map((r) => (
        <TitleRail key={r.key} rail={r} lead={false} why={why(r.key, shows)} />
      ))}
    </div>
  );
}
