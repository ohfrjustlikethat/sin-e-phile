import { describe, expect, it } from "vitest";
import { complete, completions, currentWord, edit, PAGES, remember } from "./keyboardText";

const t = (value: string, cursor = value.length) => ({ value, cursor });

describe("the TV keyboard's text (ADR-0043)", () => {
  it("types, deletes and moves the cursor", () => {
    expect(edit(t("sunrse", 4), "i")).toEqual(t("sunrise", 5));
    expect(edit(t("sunrise", 3), "backspace")).toEqual(t("surise", 2));
    expect(edit(t("ab", 0), "backspace")).toEqual(t("ab", 0));
    expect(edit(t("ab", 2), "left")).toEqual(t("ab", 1));
    expect(edit(t("ab", 2), "right")).toEqual(t("ab", 2));
  });

  it("never types a leading or doubled space", () => {
    expect(edit(t(""), "space")).toEqual(t(""));
    expect(edit(t("in "), "space")).toEqual(t("in "));
    expect(edit(t("in"), "space")).toEqual(t("in "));
  });

  it("completes the word being typed from catalogue titles", () => {
    const titles = ["Kurosawa's Dreams", "Rashomon", "Akira Kurosawa: My Life", "Kurosawa", "Kuroneko"];
    expect(currentWord(t("films by kuro"))).toEqual({ start: 9, word: "kuro" });
    expect(completions(t("films by kuro"), titles)).toEqual(["Kurosawa", "Kuroneko", "Kurosawa's"]);
    expect(completions(t("k"), titles)).toEqual([]);
    expect(complete(t("films by kuro"), "Kurosawa")).toEqual(t("films by Kurosawa "));
  });

  it("keeps eight recent searches, newest first, without repeats", () => {
    let r: string[] = [];
    for (const q of ["a", "b", "A ", "c", "d", "e", "f", "g", "h", "i"]) r = remember(r, q);
    expect(r).toEqual(["i", "h", "g", "f", "e", "d", "c", "A"]);
    expect(remember(r, "   ")).toBe(r);
  });

  it("offers upper case and symbols for keys and addresses", () => {
    expect(PAGES.map((p) => p.name)).toEqual(["abc", "ABC", "#+="]);
    expect(PAGES[2].keys).toContain("@");
    expect(PAGES[2].keys).toContain("/");
  });
});
