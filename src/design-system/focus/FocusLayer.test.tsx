import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { FocusLayer, useScreenHints } from "./FocusLayer";

/** ADR-0042: the ring shows for keyboard and pad, never the mouse; hints follow the screen. */

const html = document.documentElement;

function Screen() {
  useScreenHints([
    ["A", "Play"],
    ["B", "Back"],
  ]);
  return null;
}

describe("FocusLayer", () => {
  afterEach(() => {
    delete html.dataset["input"];
    delete html.dataset["focusVisible"];
  });

  it("a key shows focus, the mouse hides it", () => {
    render(<FocusLayer />);
    expect(html.dataset["focusRing"]).toBe("");
    fireEvent.keyDown(window, { key: "Tab" });
    expect(html.dataset["focusVisible"]).toBe("true");
    fireEvent.pointerDown(window);
    expect(html.dataset["focusVisible"]).toBe("false");
  });

  it("draws one ring element outside the app root", () => {
    render(<FocusLayer />);
    expect(document.body.querySelectorAll(":scope > [data-focus-ring-el]")).toHaveLength(1);
  });

  it("shows the button hints only with a pad, and the current screen's", async () => {
    render(
      <>
        <FocusLayer />
        <Screen />
      </>,
    );
    expect(screen.queryByRole("note", { name: "Controller buttons" })).toBeNull();
    await act(async () => {
      html.dataset["input"] = "pad";
      await Promise.resolve(); // let the MutationObserver deliver
    });
    const bar = await screen.findByRole("note", { name: "Controller buttons" });
    expect(bar).toHaveTextContent("Play");
    expect(bar).not.toHaveTextContent("Search");
  });
});
