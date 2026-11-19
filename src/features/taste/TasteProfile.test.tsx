import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  tasteProfile: vi.fn(() =>
    ok({
      modes: [
        {
          anchor: 11,
          share: 0.62,
          sentence: "You gravitate toward horror, mostly from the 1970s and 80s (Halloween, Alien, The Thing).",
          examples: [{ id: 501, title: "Halloween", year: 1978, made_by: "John Carpenter" }],
          correction: null,
        },
        {
          anchor: 22,
          share: 0.38,
          sentence: "You also love documentary, mostly from the 2010s.",
          examples: [],
          correction: "not_me",
        },
      ],
      axes: ["You tend to walk out of war."],
    }),
  ),
  correctTaste: vi.fn(() => ok(null)),
  home: vi.fn(() =>
    ok([{ key: "taste", title: "Where your taste points", items: [
      { id: 9, title: "The Fog", year: 1980, made_by: null, runtime_minutes: 89, note: null },
    ] }]),
  ),
  profiles: vi.fn(() => ok([])),
  activeProfile: vi.fn(() => ok(null)),
  // Phase 18: Comfort to Adventure.
  discoveryBudget: vi.fn(() => ok(0.5 as number | null)),
  setDiscoveryBudget: vi.fn((_v: number | null) => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { TasteProfile, shareWords, adventureWord } = await import("./TasteProfile");

describe("Your taste (Phase 15)", () => {
  it("says each mode, its share and the leanings, and keeps corrections", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <TasteProfile />
      </QueryClientProvider>,
    );
    expect(await screen.findByText(/You gravitate toward horror/)).toBeInTheDocument();
    expect(screen.getByText("62% of your taste")).toBeInTheDocument();
    expect(screen.getByText("You tend to walk out of war.")).toBeInTheDocument();
    const firstGroup = screen.getByRole("radiogroup", { name: "Taste 1" });
    await waitFor(() => expect(within(firstGroup).getByRole("radio", { name: "As it is" })).toHaveFocus());
    const second = screen.getByRole("radiogroup", { name: "Taste 2" });
    expect(within(second).getByRole("radio", { name: "Not me" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Taste 1" })).getByRole("radio", { name: "More of this" }));
    await waitFor(() => expect(commands.correctTaste).toHaveBeenCalledWith(11, "more"));
    fireEvent.click(within(second).getByRole("radio", { name: "As it is" }));
    await waitFor(() => expect(commands.correctTaste).toHaveBeenCalledWith(22, null));
    // Phase 16: the films as cards that open, and Home's taste rail as it now stands.
    const card = screen.getByRole("button", { name: /Halloween.*1978 · John Carpenter/ });
    fireEvent.click(card);
    expect(useUi.getState().detail).toBe(501);
    expect(await screen.findByText("The Fog")).toBeInTheDocument();
    expect(commands.home).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Change what you said" })).toBeInTheDocument();
  });

  it("says plainly when there is nothing yet", async () => {
    commands.tasteProfile.mockImplementationOnce(() => ok({ modes: [], axes: [] }));
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <TasteProfile />
      </QueryClientProvider>,
    );
    expect(await screen.findByText("Not enough to go on yet")).toBeInTheDocument();
    expect(shareWords(0.004)).toBe("under 1% of your taste");
  });

  it("Comfort to Adventure is saved as it is moved, and said in place (Phase 18)", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <TasteProfile />
      </QueryClientProvider>,
    );
    const slider = await screen.findByRole("slider", { name: "Comfort to Adventure" });
    expect(slider).toHaveValue("50");
    expect(screen.getByText("Balanced")).toBeInTheDocument();
    fireEvent.change(slider, { target: { value: "85" } });
    expect(screen.getByText("Adventure")).toBeInTheDocument();
    await waitFor(() => expect(commands.setDiscoveryBudget).toHaveBeenCalledWith(0.85));
    expect(await screen.findByText("Saved: Home follows")).toBeInTheDocument();
    expect([adventureWord(0), adventureWord(0.5), adventureWord(1)]).toEqual(["Comfort", "Balanced", "Adventure"]);
  });
});
