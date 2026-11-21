import { useRef, useState } from "react";

/**
 * StarRating — the viewer's own rating, Letterboxd's way (ADR-0056, the author's answers): five stars in half steps,
 * 0.5 to 5, filled in oxblood, no reviews. Values are halves, 1 to 10; `null` is not rated.
 *
 * A native `range` input (0 to 10) lies over the stars and carries everything but the mouse: the arrow keys, the pad's
 * Left and Right (`adjustValue` in `src/lib/gamepad.ts` moves any focused range by one step), the focus ring, and what a
 * screen reader says ("3½ stars"). It takes no pointer events, so the mouse reaches the stars underneath: the left half
 * of a star is a half star, hovering previews, and clicking the rating already given clears it. Down to 0 by key or pad
 * clears it too.
 */
export function StarRating({
  value,
  onChange,
  label = "Your rating",
  size = "md",
}: {
  value: number | null;
  onChange: (halves: number | null) => void;
  label?: string;
  size?: "sm" | "md";
}) {
  const [hover, setHover] = useState<number | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const px = size === "md" ? 24 : 18;
  const at = (clientX: number) => {
    const r = box.current?.getBoundingClientRect();
    if (!r || r.width === 0) return 1;
    return Math.min(10, Math.max(1, Math.ceil(((clientX - r.left) / r.width) * 10)));
  };
  return (
    <div
      ref={box}
      className="relative inline-flex cursor-pointer select-none"
      onMouseMove={(e) => setHover(at(e.clientX))}
      onMouseLeave={() => setHover(null)}
      onClick={(e) => {
        const h = at(e.clientX);
        onChange(h === value ? null : h);
      }}
    >
      <Stars halves={hover ?? value ?? 0} px={px} />
      <input
        type="range"
        min={0}
        max={10}
        step={1}
        value={value ?? 0}
        aria-label={label}
        aria-valuetext={value ? starWords(value) : "Not rated"}
        onChange={(e) => onChange(Number(e.target.value) || null)}
        className="pointer-events-none absolute inset-0 h-full w-full appearance-none opacity-0"
      />
    </div>
  );
}

/** Stars that only show: a show's average of its rated seasons ("your average"), which may fall between halves. */
export function StarsShown({ halves, label, size = "sm" }: { halves: number; label: string; size?: "sm" | "md" }) {
  return (
    <span role="img" aria-label={label} className="inline-flex">
      <Stars halves={halves} px={size === "md" ? 24 : 16} />
    </span>
  );
}

function Stars({ halves, px }: { halves: number; px: number }) {
  return (
    <span className="inline-flex gap-[3px]" aria-hidden>
      {[0, 1, 2, 3, 4].map((i) => (
        <Star key={i} fill={Math.min(2, Math.max(0, halves - i * 2)) / 2} px={px} />
      ))}
    </span>
  );
}

const STAR = "M12 2.5l2.9 6.1 6.6.8-4.9 4.6 1.3 6.6L12 17.3l-5.9 3.3 1.3-6.6-4.9-4.6 6.6-.8z";

function Star({ fill, px }: { fill: number; px: number }) {
  return (
    <span className="relative inline-block" style={{ width: px, height: px }}>
      <svg viewBox="0 0 24 24" width={px} height={px} className="absolute inset-0 text-ink-faint">
        <path d={STAR} fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
      </svg>
      {fill > 0 && (
        <span className="absolute inset-y-0 left-0 overflow-hidden" style={{ width: `${fill * 100}%` }}>
          <svg viewBox="0 0 24 24" width={px} height={px} className="text-oxblood-bright">
            <path d={STAR} fill="currentColor" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
          </svg>
        </span>
      )}
    </span>
  );
}

/** "3½ stars", "½ star", "1 star": what a rating says in words. */
export function starWords(halves: number): string {
  const whole = Math.floor(halves / 2);
  const half = halves % 2 === 1;
  const n = `${whole > 0 ? whole : ""}${half ? "½" : ""}`;
  return `${n} ${halves <= 2 ? "star" : "stars"}`;
}
