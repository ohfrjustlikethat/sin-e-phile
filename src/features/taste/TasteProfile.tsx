import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, EmptyState, Slider } from "@/design-system";
import { commands, type TasteModeView, type TastePick } from "@/lib/ipc";
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

  // Escape is the app's one way back (App.tsx), with the labelled arrow above this page (BackBar).

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
    <article className="max-w-[900px] px-16 pt-6 pb-24">
      <h1 className="font-serif text-[56px] leading-none tracking-[-0.015em] text-ink">
        {active ? `${active.name}'s taste` : "Your taste"}
      </h1>
      <p className="mt-5 max-w-[60ch] text-[15px] text-ink-muted">
        What your recommendations are made of. Not you? Say so, and Home follows.
      </p>
      {/* Ratings are what recommendations stand on (ADR-0056): rate many, one after another. */}
      <div className="mt-6">
        <Button variant="secondary" size="sm" onClick={() => useUi.getState().setRateOpen(true)}>
          Rate more films
        </Button>
      </div>

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

      {modes.length > 0 && <HomePreview />}

      {profile.isSuccess && <Adventure />}

      {profile.isSuccess && (
        <section aria-label="What you said" className="mt-14 border-t border-line-subtle pt-8">
          <p className="max-w-[60ch] text-[14px] text-ink-muted">
            A leaning wrong? Change what you said you like; it counts until what you watch says otherwise.
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

/** The budget's word: where the slider sits. */
export function adventureWord(v: number): string {
  return v < 0.34 ? "Comfort" : v < 0.67 ? "Balanced" : "Adventure";
}

/**
 * Comfort to Adventure (Phase 18): how much of Home the discovery rails start with. What the
 * profile finishes and abandons moves it from there; this is where it starts.
 */
function Adventure() {
  const client = useQueryClient();
  const saved = useQuery({ queryKey: ["discovery-budget"], queryFn: () => ok(commands.discoveryBudget()) });
  const [value, setValue] = useState<number | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const shown = value ?? saved.data ?? 0.5;
  const timer = useRef<number | undefined>(undefined);
  const save = useMutation({
    mutationFn: (v: number) => ok(commands.setDiscoveryBudget(v)),
    onSuccess: () => {
      setSaid("Saved: Home follows");
      void client.invalidateQueries({ queryKey: ["discovery-budget"] });
      void client.invalidateQueries({ queryKey: ["home"] });
    },
    onError: (e) => setSaid(`Not saved: ${e.message}`),
  });
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return (
    <section aria-label="Comfort or adventure" className="mt-14 border-t border-line-subtle pt-8">
      <h2 className="label mb-2">Comfort or adventure</h2>
      <p className="max-w-[60ch] text-[14px] text-ink-muted">
        Comfort keeps Home close to what you love. Adventure gives more of it to stretches, blind spots and deep
        cuts. Whichever you choose, what you finish pushes it further and what you abandon eases it back.
      </p>
      <div className="mt-4 max-w-[420px]">
        <Slider
          label="Comfort to Adventure"
          value={Math.round(shown * 100)}
          display={adventureWord(shown)}
          onChange={(n) => {
            const v = n / 100;
            setValue(v);
            setSaid(null);
            window.clearTimeout(timer.current);
            timer.current = window.setTimeout(() => save.mutate(v), 300);
          }}
        />
        <p role="status" className="min-h-[18px] font-ui text-[12.5px] text-ink-muted">
          {said}
        </p>
      </div>
    </section>
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
  const pct = Math.round((mode.share ?? 0) * 100);
  return (
    <li className={`grid grid-cols-[72px_1fr] gap-x-6 border-line-subtle py-10 ${n === 1 ? "" : "border-t"}`}>
      <span aria-hidden className="font-serif text-[44px] leading-none text-ink-faint tabular-nums">
        {String(n).padStart(2, "0")}
      </span>
      <div>
        <p className={`font-serif text-[28px] leading-snug ${off ? "text-ink-faint line-through" : "text-ink"}`}>
          {mode.sentence}
        </p>
        <div className="mt-4 flex items-center gap-3" aria-label={shareWords(mode.share)}>
          <span className="h-[3px] w-40 bg-line" aria-hidden>
            <span className="block h-full bg-ink-muted" style={{ width: `${Math.max(2, pct)}%` }} />
          </span>
          <span className="label">{shareWords(mode.share)}</span>
        </div>
        {mode.examples.length > 0 && <Examples films={mode.examples} />}
        <div className="mt-6">
          <ChoiceRow
            label={`Taste ${n}`}
            choices={CORRECTIONS}
            value={mode.correction ?? null}
            onChange={(c) => correct.mutate(c)}
          />
        </div>
        {correct.isError && <p className="mt-2 text-sm text-danger">{correct.error.message}</p>}
      </div>
    </li>
  );
}

/** A taste's best-known films, as small typographic cards that open the film. */
function Examples({ films }: { films: TastePick[] }) {
  const openDetail = useUi((s) => s.openDetail);
  return (
    <ul aria-label="Films in this taste" className="mt-5 flex flex-wrap gap-3">
      {films.map((f) => (
        <li key={f.id}>
          <button
            type="button"
            onClick={() => openDetail(f.id)}
            className="flex w-44 flex-col items-start gap-1 rounded-sm border border-line px-3 py-2.5 text-left hover:bg-raised"
          >
            <span className="font-serif text-[16px] leading-tight text-ink">{f.title}</span>
            <span className="text-[12px] text-ink-muted">{[f.year, f.made_by].filter(Boolean).join(" · ")}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}

/** Home's taste rail as it stands, so a correction shows what it changed (Phase 16). */
function HomePreview() {
  const home = useQuery({ queryKey: ["home"], queryFn: () => ok(commands.home(new Date().getHours())) });
  const rail = home.data?.find((r) => r.key === "taste");
  if (!rail || rail.items.length === 0) return null;
  return (
    <section aria-label="Where your taste points now" className="mt-14 border-t border-line-subtle pt-8">
      <h2 className="label mb-4">Where your taste points now</h2>
      <ol className="flex flex-wrap gap-x-6 gap-y-2">
        {rail.items.slice(0, 6).map((c) => (
          <li key={c.id} className="font-serif text-[18px] text-ink">
            {c.title}
            {c.year ? <span className="ml-2 text-[13px] text-ink-faint">{c.year}</span> : null}
          </li>
        ))}
      </ol>
    </section>
  );
}
