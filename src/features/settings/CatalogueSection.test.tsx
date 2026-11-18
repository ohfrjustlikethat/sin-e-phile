import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  catalogueStatus: vi.fn(() =>
    ok({ refreshed_at: new Date().toISOString(), titles_added: 1204, episodes_added: 87, running: false }),
  ),
  refreshCatalogue: vi.fn(() => ok(true)),
  stayingCurrent: vi.fn(() =>
    ok([
      { name: "Meaning for new titles", updated_at: new Date().toISOString(), detail: "188 titles given meaning at the last update, 374 in all" },
      { name: "TMDB's recommendations", updated_at: null, detail: "Used with a profile's own TMDB key; none is set" },
    ]),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { CatalogueSection, when } = await import("./CatalogueSection");

describe("The catalogue keeps itself current (Phase 16)", () => {
  it("says when it last updated and what it added, and refreshes on request", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <CatalogueSection />
      </QueryClientProvider>,
    );
    expect(await screen.findByText(/Updated today: 1,204 new titles, 87 new episodes\./)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh now" }));
    await waitFor(() => expect(commands.refreshCatalogue).toHaveBeenCalled());
    expect(await screen.findByText(/Updating the catalogue now/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh now" })).toBeDisabled();
  });

  it("says when the first build is running, and how far it has come", async () => {
    commands.catalogueStatus.mockImplementationOnce(() =>
      ok({ refreshed_at: null, titles_added: null, episodes_added: null, running: true, ready: false, building: "Building search", percent: 64 } as never),
    );
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <CatalogueSection />
      </QueryClientProvider>,
    );
    expect(await screen.findByText(/Building the catalogue for the first time: Building search, 64%\./)).toBeInTheDocument();
  });

  it("says, source by source, when each last updated and what it did (ADR-0054)", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <CatalogueSection />
      </QueryClientProvider>,
    );
    expect(await screen.findByText("Meaning for new titles")).toBeInTheDocument();
    expect(
      screen.getByText("Updated today. 188 titles given meaning at the last update, 374 in all."),
    ).toBeInTheDocument();
    // Without a time, the detail says what it is waiting for on its own.
    expect(screen.getByText("Used with a profile's own TMDB key; none is set.")).toBeInTheDocument();
  });

  it("puts a time in words", () => {
    const now = new Date("2026-11-08T12:00:00Z");
    expect(when("2026-11-07T12:00:00Z", now)).toBe("yesterday");
    expect(when("2026-11-04T12:00:00Z", now)).toBe("4 days ago");
    expect(when("nonsense", now)).toBe("at an unknown time");
  });
});
