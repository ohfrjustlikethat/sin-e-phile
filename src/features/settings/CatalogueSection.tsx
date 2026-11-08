import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/design-system";
import { commands } from "@/lib/ipc";
import { onEvent } from "@/lib/events";

/**
 * The catalogue keeping itself current (SPEC.md Phase 16, ADR-0030): checked every hour,
 * refreshed when IMDb republishes (about daily), new episodes of followed shows included.
 * This says when it last did something and offers to do it now.
 */

export const CATALOGUE = ["catalogue-status"] as const;

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/** "today", "yesterday", "3 days ago", or the date. */
export function when(iso: string, now: Date = new Date()): string {
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return "at an unknown time";
  const day = (d: Date) => Math.floor(d.getTime() / 86_400_000 - d.getTimezoneOffset() / 1440);
  const days = day(now) - day(then);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 7) return `${days} days ago`;
  return `on ${then.toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" })}`;
}

const n = (x: number) => x.toLocaleString();

export function CatalogueSection() {
  const client = useQueryClient();
  const status = useQuery({ queryKey: CATALOGUE, queryFn: () => ok(commands.catalogueStatus()) });
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const refreshing = onEvent<boolean>("catalogue-refreshing", (on) => {
      setBusy(on);
      if (!on) void client.invalidateQueries({ queryKey: CATALOGUE });
    });
    return () => refreshing();
  }, [client]);

  const now = useMutation({
    mutationFn: () => ok(commands.refreshCatalogue()),
    onSuccess: () => setBusy(true),
  });

  const s = status.data;
  const running = busy || s?.running === true;
  const line = s?.refreshed_at
    ? `Updated ${when(s.refreshed_at)}: ${n(s.titles_added ?? 0)} new titles, ${n(s.episodes_added ?? 0)} new episodes.`
    : "Not updated since it was built.";

  return (
    <section aria-label="Catalogue" className="mb-12">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">Catalogue</h2>
      <p className="text-[14px] text-ink" role="status">
        {running ? "Updating the catalogue now. This runs in the background." : line}
      </p>
      <p className="mt-1 text-[13px] text-ink-muted">
        New films and new episodes arrive on their own: the catalogue is checked every hour and updated
        when there is something new, usually once a day.
      </p>
      <div className="mt-4">
        <Button variant="secondary" size="sm" disabled={running} onClick={() => now.mutate()}>
          Refresh now
        </Button>
      </div>
      {now.isError && <p className="mt-2 text-sm text-danger">{now.error.message}</p>}
    </section>
  );
}
