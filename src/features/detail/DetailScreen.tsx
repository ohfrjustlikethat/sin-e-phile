import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, EmptyState, Skeleton } from "@/design-system";
import { commands, type PlayPlanView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { clock } from "@/features/player/intents";

/**
 * The minimal detail page — SPEC.md Phase 8: title, synopsis, cast, and a Play button
 * that goes through the resolver.
 *
 * Designed for the artwork-free default (ADR-0013, CLAUDE.md): typography carries it.
 * It asks the sources **as soon as it opens** (`prepare_play`), so by the time Play is
 * pressed the source is chosen and a torrent is already finding peers (Phase 7 E1).
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

type PlanState =
  | { phase: "finding" }
  | { phase: "ready"; plan: PlayPlanView }
  | { phase: "none"; reason: string };

export function DetailScreen() {
  const id = useUi((s) => s.detail);
  const closeDetail = useUi((s) => s.closeDetail);
  const play = useUi((s) => s.play);
  const [plan, setPlan] = useState<PlanState>({ phase: "finding" });

  const item = useQuery({
    queryKey: ["item", id],
    queryFn: () => ok(commands.itemDetail(id ?? 0)),
    enabled: id !== null,
  });

  useEffect(() => {
    if (id === null) return;
    let live = true;
    setPlan({ phase: "finding" });
    void commands.preparePlay(id).then((r) => {
      if (!live) return;
      setPlan(r.status === "ok" ? { phase: "ready", plan: r.data } : { phase: "none", reason: r.error });
    });
    return () => {
      live = false;
    };
  }, [id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Backspace") closeDetail();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closeDetail]);

  if (id === null) return null;
  if (item.isPending) {
    return (
      <div className="px-16 pt-14">
        <Skeleton className="h-14 w-2/3" />
      </div>
    );
  }
  const d = item.data;
  if (!d) {
    return <EmptyState title="Not found" body="This item is no longer in the catalogue." />;
  }

  const makers = d.credits.filter((c) => c.role === "director" || c.role === "creator");
  const cast = d.credits.filter((c) => c.role === "actor" || c.role === "actress" || c.role === "self");
  const byline = [
    makers.map((m) => m.name).join(", ") || null,
    d.year,
    d.runtime_minutes ? `${d.runtime_minutes} min` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  const start = () => {
    if (plan.phase !== "ready") return;
    play({ target: plan.plan.open, title: d.title, mediaItemId: d.id, start: plan.plan.start });
  };

  return (
    <article className="max-w-[1100px] px-16 pt-12 pb-24">
      <button type="button" onClick={closeDetail} className="label mb-10 hover:text-ink">
        ← Back
      </button>
      <h1 className="font-serif text-[64px] leading-[0.98] tracking-[-0.015em] text-ink">{d.title}</h1>
      {byline && <p className="label mt-5">{byline}</p>}

      <div className="mt-10 flex items-center gap-5">
        <Button
          variant="primary"
          size="lg"
          onClick={start}
          disabled={plan.phase !== "ready"}
          aria-describedby="play-status"
        >
          {plan.phase === "ready" && plan.plan.start ? `Resume from ${clock(plan.plan.start)}` : "Play"}
        </Button>
        <p id="play-status" className="text-sm text-ink-muted" role="status">
          {plan.phase === "finding" && "Asking your sources…"}
          {plan.phase === "ready" && `From ${plan.plan.from}`}
          {plan.phase === "none" && plan.reason}
        </p>
      </div>

      {d.synopsis && (
        <p className="mt-12 max-w-[68ch] text-[17px] leading-[1.65] text-ink-muted">{d.synopsis}</p>
      )}

      {cast.length > 0 && (
        <section className="mt-14" aria-label="Cast">
          <h2 className="label mb-4 border-t border-line-strong pt-3">Cast</h2>
          <ul className="grid grid-cols-2 gap-x-10 gap-y-2.5 text-[15px] md:grid-cols-3">
            {cast.map((c) => (
              <li key={`${c.name}-${c.character ?? ""}`}>
                <span className="text-ink">{c.name}</span>
                {c.character && <span className="block text-[13px] text-ink-faint">{c.character}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}
