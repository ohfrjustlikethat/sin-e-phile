/**
 * Rating.
 *
 * Numeric, not stars. Stars quantise a 7.8 into "four stars" and lose the
 * distinction between a 7.8 and a 6.2 that a cinephile actually cares about —
 * and a row of gold stars would put decorative colour on the chrome, which §9.1
 * reserves for the artwork.
 *
 * The source is named because IMDb, TMDB and AniList disagree, and an unattributed
 * number invites the reader to assume the wrong one.
 */
export function Rating({
  value, source, max = 10,
}: {
  value: number;
  source: "IMDb" | "TMDB" | "AniList";
  max?: number;
}) {
  return (
    <span className="inline-flex items-baseline gap-1.5">
      <span className="font-mono text-[13px] text-ink">{value.toFixed(1)}</span>
      <span className="text-[10px] text-ink-faint">/ {max}</span>
      <span className="label ml-1">{source}</span>
    </span>
  );
}
