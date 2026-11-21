import { Button } from "@/design-system";
import { useUi } from "@/lib/store";
import { useRatedCount } from "./useRating";

/** Below this many ratings, Home offers to rate films (ADR-0056). */
export const FEW_RATINGS = 10;

/**
 * Home's offer to rate films while a profile has fewer than ten ratings (ADR-0056, the author's answers): recommendations
 * stand on ratings, and a new profile has almost none. On the rails' grid, as the setup card is.
 */
export function RateCard() {
  const count = useRatedCount();
  const setRateOpen = useUi((s) => s.setRateOpen);
  if (count.data === undefined || count.data >= FEW_RATINGS) return null;
  return (
    <section aria-label="Rate films" className="grid" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
      <div className="border-r border-line-subtle" />
      <div className="flex min-w-0 flex-wrap items-center gap-x-8 gap-y-3 pt-2 pr-14 pl-8">
        <p className="max-w-[62ch] text-[15px] text-ink-muted">
          <span className="text-ink">Rate a few films to sharpen your recommendations.</span>{" "}
          {count.data === 0 ? "None rated yet." : `${count.data} of ${FEW_RATINGS} rated.`}
        </p>
        <Button variant="secondary" size="sm" onClick={() => setRateOpen(true)}>
          Rate films
        </Button>
      </div>
    </section>
  );
}
