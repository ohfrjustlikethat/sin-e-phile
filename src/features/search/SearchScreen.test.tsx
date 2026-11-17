import { fireEvent, render as rtlRender, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchHit, SearchResponse } from "@/lib/ipc";
import { useUi } from "@/lib/store";

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
  optionalAssets: vi.fn(() => Promise.resolve({ assets: [], outstanding_bytes: 0, complete: true, running: false })),
  // The result cards are Home's (round 9); no poster source here, so they are the lettered cards.
  artworkSettings: vi.fn(() => ok({ posters: false, tmdb: false })),
  poster: vi.fn(() => ok(null)),
  recordSignal: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { SearchScreen, filmsFirst } = await import("./SearchScreen");

function render(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return rtlRender(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const hit = (id: number, title: string, kind: string, why: SearchHit["why"] = "keyword"): SearchHit => ({
  id, title, year: 1990, kind, why, made_by: null, runtime_minutes: null, excerpt: null,
});

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

  it("Down or Enter in the field goes to the first result, which opens its title", async () => {
    // Ranked a series first: the grid still starts with the film (round 9).
    commands.search.mockReturnValue(ok({ ...FOUND, hits: [hit(11, "Twin Peaks", "series"), hit(10, "Häxan", "film")] }));
    useUi.setState({ detail: null });
    render(<SearchScreen />);
    const field = screen.getByLabelText("Search the catalogue");
    fireEvent.change(field, { target: { value: "carriage" } });
    await screen.findByText("Häxan");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(document.activeElement).toHaveAccessibleName(/^Häxan/);
    field.focus();
    fireEvent.keyDown(field, { key: "Enter" });
    expect(document.activeElement).toHaveAccessibleName(/^Häxan/);
    fireEvent.click(document.activeElement!);
    expect(useUi.getState().detail).toBe(10);
  });

  it("shows one grid, films first, each other kind marked (round 9)", async () => {
    commands.search.mockReturnValue(
      ok({
        ...FOUND,
        hits: [
          hit(1, "Twin Peaks", "series", "exact_title"),
          hit(2, "Fire Walk with Me", "film"),
          hit(3, "Perfect Blue", "anime_film", "semantic"),
          hit(4, "Lost Highway", "film", "fuzzy"),
        ],
      }),
    );
    render(<SearchScreen />);
    fireEvent.change(screen.getByLabelText("Search the catalogue"), { target: { value: "peaks" } });
    const grid = await screen.findByRole("list", { name: "Results" });
    const cards = [...grid.querySelectorAll("button")].map((b) => b.getAttribute("aria-label"));
    // The exact title first, though a series; then films first among the rest of each tier.
    expect(cards).toEqual([
      "Twin Peaks, 1990, Series",
      "Fire Walk with Me, 1990",
      "Lost Highway, 1990, close spelling",
      "Perfect Blue, 1990, Anime film, similar in meaning",
    ]);
    // No headings between kinds: one grid.
    expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
  });

  it("does not ask the sources while the catalogue is still being built", async () => {
    commands.search.mockReturnValue(ok({ ...EMPTY, catalogue_partial: true }));
    render(<SearchScreen />);
    fireEvent.change(screen.getByLabelText("Search the catalogue"), { target: { value: "zzzz" } });
    expect(await screen.findByText("Still building the catalogue")).toBeInTheDocument();
    expect(commands.searchBackfill).not.toHaveBeenCalled();
  });

  it("puts films first among equally good matches, and never a film found by meaning above a series found by name", () => {
    const ordered = filmsFirst([
      hit(1, "a", "episode"),
      hit(2, "b", "film"),
      hit(3, "c", "series"),
      hit(4, "d", "film"),
      hit(5, "the series by name", "series", "keyword"),
      hit(6, "a film by meaning", "film", "semantic"),
    ]);
    // All but the last matched by their words: films (2, 4), then series in rank order (3, 5), then the episode;
    // the film found only by meaning comes after all of them.
    expect(ordered.map((h) => h.id)).toEqual([2, 4, 3, 5, 1, 6]);
  });

  it("offers search by meaning at the top of the empty page, with its size (round 8)", async () => {
    // The author's install: never downloaded, and the offer under the results went unseen.
    commands.optionalAssets.mockImplementation(() =>
      Promise.resolve({
        assets: [{ name: "Meaning index", purpose: "by meaning", bytes: 329_537_568, state: "absent", unusable_reason: null }],
        outstanding_bytes: 329_537_568, complete: false, running: false,
      } as never),
    );
    // The query is kept in the store between visits (Phase 16); this page is the empty one.
    useUi.setState({ searchQuery: "" });
    try {
      render(<SearchScreen />);
      expect(await screen.findByRole("region", { name: "Search by meaning" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Download 314 MB" })).toBeInTheDocument();
      expect(commands.search).not.toHaveBeenCalled();
    } finally {
      commands.optionalAssets.mockImplementation(() =>
        Promise.resolve({ assets: [], outstanding_bytes: 0, complete: true, running: false }),
      );
    }
  });
});
