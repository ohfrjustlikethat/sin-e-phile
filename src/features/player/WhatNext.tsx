import { forwardRef, useEffect, useState } from "react";
import { commands, type EpisodeView, type TitleView } from "@/lib/ipc";

/**
 * What comes next — SPEC.md Phase 11, ADR-0043.
 *
 * `NextEpisodeCard`: bottom right over the video (cut out of it like the controls), the
 * next episode with a ten-second countdown when autoplay is on. A plays it now, B dismisses.
 *
 * `EndCard`: after a film's credits, a calm card: what you watched, your rating, and what
 * to watch next.
 */

export const COUNTDOWN_SECONDS = 10;

interface CardProps {
  next: EpisodeView;
  autoplay: boolean;
  onPlay: () => void;
  onDismiss: () => void;
}

export const NextEpisodeCard = forwardRef<HTMLDivElement, CardProps>(function NextEpisodeCard(
  { next, autoplay, onPlay, onDismiss },
  ref,
) {
  const [left, setLeft] = useState(COUNTDOWN_SECONDS);
  useEffect(() => {
    if (!autoplay) return;
    if (left <= 0) {
      onPlay();
      return;
    }
    const t = window.setTimeout(() => setLeft((l) => l - 1), 1000);
    return () => window.clearTimeout(t);
  }, [autoplay, left, onPlay]);

  const label = [next.season != null ? `S${next.season}` : null, next.episode != null ? `E${next.episode}` : null]
    .filter(Boolean)
    .join(" ");
  return (
    <div
      ref={ref}
      role="dialog"
      aria-label="Next episode"
      className="absolute right-3 bottom-[96px] w-[360px] rounded-md bg-surface px-5 py-4"
    >
      <p className="label text-ink-faint">Next episode{label && ` · ${label}`}</p>
      <p className="mt-1.5 truncate text-[17px] text-ink">{next.title}</p>
      <div className="mt-3 flex items-center gap-3">
        <button
          type="button"
          onClick={onPlay}
          className="rounded-sm bg-oxblood px-4 py-2 text-[12px] tracking-[0.12em] text-ink uppercase hover:bg-oxblood-hover"
        >
          {autoplay ? `Play in ${left}` : "Play"}
        </button>
        <button type="button" onClick={onDismiss} className="px-3 py-2 text-[13px] text-ink-muted hover:text-ink">
          Not now
        </button>
      </div>
    </div>
  );
});

interface EndProps {
  mediaItemId: number;
  title: string;
  moreLikeThis: TitleView[];
  onOpen: (id: number) => void;
  onLeave: () => void;
}

export function EndCard({ mediaItemId, title, moreLikeThis, onOpen, onLeave }: EndProps) {
  const [stars, setStars] = useState<number | null>(null);
  useEffect(() => {
    void commands.titleRating(mediaItemId).then((r) => r.status === "ok" && setStars(r.data));
  }, [mediaItemId]);
  const rate = (n: number) => {
    const next = stars === n ? null : n;
    setStars(next);
    void commands.rateTitle(mediaItemId, next);
  };
  return (
    <div className="absolute inset-0 grid place-items-center bg-black" role="dialog" aria-label="Finished">
      <div className="w-[min(620px,86%)]">
        <p className="label text-ink-faint">You watched</p>
        <h1 className="mt-2 font-serif text-[52px] leading-[1.02] text-ink">{title}</h1>
        <div className="mt-7" role="radiogroup" aria-label="Your rating">
          <p className="label mb-2 text-ink-faint">Your rating</p>
          <div className="flex gap-2">
            {[1, 2, 3, 4, 5].map((n) => (
              <button
                key={n}
                type="button"
                role="radio"
                aria-checked={stars === n}
                aria-label={`${n} out of 5`}
                onClick={() => rate(n)}
                className={`h-10 w-10 rounded-sm border text-[17px] ${
                  stars !== null && n <= stars ? "border-ink text-ink" : "border-line-strong text-ink-faint"
                }`}
              >
                {n}
              </button>
            ))}
          </div>
        </div>
        {moreLikeThis.length > 0 && (
          <section aria-label="Watch next" className="mt-9">
            <p className="label mb-2 text-ink-faint">Watch next</p>
            <ul>
              {moreLikeThis.slice(0, 4).map((t) => (
                <li key={t.id}>
                  <button
                    type="button"
                    onClick={() => onOpen(t.id)}
                    className="flex w-full items-baseline gap-3 border-t border-line-subtle py-2.5 text-left hover:bg-raised"
                  >
                    <span className="flex-1 text-[15px] text-ink">{t.title}</span>
                    <span className="tabular-nums text-[12px] text-ink-faint">{t.year ?? ""}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
        <button type="button" onClick={onLeave} className="label mt-8 hover:text-ink">
          ← Back to the film
        </button>
      </div>
    </div>
  );
}
