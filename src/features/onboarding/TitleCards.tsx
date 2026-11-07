import { useEffect, useState } from "react";
import { Button } from "@/design-system";
import { commands, type TastePick } from "@/lib/ipc";

/**
 * Pick any you love (SPEC.md Phase 14): typographic title cards, the artwork-free
 * equivalent of a poster grid, which "selects on knowledge and taste rather than poster
 * recognition". Up to four rounds; each is drawn across the decades and, once something
 * is picked, half from near the picks. A pick is a "liked" (four stars), and picking it
 * again takes it back.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const ROUNDS = 4;

export function TitleCards({ onPicked }: { onPicked?: (count: number) => void }) {
  const [round, setRound] = useState(1);
  const [cards, setCards] = useState<TastePick[]>([]);
  const [shown, setShown] = useState<number[]>([]);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const load = async (seen: number[]) => {
    try {
      const next = await ok(commands.tasteRound(seen, 12));
      setCards(next);
      setShown([...seen, ...next.map((c) => c.id)]);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  useEffect(() => {
    void load([]);
  }, []);

  const toggle = async (c: TastePick) => {
    const on = picked.has(c.id);
    await ok(commands.rateTitle(c.id, on ? null : 4));
    const next = new Set(picked);
    if (on) next.delete(c.id);
    else next.add(c.id);
    setPicked(next);
    onPicked?.(next.size);
  };

  return (
    <div className="flex flex-col gap-5">
      <p className="text-[13px] text-ink-muted">
        Pick any you love. Round {round} of {ROUNDS}
        {picked.size > 0 ? ` · ${picked.size} picked` : ""}
      </p>
      {error && <p className="text-sm text-danger">{error}</p>}
      <ul className="grid grid-cols-4 gap-3" aria-label="Title cards">
        {cards.map((c) => {
          const on = picked.has(c.id);
          return (
            <li key={c.id}>
              <button
                type="button"
                aria-pressed={on}
                onClick={() => void toggle(c)}
                className={[
                  "flex h-[118px] w-full flex-col justify-between rounded-sm border p-3 text-left transition-colors",
                  "duration-[var(--dur-standard)]",
                  on ? "border-ink bg-raised" : "border-line bg-surface hover:border-line-strong",
                ].join(" ")}
              >
                <span className="font-serif text-[19px] leading-tight text-ink">{c.title}</span>
                <span className="label">{[c.made_by, c.year].filter(Boolean).join(" · ")}</span>
              </button>
            </li>
          );
        })}
      </ul>
      {round < ROUNDS && (
        <div>
          <Button
            variant="secondary"
            onClick={() => {
              setRound((r) => r + 1);
              void load(shown);
            }}
          >
            {picked.size > 0 ? "More like these, and others" : "Show me others"}
          </Button>
        </div>
      )}
    </div>
  );
}
