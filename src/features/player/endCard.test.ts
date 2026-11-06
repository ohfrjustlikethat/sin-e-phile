import { describe, expect, it } from "vitest";
import { isCreditsChapter, moment } from "./endCard";

const at = (position: number, extra: Partial<Parameters<typeof moment>[0]> = {}) =>
  moment({ hasNext: true, position, duration: 1440, ended: false, chapterTitle: null, ...extra });

describe("what comes next (SPEC.md Phase 11, ADR-0043)", () => {
  it("offers the next episode in the last 30 seconds, not before", () => {
    expect(at(1400)).toBe("none");
    expect(at(1410)).toBe("next-episode");
    expect(at(1440, { ended: true })).toBe("next-episode");
  });

  it("offers it as soon as the credits chapter starts, when the file marks one", () => {
    expect(at(1300, { chapterTitle: "Ending" })).toBe("next-episode");
    expect(at(1300, { chapterTitle: "Part B" })).toBe("none");
  });

  it("lets a film's credits play, then shows the end card", () => {
    const film = { hasNext: false, duration: 5700, chapterTitle: "End Credits" };
    expect(moment({ ...film, position: 5600, ended: false })).toBe("none");
    expect(moment({ ...film, position: 5700, ended: true })).toBe("end-card");
  });

  it("knows credit chapters by their names", () => {
    expect(isCreditsChapter("Credits")).toBe(true);
    expect(isCreditsChapter("ED")).toBe(true);
    expect(isCreditsChapter("Outro")).toBe(true);
    expect(isCreditsChapter("Editing Room")).toBe(false);
    expect(isCreditsChapter(null)).toBe(false);
  });
});
