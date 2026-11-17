import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ProgressBar } from "@/design-system";
import { commands } from "@/lib/ipc";
import { onEvent } from "@/lib/events";
import { CATALOGUE } from "@/features/settings/CatalogueSection";

/**
 * The first run's catalogue (Phase 16, the author's brief item 3). A fresh install downloads
 * IMDb's catalogue and builds its search index before search, identification, Films and TV
 * can work. The author's answers: show the progress during onboarding, and open Home only
 * when search works, with a "Getting ready… 72%" screen if it is still building.
 *
 * Unknown or failed status counts as ready: this screen must never be the thing that keeps
 * someone out of their own app.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export interface CatalogueBuild {
  ready: boolean;
  /** What it is doing ("Building search"), or null when it is between steps. */
  building: string | null;
  /** 0 to 100, over the whole first run. */
  percent: number;
}

export function useCatalogueBuild(): CatalogueBuild {
  const client = useQueryClient();
  const status = useQuery({
    queryKey: CATALOGUE,
    queryFn: () => ok(commands.catalogueStatus()),
    // While it builds, every two seconds; once ready, only when something says so.
    refetchInterval: (q) => (q.state.data && q.state.data.ready === false ? 2000 : false),
  });

  useEffect(() => {
    const again = () => void client.invalidateQueries({ queryKey: CATALOGUE });
    const offs = [onEvent("catalogue-building", again), onEvent("catalogue-refreshed", again)];
    return () => offs.forEach((off) => off());
  }, [client]);

  const s = status.data;
  const ready = s?.ready !== false;

  // The moment it becomes ready, everything that was empty for want of a catalogue (Home's
  // rails, search, the library's matches) is asked again.
  const was = useRef(ready);
  useEffect(() => {
    if (ready && !was.current) void client.invalidateQueries();
    was.current = ready;
  }, [ready, client]);

  return { ready, building: s?.building ?? null, percent: ready ? 100 : Math.max(0, s?.percent ?? 0) };
}

/** Onboarding's strip: the build going on while the viewer sets up. */
export function BuildStrip({ build }: { build: CatalogueBuild }) {
  if (build.ready) {
    return (
      <p className="label text-ink-muted" role="status">
        Catalogue ready · search works
      </p>
    );
  }
  return (
    <div role="status" aria-live="polite" className="flex flex-col gap-2">
      <p className="label text-ink-muted">
        Getting the catalogue ready · {build.building ?? "Starting"} · {build.percent}%
      </p>
      <ProgressBar value={build.percent / 100} label="Catalogue" />
    </div>
  );
}

/** Where Home would be, until search works. */
export function GettingReady({ build }: { build: CatalogueBuild }) {
  return (
    <div
      role="status"
      aria-live="polite"
      className="mx-auto flex min-h-full max-w-[880px] flex-col justify-center px-10 py-14"
    >
      <p className="label text-ink-muted">First run</p>
      <h1 className="mt-3 font-display text-[56px] font-extrabold leading-none tracking-[-0.03em]">
        Getting ready… {build.percent}%
      </h1>
      <p className="mt-6 max-w-[56ch] text-[16px] text-ink">
        {build.building ?? "Starting"}. sin·e·phile is building its catalogue of films and shows on
        this PC, so that search, your library and every page work without the internet once it is done.
      </p>
      <div className="mt-8 max-w-[560px]">
        <ProgressBar value={build.percent / 100} label="Catalogue" size="thick" />
      </div>
      <p className="mt-6 max-w-[56ch] text-[13px] text-ink-muted">
        This happens once. Settings and Downloads are open in the meantime, and your films on this PC
        are matched as soon as it finishes.
      </p>
    </div>
  );
}
