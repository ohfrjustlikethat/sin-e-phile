import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useRecentSearches } from "@/lib/recentSearches";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  search: vi.fn(() =>
    ok({
      hits: [
        { id: 1, title: "Kurosawa", year: 2000 },
        { id: 2, title: "Kuroneko", year: 1968 },
        { id: 3, title: "Akira Kurosawa's Dreams", year: 1990 },
      ],
    }),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { PadKeyboard, applyKey, closePadKeyboard } = await import("./PadKeyboard");

/** ADR-0041 and ADR-0043: a controller cannot type, so every text field gets a keyboard. */

function Field({ label = "Search" }: { label?: string }) {
  const [q, setQ] = useState("");
  return (
    <>
      <input aria-label={label} value={q} onChange={(e) => setQ(e.target.value)} />
      <p data-testid="query">{q}</p>
      <PadKeyboard />
    </>
  );
}

const keyboard = () => screen.getByRole("group", { name: "On-screen keyboard" });
const key = (k: string) => fireEvent.click(within(keyboard()).getByRole("button", { name: k }));

describe("PadKeyboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useRecentSearches.setState({ recent: [] });
  });
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
    // An ordinary field waits for A (a click); only the search field opens on focus.
    expect(screen.queryByRole("group", { name: "On-screen keyboard" })).toBeNull();
    fireEvent.click(screen.getByLabelText("Search"));
    expect(document.activeElement?.textContent).toBe("a"); // the D-pad starts in the keys
    for (const k of ["n", "o", "s", "f"]) key(k);
    key("Delete");
    expect(screen.getByTestId("query").textContent).toBe("nos");
    fireEvent.click(within(keyboard()).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("group", { name: "On-screen keyboard" })).toBeNull();
  });

  it("types upper case and symbols, for API keys and addresses (ADR-0043)", () => {
    document.documentElement.dataset["input"] = "pad";
    render(<Field label="API key" />);
    fireEvent.click(screen.getByLabelText("API key"));
    key("Keys: ABC");
    key("A");
    key("Keys: #+=");
    key("@");
    key("Keys: abc");
    key("b");
    expect(screen.getByTestId("query").textContent).toBe("A@b");
  });

  it("B closes the keyboard rather than leaving the screen", () => {
    document.documentElement.dataset["input"] = "pad";
    render(<Field />);
    fireEvent.click(screen.getByLabelText("Search"));
    let closed = false;
    act(() => {
      closed = closePadKeyboard();
    });
    expect(closed).toBe(true);
    expect(screen.queryByRole("group", { name: "On-screen keyboard" })).toBeNull();
    expect(closePadKeyboard()).toBe(false); // nothing open: B means back again
  });

  it("on search: recent searches first, then suggestions and word completion", async () => {
    document.documentElement.dataset["input"] = "pad";
    useRecentSearches.setState({ recent: ["nosferatu", "late spring"] });
    render(<Field label="Search the catalogue" />);
    act(() => screen.getByLabelText("Search the catalogue").focus());
    const recent = within(keyboard()).getByRole("region", { name: "Recent searches" });
    fireEvent.click(within(recent).getByRole("button", { name: "late spring" }));
    expect(screen.getByTestId("query").textContent).toBe("late spring");
    for (let i = 0; i < "late spring".length; i++) key("Delete");

    for (const k of ["k", "u", "r", "o"]) key(k);
    await waitFor(() => expect(commands.search).toHaveBeenCalledWith("kuro", 6));
    const words = await within(keyboard()).findByRole("region", { name: "Complete the word" });
    fireEvent.click(within(words).getByRole("button", { name: "Kurosawa" }));
    expect(screen.getByTestId("query").textContent).toBe("Kurosawa ");

    const suggestions = within(keyboard()).getByRole("region", { name: "Suggestions" });
    fireEvent.click(within(suggestions).getByRole("button", { name: /Kuroneko/ }));
    expect(screen.getByTestId("query").textContent).toBe("Kuroneko");
    fireEvent.click(within(keyboard()).getByRole("button", { name: "Done" }));
    expect(useRecentSearches.getState().recent[0]).toBe("Kuroneko");
  });
});
