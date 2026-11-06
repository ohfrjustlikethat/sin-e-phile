/**
 * When the player offers what comes next — SPEC.md Phase 11 and ADR-0043.
 *
 * - An episode with a next one: the next-episode card, in the last 30 seconds, or as soon
 *   as a chapter named like the credits begins (when the file says where they start), or
 *   at the end.
 * - A film, or a last episode: the credits play in full, then the end card (ADR-0043:
 *   "credits, then a card"), so nothing interrupts them.
 */
export type Moment = "none" | "next-episode" | "end-card";

/** Chapter titles that mean the credits have started. */
const CREDITS = /\b(credits?|ending|end titles|outro|ed)\b/i;

export function isCreditsChapter(title: string | null | undefined): boolean {
  return !!title && CREDITS.test(title);
}

export interface Where {
  hasNext: boolean;
  position: number | null;
  duration: number | null;
  ended: boolean;
  chapterTitle: string | null;
}

/** The seconds before the end at which the next-episode card appears. */
export const NEXT_CARD_SECONDS = 30;

export function moment(w: Where): Moment {
  if (w.hasNext) {
    const left = w.position !== null && w.duration !== null ? w.duration - w.position : null;
    if (w.ended || isCreditsChapter(w.chapterTitle) || (left !== null && left <= NEXT_CARD_SECONDS)) {
      return "next-episode";
    }
    return "none";
  }
  return w.ended ? "end-card" : "none";
}
