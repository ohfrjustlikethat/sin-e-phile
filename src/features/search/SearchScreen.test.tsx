import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchResponse } from "@/lib/ipc";

/** ADR-0030 layer 3 on the search screen: a miss is put to the sources after it shows. */

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });

const EMPTY: SearchResponse = {
  hits: [], searchable_titles: 900_000, catalogue_partial: false, semantic: true, ready: true,
};
const FOUND: SearchResponse = {
  ...EMPTY,
  hits: [{
    id: 9, title: "The Phantom Carriage", year: 1921, kind: "film", why: "exact_title",
    made_by: null, runtime_minutes: null, excerpt: null,
  }],
};

const commands = {
  search: vi.fn(),
  searchBackfill: vi.fn(),
  optionalAssets: vi.fn(() => Promise.resolve({ missing: [], total_bytes: 0 })),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { SearchScreen } = await import("./SearchScreen");

describe("SearchScreen backfill", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the miss first, then asks the sources, then shows what one found", async () => {
    commands.search.mockReturnValueOnce(ok(EMPTY)).mockReturnValueOnce(ok(FOUND));
    let answer: (v: unknown) => void = () => {};
    commands.searchBackfill.mockReturnValueOnce(new Promise((r) => (answer = r)));

    render(<SearchScreen />);
    fireEvent.change(screen.getByLabelText("Search the catalogue"), {
      target: { value: "The Phantom Carriage" },
    });

    // The empty result is on screen BEFORE the sources have answered.
    expect(await screen.findByText("Not in the catalogue")).toBeInTheDocument();
    expect(commands.searchBackfill).toHaveBeenCalledWith("The Phantom Carriage");

    answer({ status: "ok", data: { id: 9, title: "The Phantom Carriage", year: 1921, found_by: "Internet Archive" } });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Not in the catalogue until now: Internet Archive has The Phantom Carriage (1921), so it has been added.",
    );
    expect(commands.search).toHaveBeenCalledTimes(2);
  });

  it("says nothing was found only once the sources have also said no", async () => {
    commands.search.mockReturnValue(ok(EMPTY));
    commands.searchBackfill.mockReturnValue(ok(null));
    render(<SearchScreen />);
    fireEvent.change(screen.getByLabelText("Search the catalogue"), { target: { value: "zzzz" } });
    await waitFor(() => expect(screen.getByText("Nothing found")).toBeInTheDocument());
    expect(screen.getByText(/none of your sources has it/)).toBeInTheDocument();
  });

  it("does not ask the sources while the catalogue is still being built", async () => {
    commands.search.mockReturnValue(ok({ ...EMPTY, catalogue_partial: true }));
    render(<SearchScreen />);
    fireEvent.change(screen.getByLabelText("Search the catalogue"), { target: { value: "zzzz" } });
    expect(await screen.findByText("Still building the catalogue")).toBeInTheDocument();
    expect(commands.searchBackfill).not.toHaveBeenCalled();
  });
});
