import { Badge } from "@/design-system/primitives/Badge";

export interface Episode {
  id: string;
  series: string;
  season: number;
  episode: number;
  title: string;
  runtime: number;
  still?: string;
  progress?: number;
  watched?: boolean;
}

/**
 * Landscape card for an episode.
 *
 * Deliberately a different SHAPE from PosterCard, not a wider version of it —
 * 16:9 because an episode still is a frame, and 3:4 because a poster is a poster.
 * Uniform card shapes across content types is one of the tells §9.0 bans.
 */
export function EpisodeCard({ ep, onOpen }: { ep: Episode; onOpen?: (id: string) => void }) {
  return (
    <article className="w-[320px] shrink-0">
      <button
        type="button"
        onClick={() => onOpen?.(ep.id)}
        aria-label={`${ep.series}, season ${ep.season} episode ${ep.episode}, ${ep.title}`}
        className="relative block aspect-video w-full overflow-hidden rounded-sm border border-line-subtle bg-surface transition-colors hover:border-line-strong"
      >
        {ep.still ? (
          <img src={ep.still} alt="" loading="lazy" className="h-full w-full object-cover" />
        ) : (
          <div className="flex h-full flex-col justify-end border-l-2 border-oxblood p-4">
            <span className="font-serif text-[20px] leading-tight text-ink">{ep.title}</span>
          </div>
        )}
        {/* Same legibility scrim as PosterCard — an episode still is a frame, and
            the S00E00 label lands wherever the frame happens to be brightest. */}
        {ep.still && (
          <span
            aria-hidden
            className="pointer-events-none absolute inset-x-0 top-0 h-12 bg-gradient-to-b from-void/70 to-transparent"
          />
        )}
        <span className="absolute left-2.5 top-2 font-mono text-[9.5px] tracking-[0.1em] text-ink/85">
          S{String(ep.season).padStart(2, "0")}E{String(ep.episode).padStart(2, "0")}
        </span>
        {ep.progress !== undefined && ep.progress > 0 && (
          <span className="absolute inset-x-0 bottom-0 h-[3px] bg-line">
            <span className="block h-full bg-oxblood-bright" style={{ width: `${Math.min(1, ep.progress) * 100}%` }} />
          </span>
        )}
      </button>
      <div className="mt-2.5">
        <h3 className="font-serif text-[16px] leading-tight text-ink">{ep.title}</h3>
        <div className="mt-1 flex items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.08em] text-ink-faint">
            {ep.runtime} min
          </span>
          {ep.watched && <Badge tone="success">Watched</Badge>}
        </div>
      </div>
    </article>
  );
}
