import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";
import type { HomeCard } from "@/lib/ipc";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const card = (id: number, title: string, year: number) => ({ id, title, year, made_by: null, runtime_minutes: null, note: null, progress: null });

const commands = {
  libraryTitles: vi.fn((shows: boolean) =>
    ok(shows ? [card(364360, "Samurai Champloo", 2004), card(94345, "Twin Peaks", 1990)] : [card(112691, "In the Mood for Love", 2000)]),
  ),
  continueWatching: vi.fn((_shows: boolean) => ok([] as HomeCard[])),
  catalogueRails: vi.fn((shows: boolean) =>
    ok([
      { key: "most", title: shows ? "Most watched shows" : "Most watched films", items: [card(1, "Heat", 1995)] },
      { key: "Crime", title: "Crime", items: [card(2, "Thief", 1981)] },
    ]),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
// The catalogue rails measure themselves; jsdom has no ResizeObserver.
globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;
const { LibraryShelf } = await import("./LibraryShelf");

function renderIt(shows: boolean) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <LibraryShelf shows={shows} />
    </QueryClientProvider>,
  );
}

/** Phase 16: the TV Shows page was a placeholder while the author's shows were on this PC. */
describe("LibraryShelf", () => {
  it("TV Shows lists each show on this PC once, by title, and opens it", async () => {
    renderIt(true);
    expect(await screen.findByText("Samurai Champloo")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "TV Shows" })).toBeInTheDocument();
    expect(screen.getByText(/2 shows/)).toBeInTheDocument();
    expect(commands.libraryTitles).toHaveBeenLastCalledWith(true);
    fireEvent.click(screen.getByText("Twin Peaks"));
    expect(useUi.getState().detail).toBe(94345);
  });

  it("Films lists the films", async () => {
    renderIt(false);
    expect(await screen.findByText("In the Mood for Love")).toBeInTheDocument();
    expect(screen.getByText(/1 film$/)).toBeInTheDocument();
  });

  it("with nothing on this PC, says how to add a folder rather than showing an empty page", async () => {
    commands.libraryTitles.mockImplementationOnce(() => ok([]));
    renderIt(true);
    expect(await screen.findByText(/^No shows on this PC yet\./)).toBeInTheDocument();
    // Not a dead end: the catalogue is still below.
    expect(await screen.findByRole("region", { name: "Most watched shows" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add a folder" }));
    expect(useUi.getState().settingsOpen).toBe(true);
  });

  it("puts the catalogue under the library, so the tab is never empty (Phase 16)", async () => {
    renderIt(false);
    await screen.findByText("In the Mood for Love");
    const regions = (await screen.findAllByRole("region")).map((r) => r.getAttribute("aria-label"));
    expect(regions.slice(0, 3)).toEqual(["Films", "Most watched films", "Crime"]);
    expect(commands.catalogueRails).toHaveBeenLastCalledWith(false);
    fireEvent.click(screen.getByText("Thief"));
    expect(useUi.getState().detail).toBe(2);
  });

  it("starts with Continue watching, of this page's kind only, a show's card opening the show (round 8)", async () => {
    commands.continueWatching.mockImplementationOnce((shows: boolean) =>
      ok(shows ? [{ ...card(94345, "Twin Peaks", 1990), note: "S1 E2 · Traces to Nowhere", progress: 0.03 }] : []),
    );
    renderIt(true);
    await screen.findByText("Samurai Champloo");
    expect(commands.continueWatching).toHaveBeenLastCalledWith(true);
    const regions = (await screen.findAllByRole("region")).map((r) => r.getAttribute("aria-label"));
    expect(regions[0]).toBe("Continue watching");
    const going = screen.getByRole("region", { name: "Continue watching" });
    expect(going).toHaveTextContent("S1 E2 · Traces to Nowhere");
    fireEvent.click(screen.getAllByText("Twin Peaks")[0]!);
    expect(useUi.getState().detail).toBe(94345);
  });

  it("has no Continue watching when there is nothing to continue", async () => {
    renderIt(false);
    await screen.findByText("In the Mood for Love");
    expect(screen.queryByRole("region", { name: "Continue watching" })).toBeNull();
  });
});
