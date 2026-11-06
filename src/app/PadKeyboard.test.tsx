import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { PadKeyboard, applyKey, closePadKeyboard } from "./PadKeyboard";

/** ADR-0041: a controller cannot type, so a text field gets an on-screen keyboard. */

function Field() {
  const [q, setQ] = useState("");
  return (
    <>
      <input aria-label="Search" value={q} onChange={(e) => setQ(e.target.value)} />
      <p data-testid="query">{q}</p>
      <PadKeyboard />
    </>
  );
}

describe("PadKeyboard", () => {
  afterEach(() => {
    delete document.documentElement.dataset["input"];
  });

  it("knows what each key does to the text", () => {
    expect(applyKey("sunris", "e")).toBe("sunrise");
    expect(applyKey("sunrise", "backspace")).toBe("sunris");
    expect(applyKey("", "backspace")).toBe("");
    expect(applyKey("the", "space")).toBe("the ");
    expect(applyKey("the ", "space")).toBe("the ");
    expect(applyKey("", "space")).toBe("");
  });

  it("never appears for a keyboard or mouse user", () => {
    render(<Field />);
    act(() => screen.getByLabelText("Search").focus());
    expect(screen.queryByRole("group", { name: "On-screen keyboard" })).toBeNull();
  });

  it("types into the field it opened for, and Done closes it", () => {
    document.documentElement.dataset["input"] = "pad";
    render(<Field />);
    act(() => screen.getByLabelText("Search").focus());
    expect(screen.getByRole("group", { name: "On-screen keyboard" })).toBeInTheDocument();
    expect(document.activeElement?.textContent).toBe("A"); // the D-pad starts in the keys
    for (const k of ["N", "O", "S", "F"]) fireEvent.click(screen.getByRole("button", { name: k }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(screen.getByTestId("query").textContent).toBe("nos");
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("group", { name: "On-screen keyboard" })).toBeNull();
  });

  it("B closes the keyboard rather than leaving the screen", () => {
    document.documentElement.dataset["input"] = "pad";
    render(<Field />);
    act(() => screen.getByLabelText("Search").focus());
    let closed = false;
    act(() => {
      closed = closePadKeyboard();
    });
    expect(closed).toBe(true);
    expect(screen.queryByRole("group", { name: "On-screen keyboard" })).toBeNull();
    expect(closePadKeyboard()).toBe(false); // nothing open: B means back again
  });
});
