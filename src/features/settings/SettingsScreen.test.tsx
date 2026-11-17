import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

// Each section stands in as its name: this tests the side list, not the sections.
vi.mock("./ProfilesSection", () => ({ ProfilesSection: () => <p>profiles here</p> }));
vi.mock("./LibrarySection", () => ({ LibrarySection: () => <section aria-label="Library">library here</section> }));
vi.mock("./CatalogueSection", () => ({ CatalogueSection: () => <p>catalogue here</p> }));
vi.mock("./TmdbSection", () => ({ TmdbSection: () => <p>tmdb here</p> }));
vi.mock("./ArtworkSection", () => ({ ArtworkSection: () => <p>posters here</p> }));
vi.mock("./SourcesSection", () => ({ SourcesSection: () => <section aria-label="Add a source">add-ons here</section> }));
vi.mock("./PlaybackSection", () => ({ PlaybackSection: () => <p>playback here</p> }));
vi.mock("./SubtitlesSection", () => ({ SubtitlesSection: () => <p>subtitles here</p> }));
vi.mock("./NetworkSection", () => ({ NetworkSection: () => <p>network here</p> }));
vi.mock("./ControllerSection", () => ({ ControllerSection: () => <p>controller here</p> }));
vi.mock("@/features/downloads/DownloadsScreen", () => ({ Settings: () => <p>downloads here</p> }));
vi.mock("@/lib/ipc", () => ({
  commands: {
    getHardwareProfile: vi.fn(() => new Promise(() => {})),
    getDataDir: vi.fn(() => Promise.resolve("D:/sin-e-phile/data")),
    setTierOverride: vi.fn(),
    capabilities: vi.fn(() => new Promise(() => {})),
  },
}));
const { SettingsScreen } = await import("./SettingsScreen");
const { openSettingsAt } = await import("@/lib/openSettings");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <SettingsScreen />
    </QueryClientProvider>,
  );
}

describe("Settings: a side list, one section at a time (Phase 16)", () => {
  beforeEach(() => useUi.setState({ settingsSection: "profile", settingsOpen: true }));

  it("lists the author's nine sections in order and shows only the chosen one", () => {
    renderIt();
    const nav = screen.getByRole("navigation", { name: "Settings sections" });
    expect(within(nav).getAllByRole("button").map((b) => b.textContent)).toEqual([
      "Profile", "Library", "Add-ons", "Playback", "Subtitles", "Downloads", "Network", "Controller", "About",
    ]);
    expect(screen.getByText("profiles here")).toBeInTheDocument();
    expect(screen.queryByText("add-ons here")).toBeNull();
    fireEvent.click(within(nav).getByRole("button", { name: "Add-ons" }));
    expect(screen.getByText("add-ons here")).toBeInTheDocument();
    expect(screen.queryByText("profiles here")).toBeNull();
    expect(within(nav).getByRole("button", { name: "Add-ons" })).toHaveAttribute("aria-current", "page");
  });

  it("a link from elsewhere opens the section that holds its place", () => {
    renderIt();
    act(() => openSettingsAt("Add a source"));
    expect(screen.getByText("add-ons here")).toBeInTheDocument();
    act(() => openSettingsAt("Library"));
    expect(screen.getByText("library here")).toBeInTheDocument();
  });
});
