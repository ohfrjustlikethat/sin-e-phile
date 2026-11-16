import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const card = (id: number, title: string, year: number) => ({ id, title, year, made_by: null, runtime_minutes: null, note: null });

const commands = {
  libraryTitles: vi.fn((shows: boolean) =>
    ok(shows ? [card(364360, "Samurai Champloo", 2004), card(94345, "Twin Peaks", 1990)] : [card(112691, "In the Mood for Love", 2000)]),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
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
    expect(await screen.findByRole("heading", { name: "No shows on this PC yet" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add a folder" }));
    expect(useUi.getState().settingsOpen).toBe(true);
  });
});
