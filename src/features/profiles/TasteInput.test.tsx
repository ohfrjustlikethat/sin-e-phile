import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  tasteInput: vi.fn(() =>
    ok({ statements: { pace: -1, ambiguity: null, era: null, subtitles: 1, runtime: null, form: null }, love: "Rain." }),
  ),
  setTasteInput: vi.fn(() => ok(null)),
  tastePreview: vi.fn(() => ok([{ id: 7, title: "Sunrise", year: 1927, made_by: "F. W. Murnau" }])),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { TasteInput } = await import("./TasteInput");

describe("What you like (Phase 14)", () => {
  it("shows what was said, saves changes, and shows where the taste points", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <TasteInput />
      </QueryClientProvider>,
    );
    const pace = await screen.findByRole("radiogroup", { name: "Pace" });
    await waitFor(() => expect(within(pace).getByRole("radio", { name: "Slow and patient" })).toHaveAttribute("aria-checked", "true"));
    expect(screen.getByDisplayValue("Rain.")).toBeInTheDocument();
    // Choose; choosing the chosen one again clears it.
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Form" })).getByRole("radio", { name: "Experimental" }));
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Subtitles" })).getByRole("radio", { name: "Happily" }));
    fireEvent.change(screen.getByLabelText("What you love, in your own words"), { target: { value: "  Long takes.  " } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(commands.setTasteInput).toHaveBeenCalledWith(
        { pace: -1, ambiguity: null, era: null, subtitles: null, runtime: null, form: 1 },
        "Long takes.",
      ),
    );
    expect(await screen.findByText("Sunrise")).toBeInTheDocument();
    expect(screen.getByText("1927 · F. W. Murnau")).toBeInTheDocument();
  });
});
