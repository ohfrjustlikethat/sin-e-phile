import { describe, expect, it } from "vitest";
import { previewLeft, tileAt } from "./scrubPreview";

const S = { interval: 10, cols: 10, rows: 10, count: 8 };

describe("scrub thumbnails (SPEC.md Phase 11)", () => {
  it("finds the tile the Rust side made for that time", () => {
    // The same cases as crates/player/src/thumbs.rs a_time_finds_its_tile.
    expect(tileAt(0, S)).toEqual({ sheet: 0, col: 0, row: 0 });
    expect(tileAt(125, S)).toEqual({ sheet: 0, col: 2, row: 1 });
    expect(tileAt(1000, S)).toEqual({ sheet: 1, col: 0, row: 0 });
    expect(tileAt(50_000, { ...S, count: 1 })).toEqual({ sheet: 0, col: 9, row: 9 });
  });

  it("keeps the preview inside the bar", () => {
    expect(previewLeft(500, 160, 20, 1000)).toBe(420);
    expect(previewLeft(30, 160, 20, 1000)).toBe(20);
    expect(previewLeft(1015, 160, 20, 1000)).toBe(860);
  });
});
