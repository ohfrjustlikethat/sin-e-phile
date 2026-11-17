import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  home: vi.fn(() =>
    ok([
      { key: "continue", title: "Continue watching", items: [{ id: 1, title: "Heat", year: 1995, made_by: "Michael Mann", runtime_minutes: 170 }] },
      { key: "loved", title: "Widely loved", items: [{ id: 7, title: "Sunrise", year: 1927, made_by: null, runtime_minutes: 94 }] },
    ]),
  ),
  // What the "Finish setting up" card reads (Appendix F, I38): a fresh install.
  libraryFolders: vi.fn(() => ok([] as { id: number }[])),
  sourcesOverview: vi.fn(() =>
    ok({ sources: [{ id: "archive", name: "Internet Archive", addon_host: null as string | null }], refused_addons: [] }),
  ),
  subtitleSettings: vi.fn(() => ok({ has_key: false })),
  torrentSettings: vi.fn(() => ok({ bind_interface: null as string | null })),
};
vi.mock("@/lib/ipc", () => ({ commands }));
// jsdom has no layout: the rail measures nothing, and renders its first cards.
globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;
const { HomeScreen } = await import("./HomeScreen");

describe("Home, first version (Phase 14)", () => {
  it("shows each rail, with a reason, and a card opens its film", async () => {
    useUi.setState({ detail: null });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <HomeScreen />
      </QueryClientProvider>,
    );
    const loved = await screen.findByRole("region", { name: "Widely loved" });
    expect(within(loved).getByText("The films most people have seen and rated")).toBeInTheDocument();
    // No director known: the byline says the year alone, never " · 1927" or "Uncatalogued".
    expect(within(loved).getByText("1927")).toBeInTheDocument();
    expect(screen.queryByText("Uncatalogued")).toBeNull();
    fireEvent.click(within(screen.getByRole("region", { name: "Continue watching" })).getByRole("button", { name: /Heat/ }));
    expect(useUi.getState().detail).toBe(1);
  });

  const renderHome = () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    return render(
      <QueryClientProvider client={client}>
        <HomeScreen />
      </QueryClientProvider>,
    );
  };

  it("a fresh install is told what is left to set up, and each line opens its setting (I38)", async () => {
    useUi.setState({ settingsOpen: false });
    renderHome();
    const card = await screen.findByRole("region", { name: "Finish setting up" });
    expect(within(card).getByText("0 of 4 done")).toBeInTheDocument();
    // A built-in source is not a source the viewer chose: the line stays undone.
    expect(within(card).getByText(/no sources of its own/)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: /Somewhere to watch from/ }));
    expect(useUi.getState().settingsOpen).toBe(true);
    useUi.setState({ settingsOpen: false });
  });

  it("with everything set up the card is not there; hidden, it stays hidden", async () => {
    commands.libraryFolders.mockImplementationOnce(() => ok([{ id: 1 }]));
    commands.sourcesOverview.mockImplementationOnce(() =>
      ok({ sources: [{ id: "a", name: "Mine", addon_host: "films.example" }], refused_addons: [] }),
    );
    commands.subtitleSettings.mockImplementationOnce(() => ok({ has_key: true }));
    commands.torrentSettings.mockImplementationOnce(() => ok({ bind_interface: "VPN" }));
    const done = renderHome();
    await screen.findByRole("region", { name: "Widely loved" });
    await waitFor(() => expect(commands.torrentSettings).toHaveBeenCalled());
    expect(screen.queryByRole("region", { name: "Finish setting up" })).toBeNull();
    done.unmount();

    renderHome();
    const card = await screen.findByRole("region", { name: "Finish setting up" });
    fireEvent.click(within(card).getByRole("button", { name: "Hide" }));
    expect(screen.queryByRole("region", { name: "Finish setting up" })).toBeNull();
  });
});
