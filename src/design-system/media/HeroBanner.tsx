import type { ReactNode } from "react";

export interface HeroFilm {
  title: string;
  altTitle?: string;
  year: number;
  runtime: number;
  director: string;
  country: string;
  spine?: number;
  artwork?: string;
  /** Honest reason this is here — Phase 17 supplies it. */
  reason?: string;
}

/**
 * The billboard.
 *
 * 74vh, per the author's choice of Take A's hero on Take B's system (ADR-0024).
 * That height is not cosmetic: it puts the first rail below the fold, so Home
 * opens with one curated statement rather than a browsing grid.
 *
 * The gradient here IS permitted. §9.0 bans gradients as DECORATION; this one is
 * a legibility scrim over artwork, without which white text over a bright frame
 * is unreadable. It is not the player-chrome scrim, which ADR-0020 removed.
 */
export function HeroBanner({ film, actions }: { film: HeroFilm; actions?: ReactNode }) {
  return (
    <div className="grid border-b border-line" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
      <div className="border-r border-line-subtle pt-8 pl-6">
        {film.spine !== undefined && (
          <span className="spine">№ {String(film.spine).padStart(3, "0")}</span>
        )}
      </div>

      <div className="relative" style={{ height: "var(--hero-height)" }}>
        {film.artwork ? (
          <img
            src={film.artwork}
            alt=""
            className="h-full w-full object-cover"
            style={{ filter: "saturate(0.9) contrast(1.05)" }}
          />
        ) : (
          <div className="h-full w-full bg-surface" />
        )}
        <div
          aria-hidden
          className="absolute inset-0"
          style={{
            background:
              "linear-gradient(to top, rgba(0,0,0,0.92) 0%, rgba(0,0,0,0.18) 58%, transparent 100%)",
          }}
        />
        <div className="absolute inset-x-0 bottom-0 z-10 p-9">
          {film.reason && <div className="label mb-3">{film.reason}</div>}
          <h1 className="mb-3 max-w-[820px] font-display text-[62px] font-extrabold uppercase leading-[0.92] tracking-[-0.045em] text-ink">
            {film.title}
          </h1>
          {film.altTitle && (
            <div className="mb-4 font-serif text-[20px] text-ink-muted">{film.altTitle}</div>
          )}
          <div className="flex gap-5 text-[11px] uppercase tracking-[0.06em] text-ink-muted">
            <span>{film.year}</span>
            <span>{Math.floor(film.runtime / 60)}h {film.runtime % 60}m</span>
            <span>{film.director}</span>
            <span>{film.country}</span>
          </div>
          {actions && <div className="mt-6 flex gap-2.5">{actions}</div>}
        </div>
      </div>
    </div>
  );
}
