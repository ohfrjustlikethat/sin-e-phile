import { beforeEach, describe, expect, it } from "vitest";
import { DESTINATIONS, useUi } from "@/lib/store";

describe("ui store", () => {
  const fresh = useUi.getState();
  beforeEach(() => {
    useUi.setState({
      destination: "home",
      tab: "home",
      trails: { home: [], films: [], tv: [], watchlist: [], live: [], search: [] },
      detail: null,
      settingsOpen: false,
      downloadsOpen: false,
      reviewOpen: false,
      tasteOpen: false,
      searchQuery: "",
    });
  });

  it("exposes exactly the five destinations SPEC.md §3.1 requires, in order", () => {
    // Order matters: it is the order of the masthead tabs, and §3.1 fixes it.
    expect(DESTINATIONS).toEqual(["home", "films", "tv", "watchlist", "live"]);
  });

  it("changes destination", () => {
    useUi.getState().setDestination("films");
    expect(useUi.getState().destination).toBe("films");
  });

  it("keeps settings separate from destination", () => {
    // Opening settings must not lose where the user was; closing returns there.
    useUi.getState().setDestination("watchlist");
    useUi.getState().setSettingsOpen(true);
    expect(useUi.getState().destination).toBe("watchlist");
    useUi.getState().setSettingsOpen(false);
    expect(useUi.getState().destination).toBe("watchlist");
  });

  describe("each tab keeps its trail (Phase 16)", () => {
    const ui = () => useUi.getState();

    it("walks back through the title pages opened from a tab", () => {
      ui().openDetail(1);
      ui().openDetail(2);
      expect(ui().detail).toBe(2);
      expect(ui().back()).toBe(true);
      expect(ui().detail).toBe(1);
      expect(ui().back()).toBe(true);
      expect(ui().detail).toBeNull();
      expect(ui().back()).toBe(false);
    });

    it("comes back to the page a tab was left on, and to its start when pressed again", () => {
      ui().openDetail(1);
      ui().goTab("films");
      expect(ui().detail).toBeNull();
      ui().openDetail(7);
      ui().goTab("home");
      expect(ui().detail).toBe(1);
      ui().goTab("films");
      expect(ui().detail).toBe(7);
      ui().goTab("films");
      expect(ui().detail).toBeNull();
      expect(ui().trails.films).toEqual([]);
      expect(ui().trails.home).toEqual([1]);
    });

    it("closes Settings or Downloads before leaving the page under them", () => {
      ui().openDetail(3);
      ui().setSettingsOpen(true);
      expect(ui().back()).toBe(true);
      expect(ui().settingsOpen).toBe(false);
      expect(ui().detail).toBe(3);
      ui().setDownloadsOpen(true);
      // A tab pressed from Downloads goes where that tab was, not to its start.
      ui().goTab("home");
      expect(ui().downloadsOpen).toBe(false);
      expect(ui().detail).toBe(3);
    });

    it("keeps search's results and trail, and leaves search for the destination", () => {
      ui().goTab("films");
      ui().goTab("search");
      ui().setSearchQuery("heat");
      ui().openDetail(9);
      ui().goTab("tv");
      ui().goTab("search");
      expect(ui().detail).toBe(9);
      expect(ui().searchQuery).toBe("heat");
      ui().back();
      expect(ui().detail).toBeNull();
      ui().back();
      expect(ui().tab).toBe("tv");
      expect(ui().destination).toBe("tv");
    });

    it("goes to the search box itself from a shortcut, keeping the words", () => {
      ui().goTab("search");
      ui().setSearchQuery("heat");
      ui().openDetail(9);
      ui().goTab("home");
      ui().searchNow();
      expect(ui().tab).toBe("search");
      expect(ui().detail).toBeNull();
      expect(ui().searchQuery).toBe("heat");
    });

    it("leaves the destination alone while searching", () => {
      ui().goTab("watchlist");
      ui().goTab("search");
      expect(ui().destination).toBe("watchlist");
    });
  });

  it("starts from a clean store between tests", () => {
    expect(fresh.tab).toBe("home");
  });
});
