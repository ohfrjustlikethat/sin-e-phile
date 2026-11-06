/**
 * What the TV keyboard does to text — ADR-0043. Pure, so it is tested without a DOM.
 */

/** The grid's pages: LB and RB step through them. */
export const PAGES = [
  { name: "abc", keys: "abcdefghijklmnopqrstuvwxyz1234567890" },
  { name: "ABC", keys: "ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890" },
  { name: "#+=", keys: ".,-_@:/'!?&+#()*=%$\"~;[]{}<>|\\^`" },
] as const;

/** The grid is six keys wide, Netflix's TV layout. */
export const COLUMNS = 6;

export interface Text {
  value: string;
  /** Where the next character goes. */
  cursor: number;
}

/** A key pressed: a character, `space`, `backspace`, or the cursor moving. */
export function edit(t: Text, key: string): Text {
  const at = Math.max(0, Math.min(t.cursor, t.value.length));
  switch (key) {
    case "backspace":
      return at === 0 ? { value: t.value, cursor: 0 } : { value: t.value.slice(0, at - 1) + t.value.slice(at), cursor: at - 1 };
    case "left":
      return { value: t.value, cursor: Math.max(0, at - 1) };
    case "right":
      return { value: t.value, cursor: Math.min(t.value.length, at + 1) };
    case "space":
      // No leading space and no double space: a remote makes those by accident.
      if (at === 0 || t.value[at - 1] === " ") return { value: t.value, cursor: at };
      return { value: `${t.value.slice(0, at)} ${t.value.slice(at)}`, cursor: at + 1 };
    default:
      return { value: t.value.slice(0, at) + key + t.value.slice(at), cursor: at + key.length };
  }
}

/** The word being typed: from the last space before the cursor to the cursor. */
export function currentWord(t: Text): { start: number; word: string } {
  const before = t.value.slice(0, t.cursor);
  const start = before.lastIndexOf(" ") + 1;
  return { start, word: before.slice(start) };
}

/**
 * Word completion: whole words from `titles` (and names) that start with the word being
 * typed, longest-common first by how often they occur. At most `limit`, none when fewer
 * than two letters are typed.
 */
export function completions(t: Text, titles: string[], limit = 3): string[] {
  const { word } = currentWord(t);
  if (word.length < 2) return [];
  const w = word.toLowerCase();
  const counts = new Map<string, number>();
  for (const title of titles) {
    for (const raw of title.split(/[\s,:;.!?()"]+/)) {
      if (raw.length > w.length && raw.toLowerCase().startsWith(w)) counts.set(raw, (counts.get(raw) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([word]) => word);
}

/** Replace the word being typed with `word` and a space. */
export function complete(t: Text, word: string): Text {
  const { start } = currentWord(t);
  const value = `${t.value.slice(0, start)}${word} ${t.value.slice(t.cursor).trimStart()}`;
  return { value, cursor: start + word.length + 1 };
}

/** Recent searches: newest first, no repeats (case-insensitively), at most 8. */
export function remember(recent: string[], query: string): string[] {
  const q = query.trim();
  if (!q) return recent;
  return [q, ...recent.filter((r) => r.toLowerCase() !== q.toLowerCase())].slice(0, 8);
}
