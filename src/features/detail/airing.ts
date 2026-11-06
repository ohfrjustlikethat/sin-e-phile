/**
 * "Episode 7 airs Thursday" — SPEC.md Phase 11, ADR-0030. AniList knows the schedule before
 * the catalogue lists the episode, so this reads the schedule, not the episode list.
 *
 * In the viewer's own time zone; `timeZone` is for tests.
 */
export function airsLine(episode: number, airsAt: number, now: Date, timeZone?: string): string {
  const at = new Date(airsAt * 1000);
  const opts = timeZone ? { timeZone } : {};
  const day = (d: Date) => {
    const p = new Intl.DateTimeFormat("en-CA", { ...opts, year: "numeric", month: "2-digit", day: "2-digit" }).format(d);
    const [y, m, dd] = p.split("-").map(Number);
    return Date.UTC(y!, m! - 1, dd!) / 86_400_000;
  };
  const days = day(at) - day(now);
  const time = new Intl.DateTimeFormat("en-GB", { ...opts, hour: "2-digit", minute: "2-digit" }).format(at);
  const ep = `Episode ${episode}`;
  if (at.getTime() < now.getTime()) return `${ep} has aired`;
  if (days === 0) return `${ep} airs today at ${time}`;
  if (days === 1) return `${ep} airs tomorrow at ${time}`;
  if (days < 7) return `${ep} airs ${new Intl.DateTimeFormat("en-GB", { ...opts, weekday: "long" }).format(at)}`;
  return `${ep} airs on ${new Intl.DateTimeFormat("en-GB", { ...opts, day: "numeric", month: "long" }).format(at)}`;
}
