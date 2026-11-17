import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const card = (id: number, title: string, year: number) => ({ id, title, year, made_by: "F. W. Murnau", runtime_minutes: 94, note: null });

const commands = { watchlist: vi.fn(() => ok([card(7, "Sunrise", 1927), card(8, "Nosferatu", 1922)])) };
vi.mock("@/lib/ipc", () => ({ commands }));
const { WatchlistScreen } = await import("./WatchlistScreen");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <WatchlistScreen />
    </QueryClientProvider>,
  );
}

describe("WatchlistScreen (SPEC.md Appendix F, I39)", () => {
  it("lists what was saved, says how many, and opens a title", async () => {
    renderIt();
    expect(await screen.findByText("Sunrise")).toBeInTheDocument();
    expect(screen.getByText("Nosferatu")).toBeInTheDocument();
    expect(screen.getByText(/2 titles/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Nosferatu"));
    expect(useUi.getState().detail).toBe(8);
  });

  it("with nothing saved, says how to save something rather than showing an empty page", async () => {
    commands.watchlist.mockImplementationOnce(() => ok([]));
    renderIt();
    expect(await screen.findByRole("heading", { name: "Nothing saved yet" })).toBeInTheDocument();
    expect(screen.getByText(/Add to watchlist/)).toBeInTheDocument();
  });
});
