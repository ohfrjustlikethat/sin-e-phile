import { fireEvent, render, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  home: vi.fn(() =>
    ok([
      { key: "continue", title: "Continue watching", items: [{ id: 1, title: "Heat", year: 1995, made_by: "Michael Mann", runtime_minutes: 170 }] },
      { key: "loved", title: "Widely loved", items: [{ id: 7, title: "Sunrise", year: 1927, made_by: null, runtime_minutes: 94 }] },
    ]),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
// jsdom has no layout: the rail measures nothing, and renders its first cards.
globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;
const { HomeScreen } = await import("./HomeScreen");

describe("Home, first version (Phase 14)", () => {
  it("shows each rail, with a reason, and a card opens its film", async () => {
    useUi.setState({ detail: null });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <HomeScreen />
      </QueryClientProvider>,
    );
    const loved = await screen.findByRole("region", { name: "Widely loved" });
    expect(within(loved).getByText("The films most people have seen and rated")).toBeInTheDocument();
    // No director known: the byline says the year alone, never " · 1927" or "Uncatalogued".
    expect(within(loved).getByText("1927")).toBeInTheDocument();
    expect(screen.queryByText("Uncatalogued")).toBeNull();
    fireEvent.click(within(screen.getByRole("region", { name: "Continue watching" })).getByRole("button", { name: /Heat/ }));
    expect(useUi.getState().detail).toBe(1);
  });
});
