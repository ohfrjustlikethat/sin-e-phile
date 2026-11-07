import { beforeEach, describe, expect, it } from "vitest";
import { create } from "zustand";
import { persist } from "zustand/middleware";
import { profileStorage, setStorageProfile, switchStores } from "./profileStorage";

interface Choice {
  audio: string | null;
  setAudio: (a: string | null) => void;
}

function store() {
  return create<Choice>()(
    persist((set) => ({ audio: null, setAudio: (audio) => set({ audio }) }), {
      name: "test.choice",
      storage: profileStorage,
    }),
  );
}

describe("personal browser storage (Phase 14)", () => {
  beforeEach(() => {
    window.localStorage.clear();
    setStorageProfile(1);
  });

  it("keeps each profile's values apart, and a switch shows the new profile's own", async () => {
    const s = store();
    await switchStores(1, [s]);
    s.getState().setAudio("ja");
    await switchStores(2, [s]);
    expect(s.getState().audio, "profile 2 starts clean, not with 1's language").toBeNull();
    s.getState().setAudio("hi");
    await switchStores(1, [s]);
    expect(s.getState().audio).toBe("ja");
    await switchStores(2, [s]);
    expect(s.getState().audio).toBe("hi");
  });

  it("the first profile to load keeps what was saved before profiles, and only it", async () => {
    window.localStorage.setItem("test.choice", JSON.stringify({ state: { audio: "fr" }, version: 0 }));
    const s = store();
    await switchStores(1, [s]);
    expect(s.getState().audio).toBe("fr");
    await switchStores(2, [s]);
    expect(s.getState().audio, "claimed once, by the first").toBeNull();
  });
});
