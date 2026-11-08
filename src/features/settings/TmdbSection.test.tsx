import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
let saved = false;
const commands = {
  tmdbStatus: vi.fn(() => ok({ has_key: saved, fetched_on: saved ? "2026-11-08" : null })),
  setTmdbKey: vi.fn((key: string) => {
    if (key !== "good") {
      return Promise.resolve({ status: "error" as const, error: "TMDB did not accept that key." });
    }
    saved = true;
    return ok(null);
  }),
  clearTmdbKey: vi.fn(() => ok(true)),
  openSignUp: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { TmdbSection } = await import("./TmdbSection");

describe("TMDB with the viewer's own key (Phase 16)", () => {
  it("says when a key is refused, and saves one that is accepted", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <TmdbSection />
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Get a free key" }));
    expect(commands.openSignUp).toHaveBeenCalledWith("tmdb");
    fireEvent.change(screen.getByLabelText("TMDB API key"), { target: { value: "bad" } });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));
    expect(await screen.findByText("TMDB did not accept that key.")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("TMDB API key"), { target: { value: "good" } });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("last fetched 2026-11-08"));
  });
});
