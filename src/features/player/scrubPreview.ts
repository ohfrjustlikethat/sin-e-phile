/**
 * Which thumbnail to show for a time on the scrub bar — the page's half of
 * `sinephile_player::thumbs::tile_at`, the same arithmetic (SPEC.md Phase 11).
 */
export interface SheetLayout {
  interval: number | null;
  cols: number;
  rows: number;
  count: number;
}

export function tileAt(t: number, s: SheetLayout): { sheet: number; col: number; row: number } {
  const interval = s.interval ?? 10;
  const n = Math.floor(Math.max(0, t) / interval);
  const per = s.cols * s.rows;
  const sheet = Math.min(Math.floor(n / per), Math.max(0, s.count - 1));
  const i = Math.floor(n / per) > sheet ? per - 1 : n % per;
  return { sheet, col: i % s.cols, row: Math.floor(i / s.cols) };
}

/** Where the preview sits: centred on the pointer, kept inside the bar. */
export function previewLeft(x: number, width: number, barLeft: number, barWidth: number): number {
  return Math.min(Math.max(x - width / 2, barLeft), barLeft + barWidth - width);
}
