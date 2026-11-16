import { beforeEach, describe, expect, it } from "vitest";
import { useUi } from "@/lib/store";
import { handleBack } from "./backKey";

const press = (key: string, target: EventTarget = window, prevented = false) => {
  const event = new KeyboardEvent("keydown", { key, cancelable: true, bubbles: true });
  if (prevented) event.preventDefault();
  Object.defineProperty(event, "target", { value: target });
  return { handled: handleBack(event, false), event };
};

describe("The one way back from the keyboard (Phase 16)", () => {
  beforeEach(() => {
    useUi.setState({
      tab: "home",
      destination: "home",
      trails: { home: [], films: [], tv: [], watchlist: [], live: [], search: [] },
      detail: null,
      settingsOpen: false,
      downloadsOpen: false,
      reviewOpen: false,
      tasteOpen: false,
      pickerOpen: false,
      playing: null,
      mini: false,
    });
  });

  it("closes Downloads with Escape, but from inside a field only leaves the field", () => {
    useUi.setState({ downloadsOpen: true });
    const field = document.createElement("input");
    document.body.append(field);
    field.focus();
    press("Escape", field);
    expect(useUi.getState().downloadsOpen).toBe(true);
    expect(document.activeElement).not.toBe(field);
    press("Escape");
    expect(useUi.getState().downloadsOpen).toBe(false);
    field.remove();
  });

  it("steps back a title page with Escape or Backspace, and Backspace in a field is a letter", () => {
    useUi.getState().openDetail(1);
    useUi.getState().openDetail(2);
    press("Backspace");
    expect(useUi.getState().detail).toBe(1);
    const field = document.createElement("textarea");
    expect(press("Backspace", field).handled).toBe(false);
    expect(useUi.getState().detail).toBe(1);
    press("Escape");
    expect(useUi.getState().detail).toBeNull();
  });

  it("leaves search from its own box", () => {
    useUi.getState().goTab("search");
    press("Escape", document.createElement("input"));
    expect(useUi.getState().tab).toBe("home");
  });

  it("does nothing when a menu, the player or a picker already has the key", () => {
    useUi.getState().openDetail(4);
    press("Escape", window, true);
    useUi.setState({ playing: { target: "x", title: "x", mediaItemId: null, start: null } });
    press("Escape");
    useUi.setState({ playing: null, pickerOpen: true });
    press("Escape");
    expect(useUi.getState().detail).toBe(4);
  });
});
