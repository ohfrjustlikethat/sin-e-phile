import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  catalogueStatus: vi.fn(() =>
    ok({ refreshed_at: new Date().toISOString(), titles_added: 1204, episodes_added: 87, running: false }),
  ),
  refreshCatalogue: vi.fn(() => ok(true)),
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

  it("puts a time in words", () => {
    const now = new Date("2026-11-08T12:00:00Z");
    expect(when("2026-11-07T12:00:00Z", now)).toBe("yesterday");
    expect(when("2026-11-04T12:00:00Z", now)).toBe("4 days ago");
    expect(when("nonsense", now)).toBe("at an unknown time");
  });
});
