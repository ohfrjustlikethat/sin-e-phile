import { describe, expect, it } from "vitest";

/**
 * Tailwind v4 reads `text-base` as a colour as well as a size, and this theme has both a `--text-base` size and a
 * `base` colour (the page's background). It drew the "Search by meaning" heading in the background colour, invisible
 * (found on the author's install, Phase 16). Sizes are written `text-[16px]`, colours `text-ink` and the rest.
 */

// Every component's source, read by the bundler (no Node APIs in the app's types).
const sources = import.meta.glob(["../**/*.tsx", "!../**/*.test.tsx"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

describe("class names that mean two things", () => {
  it("reads the sources", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(50);
  });

  it("no component uses `text-base`", () => {
    const found = Object.entries(sources).flatMap(([file, text]) =>
      text
        .split("\n")
        .map((line, i) => ({ at: `${file}:${i + 1}`, line }))
        .filter((l) => /className=.*\btext-base\b/.test(l.line))
        .map((l) => l.at),
    );
    expect(found).toEqual([]);
  });
});
