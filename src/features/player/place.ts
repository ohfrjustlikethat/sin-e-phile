/**
 * Where an episode sits, in words (Phase 16, the author: the pause screen did not say which show or episode).
 * The show's name comes first; these say the rest.
 */

/** The catalogue's placeholder for an unnamed episode ("Episode #1.2"): says nothing the numbers do not. The same
 * rule as `episode_name` in crates/taste/src/continuing.rs, for titles that arrive unfiltered (the next episode). */
export function named(title: string | null | undefined): string | null {
  const t = (title ?? "").trim();
  if (!t || /^Episode\s*#?\s*[\d.]+$/.test(t)) return null;
  return t;
}

/** "S1 E2", or as much as is known. */
export function shortPlace(season: number | null, episode: number | null): string {
  return [season !== null ? `S${season}` : null, episode !== null ? `E${episode}` : null].filter(Boolean).join(" ");
}

/** "Season 1 · Episode 2 — Traces to Nowhere". */
export function episodeLine(p: { season: number | null; episode: number | null; episode_title: string | null }): string {
  const place = [p.season !== null ? `Season ${p.season}` : null, p.episode !== null ? `Episode ${p.episode}` : null]
    .filter(Boolean)
    .join(" · ");
  const name = named(p.episode_title);
  return [place || null, name].filter(Boolean).join(" — ");
}

/** "Next: S1 E3 · Zen, or the Skill to Catch a Killer". */
export function nextLine(n: { season: number | null; episode: number | null; title: string }): string {
  return `Next: ${[shortPlace(n.season, n.episode) || null, named(n.title)].filter(Boolean).join(" · ")}`;
}
