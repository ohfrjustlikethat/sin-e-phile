import { fireEvent, render as rtlRender, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const row = (state: string) => ({ speed: 0, peers: 0, download: { id: state.length, state } });
const commands = {
  downloads: vi.fn(() => ok<unknown[]>([])),
  surpriseMe: vi.fn((_hour: number | null) => ok({ id: 7, note: null, on_this_pc: false })),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
const { Masthead } = await import("@/app/Masthead");

function render(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return rtlRender(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

/** SPEC.md §3.1 and §9.4, ADR-0037. */
describe("Masthead", () => {
  beforeEach(() => {
    useUi.setState({
      destination: "home",
      tab: "home",
      trails: { home: [], films: [], tv: [], watchlist: [], live: [], search: [] },
      detail: null,
      settingsOpen: false,
      downloadsOpen: false,
    });
  });

  it("has Downloads as a word, with the number in progress only when there is one", async () => {
    commands.downloads.mockImplementationOnce(() => ok([row("active"), row("queued"), row("done")]));
    render(<Masthead />);
    const button = await screen.findByRole("button", { name: "Downloads, 2 in progress" });
    expect(button).toHaveTextContent("Downloads2");
    fireEvent.click(button);
    expect(useUi.getState().downloadsOpen).toBe(true);
    expect(button).toHaveAttribute("aria-pressed", "true");
    const nav = screen.getByRole("navigation", { name: "Primary" });
    expect(within(nav).queryByRole("button", { current: "page" })).toBeNull();
  });

  it("has Surprise me as a word just left of Search, opening a film (ADR-0056)", async () => {
    render(<Masthead />);
    const surprise = screen.getByRole("button", { name: "Surprise me" });
    const search = screen.getByRole("button", { name: /Search/ });
    expect(surprise.compareDocumentPosition(search) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(surprise.nextElementSibling).toBe(search);
    fireEvent.click(surprise);
    await vi.waitFor(() => expect(useUi.getState().detail).toBe(7));
    expect(commands.surpriseMe).toHaveBeenCalledWith(expect.any(Number));
  });

  it("shows every destination as a word, in §3.1's order", () => {
    render(<Masthead />);
    const nav = screen.getByRole("navigation", { name: "Primary" });
    const labels = within(nav).getAllByRole("button").map((b) => b.textContent);
    expect(labels).toEqual(["Home", "Films", "TV Shows", "Watchlist", "Live"]);
  });

  it("has a visible, labelled Search button that shows its key", () => {
    // Search was reachable only by shortcuts that appear nowhere on screen.
    render(<Masthead />);
    const search = screen.getByRole("button", { name: /Search/ });
    expect(search).toHaveTextContent("/");
    fireEvent.click(search);
    expect(useUi.getState().tab).toBe("search");
    expect(search).toHaveAttribute("aria-pressed", "true");
  });

  it("Search and the destinations work from a film's page, Settings or Downloads (Phase 16)", () => {
    useUi.getState().openDetail(7);
    useUi.setState({ settingsOpen: true, downloadsOpen: true });
    render(<Masthead />);
    fireEvent.click(screen.getByRole("button", { name: /Search/ }));
    expect(useUi.getState()).toMatchObject({ tab: "search", detail: null, settingsOpen: false, downloadsOpen: false });

    useUi.setState({ downloadsOpen: true });
    const nav = screen.getByRole("navigation", { name: "Primary" });
    fireEvent.click(within(nav).getByRole("button", { name: "Watchlist" }));
    expect(useUi.getState()).toMatchObject({ detail: null, downloadsOpen: false, destination: "watchlist" });
    // Home kept its film: its tab shows it again.
    fireEvent.click(within(nav).getByRole("button", { name: "Home" }));
    expect(useUi.getState().detail).toBe(7);
  });

  it("marks exactly the current destination", () => {
    useUi.setState({ destination: "films" });
    render(<Masthead />);
    const current = screen
      .getAllByRole("button")
      .filter((b) => b.getAttribute("aria-current") === "page");
    expect(current.map((b) => b.textContent)).toEqual(["Films"]);
  });

  it("claims no destination while search is showing", () => {
    // A tab saying "Films" behind a page of search results says you are somewhere
    // you are not.
    useUi.setState({ destination: "films", tab: "search" });
    render(<Masthead />);
    expect(document.querySelector('[aria-current="page"]')).toBeNull();
  });

  it("leaves search and settings when a destination is chosen", () => {
    useUi.setState({ settingsOpen: true, tab: "search" });
    render(<Masthead />);
    fireEvent.click(screen.getByRole("button", { name: "Watchlist" }));
    expect(useUi.getState().tab).toBe("watchlist");
    expect(useUi.getState().settingsOpen).toBe(false);
    expect(useUi.getState().destination).toBe("watchlist");
  });

  it("renders inert window controls outside the Tauri shell rather than crashing", () => {
    render(<Masthead />);
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
  });
});
