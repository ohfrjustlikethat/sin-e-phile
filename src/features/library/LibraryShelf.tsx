import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, EmptyState, PosterCard, Skeleton } from "@/design-system";
import { onEvent } from "@/lib/events";
import { commands } from "@/lib/ipc";
import { openSettingsAt } from "@/lib/openSettings";
import { useUi } from "@/lib/store";

/**
 * The Films and TV Shows pages, first version (Phase 16): what is on this PC, by title, a
 * show once however many of its episodes are here. The author found the TV Shows page a
 * placeholder while their shows sat on Home one episode at a time. Phase 19 adds the
 * catalogue around this (genre, decade, country, director).
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const LIBRARY_TITLES = (shows: boolean) => ["library-titles", shows];

export function LibraryShelf({ shows }: { shows: boolean }) {
  const openDetail = useUi((s) => s.openDetail);
  const titles = useQuery({ queryKey: LIBRARY_TITLES(shows), queryFn: () => ok(commands.libraryTitles(shows)) });
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

  if (titles.isPending) {
    return (
      <div className="flex flex-col gap-10 px-16 pt-12">
        <Skeleton className="h-6 w-56" />
        <Skeleton className="h-[260px] w-full" />
      </div>
    );
  }
  if (titles.isError) {
    return <p className="px-16 pt-12 text-sm text-ink-muted">{titles.error.message}</p>;
  }
  if (titles.data.length === 0) {
    return (
      <EmptyState
        title={shows ? "No shows on this PC yet" : "No films on this PC yet"}
        body={`${name} lists what is in your library folders. Add the folder they are in, and they appear here as they are found.`}
        whatInstead="Settings, Library: a folder of films, or of shows in their own folders."
        action={
          <Button variant="secondary" onClick={() => openSettingsAt("Library")}>
            Add a folder
          </Button>
        }
      />
    );
  }
  const count = titles.data.length;
  return (
    // The same 96px | 1fr grid as Home's rails (ADR-0024), so the spine runs on.
    <section aria-label={name} className="grid min-h-full" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
      <div className="border-r border-line-subtle" />
      <div className="min-w-0 pt-10 pr-14 pb-24 pl-8">
        <header className="mb-8">
          <div className="flex items-baseline gap-4">
            <h1 className="font-display text-[15px] font-extrabold uppercase tracking-[-0.01em] text-ink">{name}</h1>
            <span className="h-px flex-1 bg-line-subtle" />
          </div>
          <p className="label mt-2">
            On this PC, by title · {count} {shows ? (count === 1 ? "show" : "shows") : count === 1 ? "film" : "films"}
          </p>
        </header>
        <ul className="grid gap-x-6 gap-y-10" style={{ gridTemplateColumns: "repeat(auto-fill, 194px)" }}>
          {titles.data.map((c, i) => (
            <li key={c.id}>
              <PosterCard
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
