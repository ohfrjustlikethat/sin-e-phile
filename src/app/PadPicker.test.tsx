import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { PadPicker, closePadPicker, openPadPicker } from "./PadPicker";
import { adjustValue } from "@/lib/gamepad";

/** Phase 16, ADR-0041: a pad cannot open a native select, so A opens this list instead. */

function Adapter({ onBack }: { onBack?: () => void }) {
  const [adapter, setAdapter] = useState("");
  return (
    <>
      <label htmlFor="adapter">Use only this network adapter</label>
      <select id="adapter" value={adapter} onChange={(e) => setAdapter(e.target.value)}>
        <option value="">Any connection</option>
        <option value="Ethernet">Ethernet</option>
        <option value="VPN">VPN</option>
      </select>
      <p data-testid="adapter">{adapter || "any"}</p>
      <button type="button" onClick={onBack}>Screen beneath</button>
      <PadPicker />
    </>
  );
}

const select = () => screen.getByLabelText("Use only this network adapter", { selector: "select" }) as HTMLSelectElement;
const list = () => screen.getByRole("dialog", { name: "Use only this network adapter" });

describe("PadPicker", () => {
  it("lists the select's options, with focus on the one chosen now", () => {
    render(<Adapter />);
    act(() => void openPadPicker(select()));
    const options = within(list()).getAllByRole("radio");
    expect(options.map((o) => o.textContent)).toEqual(["Any connectionChosen", "Ethernet", "VPN"]);
    expect(document.activeElement).toBe(options[0]);
  });

  it("choosing sets the select as a person would, so its onChange runs, and focus goes back", () => {
    render(<Adapter />);
    act(() => void openPadPicker(select()));
    fireEvent.click(within(list()).getByRole("radio", { name: "VPN" }));
    expect(screen.getByTestId("adapter").textContent).toBe("VPN");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(select());
  });

  it("B (closePadPicker) leaves it unchanged, and says whether a list was open", () => {
    render(<Adapter />);
    expect(closePadPicker()).toBe(false);
    act(() => void openPadPicker(select()));
    let closed = false;
    act(() => void (closed = closePadPicker()));
    expect(closed).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByTestId("adapter").textContent).toBe("any");
  });

  it("Escape closes the list and goes no further, so the screen beneath does not go back", () => {
    const beneath = vi.fn();
    render(<Adapter />);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && beneath();
    window.addEventListener("keydown", onKey);
    act(() => void openPadPicker(select()));
    act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(beneath).not.toHaveBeenCalled();
    // With no list open, Escape is the screen's again.
    act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(beneath).toHaveBeenCalledTimes(1);
    window.removeEventListener("keydown", onKey);
  });
});

describe("a slider by pad, in React (Phase 16)", () => {
  function Size() {
    const [size, setSize] = useState(46);
    return (
      <>
        <input aria-label="Size" type="range" min={30} max={80} step={2} value={size}
          onChange={(e) => setSize(Number(e.target.value))} />
        <p data-testid="size">{size}</p>
      </>
    );
  }

  it("Right on the focused slider reaches its onChange, as the arrow key would", () => {
    render(<Size />);
    screen.getByLabelText("Size").focus();
    act(() => void adjustValue("right"));
    act(() => void adjustValue("right"));
    expect(screen.getByTestId("size").textContent).toBe("50");
  });
});
