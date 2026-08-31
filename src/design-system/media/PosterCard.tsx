import { useState } from "react";
import { Badge } from "@/design-system/primitives/Badge";

/**
 * PosterCard — the most-repeated object in the app.
 *
 * TWO STATES, and the second is not a fallback.
 *
 * ADR-0013 made TMDB optional, so a large fraction of a real library will have no
 * artwork at all. §9.4 therefore requires the artwork-free state to be *designed*
 * — "genuinely beautiful rather than a fallback", in the author's words. It is a
 * typographic title card: spine number, the title set in the editorial serif,
 * director and year in wide-tracked caps. For a film app that is arguably the
 * better card, because it selects on knowledge rather than on poster recognition.
 *
 * Card sizes are deliberately NOT uniform (§9.0): `size` exists so a rail can
 * lead with a larger card. A rail of identical tiles is the generated look the
 * brief bans.
 */

export interface PosterFilm {
  id: string;
  title: string;
  year: number;
  director: string;
  /** Catalogue number. The Criterion-ish quirk that ADR-0024 keeps. */
  spine?: number;
  /** Absent for most of a real library — see above. */
  artwork?: string;
  /** 0-1, drawn as a hairline across the bottom of the artwork. */
  progress?: number;
  availability?: "local" | "stream" | "download";
}

const AVAILABILITY: Record<
  NonNullable<PosterFilm["availability"]>,
  { label: string; tone: "success" | "info" | "neutral" }
> = {
  local: { label: "On disk", tone: "success" },
  stream: { label: "Stream", tone: "info" },
  download: { label: "Download", tone: "neutral" },
};

export function PosterCard({
  film,
  size = "md",
  onOpen,
}: {
  film: PosterFilm;
  size?: "sm" | "md" | "lead";
  onOpen?: (id: string) => void;
}) {
  const [hovered, setHovered] = useState(false);
  const width = size === "lead" ? "w-[236px]" : size === "sm" ? "w-[152px]" : "w-[176px]";
  const avail = film.availability ? AVAILABILITY[film.availability] : null;

  return (
    <article data-poster-card className={["shrink-0", width].join(" ")}>
      <button
        type="button"
        onClick={() => onOpen?.(film.id)}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onFocus={() => setHovered(true)}
        onBlur={() => setHovered(false)}
        aria-label={`${film.title}, ${film.director}, ${film.year}`}
        className={[
          "relative block w-full overflow-hidden rounded-sm border text-left",
          "aspect-[3/4] bg-surface transition-colors",
          "duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
          // Depth from the border and the surface value, never a shadow (§9.0).
          hovered ? "border-line-strong" : "border-line-subtle",
        ].join(" ")}
      >
        {film.artwork ? (
          <img
            src={film.artwork}
            alt=""
            loading="lazy"
            data-motion="scale"
            className={[
              "h-full w-full object-cover transition-transform",
              "duration-[220ms] ease-[var(--ease-standard)]",
              hovered ? "scale-[1.045]" : "scale-100",
            ].join(" ")}
          />
        ) : (
          <TypographicCard film={film} size={size} />
        )}

        {/* Legibility scrim. The spine number sits on an arbitrary frame, and a
            film still is frequently bright exactly where the number goes — the
            Potemkin and Night of the Living Dead frames both washed it out. §9.0
            bans shadows used for DEPTH; a scrim for legibility is what the hero
            already does, and it is the same fix. Artwork only: the typographic
            card controls its own background. */}
        {film.artwork && (
          <span
            aria-hidden
            className="pointer-events-none absolute inset-x-0 top-0 h-14 bg-gradient-to-b from-void/70 to-transparent"
          />
        )}

        {film.spine !== undefined && film.artwork && (
          <span className="absolute left-2.5 top-2 font-mono text-[9.5px] tracking-[0.1em] text-ink/85">
            № {String(film.spine).padStart(3, "0")}
          </span>
        )}

        {film.progress !== undefined && film.progress > 0 && (
          <span className="absolute inset-x-0 bottom-0 h-[3px] bg-line">
            <span
              className="block h-full bg-oxblood-bright"
              style={{ width: `${Math.min(1, film.progress) * 100}%` }}
            />
          </span>
        )}
      </button>

      {/* The caption repeats nothing. An artwork-free card already sets the title
          and credits INSIDE the frame, so printing them again underneath said the
          same thing twice — visible immediately once the gallery was rendered with
          and without artwork side by side. Only the availability badge is common
          to both states. */}
      <div className="mt-2.5">
        {film.artwork && (
          <>
            <h3 className="font-serif text-[17px] leading-[1.12] text-ink">{film.title}</h3>
            <div className="mt-1 flex items-baseline gap-2">
              <span className="text-[10px] uppercase tracking-[0.08em] text-ink-faint">
                {film.director} · {film.year}
              </span>
            </div>
          </>
        )}
        {avail && (
          <div className={film.artwork ? "mt-2" : ""}>
            <Badge tone={avail.tone}>{avail.label}</Badge>
          </div>
        )}
      </div>
    </article>
  );
}

/**
 * The artwork-free card (ADR-0013).
 *
 * Composed rather than filled: the spine number sits top, the title takes the
 * space it needs in the editorial serif, credits sit bottom. The oxblood rule on
 * the left is the only accent, and it is what makes the card read as *catalogued*
 * rather than as *missing something*.
 */
function TypographicCard({
  film, size,
}: {
  film: PosterFilm;
  size: "sm" | "md" | "lead";
}) {
  const titleSize =
    size === "lead" ? "text-[34px]" : size === "sm" ? "text-[21px]" : "text-[26px]";
  return (
    <div className="flex h-full flex-col justify-between border-l-2 border-oxblood bg-surface p-4">
      <span className="spine">
        {film.spine !== undefined ? `№ ${String(film.spine).padStart(3, "0")}` : "Uncatalogued"}
      </span>
      <span
        className={[
          "font-serif leading-[1.02] tracking-[-0.02em] text-ink",
          titleSize,
        ].join(" ")}
      >
        {film.title}
      </span>
      <span className="label">
        {film.director} · {film.year}
      </span>
    </div>
  );
}
