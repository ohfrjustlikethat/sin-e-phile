import { describe, expect, it } from "vitest";
import { airsLine } from "./airing";

// Friday 6 November 2026, 10:00 UTC.
const NOW = new Date(Date.UTC(2026, 10, 6, 10, 0));
const at = (d: number, h: number, m = 0) => Date.UTC(2026, 10, d, h, m) / 1000;

describe("airsLine (SPEC.md Phase 11)", () => {
  it("says when, as a person would", () => {
    expect(airsLine(7, at(6, 18, 30), NOW, "UTC")).toBe("Episode 7 airs today at 18:30");
    expect(airsLine(7, at(7, 9), NOW, "UTC")).toBe("Episode 7 airs tomorrow at 09:00");
    expect(airsLine(7, at(12, 15), NOW, "UTC")).toBe("Episode 7 airs Thursday");
    expect(airsLine(7, at(20, 15), NOW, "UTC")).toBe("Episode 7 airs on 20 November");
    expect(airsLine(7, at(5, 15), NOW, "UTC")).toBe("Episode 7 has aired");
  });

  it("counts days in the viewer's time zone, not UTC", () => {
    // 01:00 UTC on the 7th is still the 6th in New York: "today", not "tomorrow".
    expect(airsLine(3, at(7, 1), new Date(Date.UTC(2026, 10, 6, 20)), "America/New_York")).toBe(
      "Episode 3 airs today at 20:00",
    );
  });
});
