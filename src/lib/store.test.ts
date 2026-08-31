import { beforeEach, describe, expect, it } from "vitest";
import { DESTINATIONS, useUi } from "@/lib/store";

describe("ui store", () => {
  beforeEach(() => {
    useUi.setState({ destination: "home", railExpanded: true, settingsOpen: false });
  });

  it("exposes exactly the five destinations SPEC.md §3.1 requires, in order", () => {
    // Order matters: it is the order they appear in the rail, and §3.1 fixes it.
    expect(DESTINATIONS).toEqual(["home", "films", "tv", "watchlist", "live"]);
  });

  it("changes destination", () => {
    useUi.getState().setDestination("films");
    expect(useUi.getState().destination).toBe("films");
  });

  it("toggles the rail", () => {
    expect(useUi.getState().railExpanded).toBe(true);
    useUi.getState().toggleRail();
    expect(useUi.getState().railExpanded).toBe(false);
  });

  it("keeps settings separate from destination", () => {
    // Opening settings must not lose where the user was; closing returns there.
    useUi.getState().setDestination("watchlist");
    useUi.getState().setSettingsOpen(true);
    expect(useUi.getState().destination).toBe("watchlist");
    useUi.getState().setSettingsOpen(false);
    expect(useUi.getState().destination).toBe("watchlist");
  });
});
