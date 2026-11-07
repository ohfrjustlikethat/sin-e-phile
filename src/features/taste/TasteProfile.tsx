import { useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, EmptyState } from "@/design-system";
import { commands, type TasteModeView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { ChoiceRow, type Choice } from "@/features/detail/ChoiceRow";
import { useProfiles } from "@/features/profiles/useProfiles";

/**
 * Your taste, shown (SPEC.md Phase 15): each mode the model found, said in a sentence
 * from what it holds, with its share and its three best-known titles; then the axes and
 * what you walk out of. Each mode can be corrected (more of this, less, not me), and the
 * correction is kept per profile and weighs Home's taste rail. The leanings are corrected
 * through what was said (Settings → Profiles), which is their prior: they describe, and do
 * not weigh recommendations themselves, so a toggle on one would change nothing (a Phase 15 decision, SESSION_LOG.md).
 *
 * Reached from Settings → Profiles and from Who's watching?. A settings sub-screen, like
 * the review queue: Escape or Back returns to Settings.
 */

export const TASTE_PROFILE = ["taste", "profile"] as const;

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const CORRECTIONS: Choice[] = [
  { value: null, label: "As it is" },
  { value: "more", label: "More of this" },
  { value: "less", label: "Less" },
  { value: "not_me", label: "Not me" },
];

/** "About a third", said as a share a person reads at a glance. */
export function shareWords(share: number | null): string {
  // A share crosses IPC as `number | null`: specta's f64 allows for a NaN.
  const pct = Math.round((share ?? 0) * 100);
  return pct < 1 ? "under 1% of your taste" : `${pct}% of your taste`;
}

export function TasteProfile() {
  const setTasteOpen = useUi((s) => s.setTasteOpen);
  const { active } = useProfiles();
  const profile = useQuery({ queryKey: TASTE_PROFILE, queryFn: () => ok(commands.tasteProfile()) });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setTasteOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setTasteOpen]);

  // A pad lands on the first mode's choice, once, rather than wherever focus fell.
  const list = useRef<HTMLOListElement>(null);
  const landed = useRef(false);
  useEffect(() => {
    if (landed.current || !profile.isSuccess) return;
    const first = list.current?.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]');
    if (first) {
      first.focus();
      landed.current = true;
    }
  }, [profile.isSuccess, profile.data]);

  const modes = profile.data?.modes ?? [];
  const axes = profile.data?.axes ?? [];
  return (
    <article className="max-w-[900px] px-16 pt-12 pb-24">
      <button type="button" onClick={() => setTasteOpen(false)} className="label mb-10 hover:text-ink">
        ← Back
      </button>
      <h1 className="font-serif text-[56px] leading-none tracking-[-0.015em] text-ink">
        {active ? `${active.name}'s taste` : "Your taste"}
      </h1>
      <p className="mt-5 max-w-[60ch] text-[15px] text-ink-muted">
        What the recommendations are made of, in plain words. If something here is not you, say so: Home
        changes with it.
      </p>

      {profile.isError && <p className="mt-10 text-sm text-danger">{profile.error.message}</p>}
      {profile.isSuccess && modes.length === 0 && axes.length === 0 && (
        <div className="mt-12">
          <EmptyState
            title="Not enough to go on yet"
            body="Watch, rate or add a few films to your watchlist, or import your history in Settings → Profiles. Your taste appears here as it forms."
          />
        </div>
      )}

      {modes.length > 0 && (
        <ol ref={list} aria-label="Your tastes" className="mt-12 flex flex-col">
          {modes.map((m, i) => (
            <Mode key={m.anchor} mode={m} n={i + 1} />
          ))}
        </ol>
      )}

      {axes.length > 0 && (
        <section aria-label="Leanings" className="mt-14">
          <h2 className="label mb-4">Leanings</h2>
          <ul className="flex flex-col gap-2">
            {axes.map((a) => (
              <li key={a} className="font-serif text-[20px] leading-snug text-ink">
                {a}
              </li>
            ))}
          </ul>
        </section>
      )}

      {profile.isSuccess && (
        <section aria-label="What you said" className="mt-14 border-t border-line-subtle pt-8">
          <p className="max-w-[60ch] text-[14px] text-ink-muted">
            Leanings come from what you watch and from what you said you like. If one is wrong, change what you
            said: it counts until what you watch says otherwise.
          </p>
          <div className="mt-4">
            <Button variant="secondary" size="sm" onClick={() => setTasteOpen(false)}>
              Change what you said
            </Button>
          </div>
        </section>
      )}
    </article>
  );
}

function Mode({ mode, n }: { mode: TasteModeView; n: number }) {
  const client = useQueryClient();
  const correct = useMutation({
    mutationFn: (c: string | null) => ok(commands.correctTaste(mode.anchor, c)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: TASTE_PROFILE });
      void client.invalidateQueries({ queryKey: ["home"] });
    },
  });
  const off = mode.correction === "not_me";
  return (
    <li className={`border-line-subtle py-8 ${n === 1 ? "" : "border-t"}`}>
      <p className={`font-serif text-[24px] leading-snug ${off ? "text-ink-faint line-through" : "text-ink"}`}>
        {mode.sentence}
      </p>
      <p className="label mt-3">{shareWords(mode.share)}</p>
      <div className="mt-5">
        <ChoiceRow
          label={`Taste ${n}`}
          choices={CORRECTIONS}
          value={mode.correction ?? null}
          onChange={(c) => correct.mutate(c)}
        />
      </div>
      {correct.isError && <p className="mt-2 text-sm text-danger">{correct.error.message}</p>}
    </li>
  );
}
