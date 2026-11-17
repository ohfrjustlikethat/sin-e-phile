import { fireEvent, within, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const YOU = { id: 1, name: "You", is_default: true, avatar_colour: null, is_kids: false, has_pin: false, active: true };
const card = (id: number) => ({ id, title: `Film ${id}`, year: 1900 + id, made_by: "A Director" });
const commands = {
  profiles: vi.fn(() => ok([YOU])),
  updateProfile: vi.fn(() => ok(null)),
  finishOnboarding: vi.fn(() => ok(null)),
  tasteRound: vi.fn((shown: number[]) => ok(Array.from({ length: 12 }, (_, i) => card(shown.length + i + 1)))),
  rateTitle: vi.fn(() => ok(null)),
  tasteInput: vi.fn(() =>
    ok({ statements: { pace: null, ambiguity: null, era: null, subtitles: null, runtime: null, form: null }, love: null }),
  ),
  tastePreview: vi.fn(() => ok([])),
  libraryFolders: vi.fn(() => ok([])),
  libraryReviewCount: vi.fn(() => ok(0)),
  sources: vi.fn(() => ok([])),
  // Not a Result: the plan is pinned values, so the command cannot fail.
  optionalAssets: vi.fn(() =>
    Promise.resolve({
      assets: [{ name: "Meaning index", purpose: "Describe a film.", bytes: 329_537_568, state: "absent", unusable_reason: null }],
      outstanding_bytes: 329_537_568,
      complete: false,
    }),
  ),
  downloadOptionalAssets: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
vi.mock("@/features/settings/SourcesSection", () => ({ SourcesSection: () => <p>Sources here</p> }));
vi.mock("@/features/settings/SubtitlesSection", () => ({ SubtitlesSection: () => <p>Subtitles here</p> }));
vi.mock("@/features/settings/NetworkSection", () => ({ NetworkSection: () => <p>Network here</p> }));
vi.mock("@/features/settings/TmdbSection", () => ({ TmdbSection: () => <p>TMDB here</p> }));
vi.mock("@/features/downloads/DownloadsScreen", () => ({ Settings: () => <p>Download settings here</p> }));
const { Onboarding } = await import("./Onboarding");

function renderIt(onDone = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <Onboarding onDone={onDone} />
    </QueryClientProvider>,
  );
  return onDone;
}

describe("First run (Phase 14)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("names the profile, picks films by name, and starts", async () => {
    const done = renderIt();
    expect(screen.getByText("Who's this for?")).toBeInTheDocument();
    fireEvent.change(await screen.findByLabelText("Your name"), { target: { value: "Ada" } });
    fireEvent.click(screen.getByRole("radio", { name: "plum" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(commands.updateProfile).toHaveBeenCalledWith(1, "Ada", "plum", false));

    expect(await screen.findByText("What do you like?")).toBeInTheDocument();
    // The search model is offered during setup, with its size, and fetched only when asked (ADR-0014).
    expect(await screen.findByText("Search by meaning")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Download 314 MB" })).toBeInTheDocument();
    expect(commands.downloadOptionalAssets).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: /Film 3/ }));
    await waitFor(() => expect(commands.rateTitle).toHaveBeenCalledWith(3, 4));
    // A second round asks for films not yet shown.
    fireEvent.click(screen.getByRole("button", { name: "More like these, and others" }));
    await waitFor(() => expect(commands.tasteRound).toHaveBeenLastCalledWith(Array.from({ length: 12 }, (_, i) => i + 1), 12));

    // Phase 16: ready to watch — the library, sources (said plainly: none come with the app),
    // subtitles, downloads.
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("Your films on this PC")).toBeInTheDocument();
    // Phase 16: the film picked comes back as imagery in the steps after the picking.
    const picks = screen.getByLabelText("Your picks");
    expect(picks).toHaveTextContent("Your picks · 1");
    expect(within(picks).getByRole("img", { name: /^Film 3/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("Where films come from")).toBeInTheDocument();
    expect(screen.getByText(/comes with no sources of its own/)).toBeInTheDocument();
    expect(screen.getByText("Sources here")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("Subtitles here")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("TMDB here")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("Downloads")).toBeInTheDocument();
    expect(screen.getByText("Download settings here")).toBeInTheDocument();
    expect(screen.getByText("Network here")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Start watching" }));
    await waitFor(() => expect(commands.finishOnboarding).toHaveBeenCalled());
    expect(done).toHaveBeenCalled();
  });

  it("skipping every step still finishes, and nothing is a dead end", async () => {
    const done = renderIt();
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(await screen.findByText("What do you like?")).toBeInTheDocument();
    expect(commands.updateProfile).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(await screen.findByText("Your films on this PC")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByText("What do you like?")).toBeInTheDocument();
    for (let i = 0; i < 5; i++) fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(await screen.findByText("Downloads")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Skip, and start" }));
    await waitFor(() => expect(done).toHaveBeenCalled());
  });

  it("the three ways to tell it what you like are one press apart", async () => {
    renderIt();
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("tab", { name: "Say what you like" }));
    expect(await screen.findByRole("radiogroup", { name: "Pace" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Import from Letterboxd, Trakt, IMDb" }));
    expect(await screen.findByRole("button", { name: "Choose an export…" })).toBeInTheDocument();
  });
});
