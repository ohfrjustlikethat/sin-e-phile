import type { CandidateView } from "@/lib/ipc";

/**
 * The expert panel — SPEC.md Phase 9: "the full ranked candidate list with every score
 * component". A debugging tool and a demo, so it shows the arithmetic, not a summary:
 * each term's value, its weight, and the points they make, which sum to the total.
 *
 * The numbers are the backend's own (`sinephile_selection::Score`), never recomputed
 * here, so the panel cannot disagree with the ranking it explains (E5).
 */

/** Specta types an f64 as `number | null` (JSON has no NaN); a score never is NaN. */
const signed = (n: number | null) =>
  n === null ? "–" : `${n > 0 ? "+" : n < 0 ? "−" : ""}${Math.abs(n).toFixed(2)}`;

export function SourcePanel({ candidates }: { candidates: CandidateView[] }) {
  if (candidates.length === 0) return null;
  return (
    <section aria-label="Sources, ranked" className="mt-14">
      <h2 className="label mb-4 border-t border-line-strong pt-3">
        Sources, ranked <span className="text-ink-faint">· {candidates.length}</span>
      </h2>
      <ol className="flex flex-col gap-8">
        {candidates.map((c, i) => (
          // A focus stop per candidate, so a pad can scroll the panel.
          <li key={i} tabIndex={0} aria-label={`${i + 1}. ${c.from}, score ${signed(c.total)}`}>
            <div className="flex items-baseline justify-between gap-6">
              <p className="text-[15px] text-ink">
                <span className="mr-3 tabular-nums text-ink-faint">{i + 1}</span>
                {c.from}
                <span className="text-ink-muted">
                  {" · "}
                  {c.title} · {c.quality} · {c.codec === "unknown" ? "codec not stated" : c.codec} · {c.transport}
                </span>
              </p>
              <p className="shrink-0 tabular-nums text-[15px] text-ink" data-total>
                {signed(c.total)}
              </p>
            </div>
            <Notes c={c} />
            <table className="mt-3 w-full text-[13px]">
              <thead className="sr-only">
                <tr>
                  <th>Term</th>
                  <th>Value</th>
                  <th>Weight</th>
                  <th>Points</th>
                  <th>Why</th>
                </tr>
              </thead>
              <tbody>
                {c.terms.map((t) => (
                  <tr key={t.name} className="border-t border-line-subtle">
                    <td className="w-32 py-1 text-ink-muted">{t.name}</td>
                    <td className="w-16 py-1 text-right tabular-nums text-ink-muted">{signed(t.value)}</td>
                    <td className="w-14 py-1 text-right tabular-nums text-ink-faint">×{t.weight}</td>
                    <td className="w-16 py-1 text-right tabular-nums text-ink" data-points>
                      {signed((t.value ?? 0) * (t.weight ?? 0))}
                    </td>
                    <td className="py-1 pl-6 text-ink-muted">{t.why}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </li>
        ))}
      </ol>
    </section>
  );
}

/** "Plays · On this machine · Last: …", only the ones that are true, with no stray dot. */
function Notes({ c }: { c: CandidateView }) {
  const notes = [
    c.chosen && <span className="text-ink">Plays</span>,
    c.local && "On this machine",
    c.sunk && <span className="text-warning">Last: {c.sunk}</span>,
    !c.sunk && c.unmet && `Not as chosen: ${c.unmet}`,
  ].filter(Boolean);
  if (notes.length === 0) return null;
  return (
    <p className="mt-1 text-[13px] text-ink-muted">
      {notes.map((n, i) => (
        <span key={i}>
          {i > 0 && " · "}
          {n}
        </span>
      ))}
    </p>
  );
}
