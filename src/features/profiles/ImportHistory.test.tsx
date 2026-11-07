import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const VIEW = {
  from: "Letterboxd", entries: 600, matched: 592, ratings: 345, watches: 543, watchlist: 49,
  unmatched_total: 2,
  unmatched: [
    { title: "Daredevil", year: 2003, why: "more than one title by that name from 2003" },
    { title: "Men in Black III", year: 2012, why: "no title by that name in the catalogue" },
  ],
};
const commands = {
  importHistory: vi.fn(() => ok(VIEW)),
  pickImportFile: vi.fn(() => ok<string | null>(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { ImportHistory, summary } = await import("./ImportHistory");

describe("Import your history (Phase 14)", () => {
  it("says what came across in one line", () => {
    expect(summary(VIEW)).toBe(
      "Letterboxd: 592 of 600 found. 345 ratings, 543 watches and 49 on your watchlist.",
    );
    expect(summary({ ...VIEW, ratings: 1, watches: 0, watchlist: 0 })).toBe("Letterboxd: 592 of 600 found. 1 rating.");
    expect(summary({ ...VIEW, ratings: 0, watches: 0, watchlist: 0 })).toBe("Letterboxd: 592 of 600 found. Nothing new to record.");
  });

  it("imports a typed path and lists what it could not place", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <ImportHistory />
      </QueryClientProvider>,
    );
    fireEvent.change(screen.getByLabelText("Or its path"), { target: { value: "  D:/exports/x.zip " } });
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    await waitFor(() => expect(commands.importHistory).toHaveBeenCalledWith("D:/exports/x.zip"));
    fireEvent.click(await screen.findByRole("button", { name: "Show the 2 not found" }));
    expect(screen.getByText("Daredevil")).toBeInTheDocument();
    expect(screen.getByText(/more than one title by that name from 2003/)).toBeInTheDocument();
  });

  it("a cancelled picker imports nothing", async () => {
    commands.importHistory.mockClear();
    const client = new QueryClient();
    render(
      <QueryClientProvider client={client}>
        <ImportHistory />
      </QueryClientProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Choose an export…" }));
    await waitFor(() => expect(commands.pickImportFile).toHaveBeenCalled());
    expect(commands.importHistory).not.toHaveBeenCalled();
  });
});
