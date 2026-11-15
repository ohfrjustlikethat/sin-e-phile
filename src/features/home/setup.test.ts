import { describe, expect, it } from "vitest";
import { setupLines, setupProgress, showSetup } from "./setup";

const nothing = { folders: 0, addons: 0, subtitlesKey: false, adapterBound: false };
const everything = { folders: 2, addons: 1, subtitlesKey: true, adapterBound: true };

describe("the Finish setting up card (SPEC.md Appendix F, I38)", () => {
  it("a fresh install has four lines, none done, and the card shows", () => {
    const lines = setupLines(nothing);
    expect(lines.map((l) => l.key)).toEqual(["library", "source", "subtitles", "downloads"]);
    expect(lines.every((l) => !l.done)).toBe(true);
    expect(showSetup(lines, false)).toBe(true);
    expect(setupProgress(lines)).toBe("0 of 4 done");
  });

  it("says plainly that the app ships no sources, and names none", () => {
    const source = setupLines(nothing).find((l) => l.key === "source")!;
    expect(source.detail).toMatch(/no sources of its own/);
    expect(source.optional).toBe(false);
    expect(source.section).toBe("Add a source");
  });

  it("counts what is there in words", () => {
    const lines = setupLines({ ...nothing, folders: 1, addons: 3 });
    expect(lines[0]!.detail).toBe("1 folder added");
    expect(lines[1]!.detail).toBe("3 add-ons installed");
    expect(setupProgress(lines)).toBe("2 of 4 done");
  });

  it("goes away when everything is done, and when the viewer hides it", () => {
    expect(showSetup(setupLines(everything), false)).toBe(false);
    expect(showSetup(setupLines(nothing), true)).toBe(false);
    // Only optional lines left: still offered, until hidden.
    expect(showSetup(setupLines({ ...everything, subtitlesKey: false }), false)).toBe(true);
  });

  it("downloads are ready without a VPN; binding one is the optional part", () => {
    const downloads = setupLines(nothing).find((l) => l.key === "downloads")!;
    expect(downloads.optional).toBe(true);
    expect(downloads.detail).toMatch(/^Ready, with the built-in engine/);
  });
});
