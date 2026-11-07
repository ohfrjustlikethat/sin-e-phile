import { fireEvent, render as rtlRender, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const row = (state: string) => ({ speed: 0, peers: 0, download: { id: state.length, state } });
const commands = { downloads: vi.fn(() => ok<unknown[]>([])) };
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
    useUi.setState({ destination: "home", settingsOpen: false, downloadsOpen: false });
  });

  it("has Downloads as a word, with the number in progress only when there is one", async () => {
    commands.downloads.mockImplementationOnce(() => ok([row("active"), row("queued"), row("done")]));
    render(<Masthead searching={false} onSearchChange={() => {}} />);
    const button = await screen.findByRole("button", { name: "Downloads, 2 in progress" });
    expect(button).toHaveTextContent("Downloads2");
    fireEvent.click(button);
    expect(useUi.getState().downloadsOpen).toBe(true);
    expect(button).toHaveAttribute("aria-pressed", "true");
    const nav = screen.getByRole("navigation", { name: "Primary" });
    expect(within(nav).queryByRole("button", { current: "page" })).toBeNull();
  });

  it("shows every destination as a word, in §3.1's order", () => {
    render(<Masthead searching={false} onSearchChange={() => {}} />);
    const nav = screen.getByRole("navigation", { name: "Primary" });
    const labels = within(nav).getAllByRole("button").map((b) => b.textContent);
    expect(labels).toEqual(["Home", "Films", "TV Shows", "Watchlist", "Live"]);
  });

  it("has a visible, labelled Search button that shows its key", () => {
    // Search was reachable only by shortcuts that appear nowhere on screen.
    const onSearchChange = vi.fn();
    render(<Masthead searching={false} onSearchChange={onSearchChange} />);
    const search = screen.getByRole("button", { name: /Search/ });
    expect(search).toHaveTextContent("/");
    fireEvent.click(search);
    expect(onSearchChange).toHaveBeenCalledWith(true);
  });

  it("marks exactly the current destination", () => {
    useUi.setState({ destination: "films" });
    render(<Masthead searching={false} onSearchChange={() => {}} />);
    const current = screen
      .getAllByRole("button")
      .filter((b) => b.getAttribute("aria-current") === "page");
    expect(current.map((b) => b.textContent)).toEqual(["Films"]);
  });

  it("claims no destination while search is showing", () => {
    // A tab saying "Films" behind a page of search results says you are somewhere
    // you are not.
    useUi.setState({ destination: "films" });
    render(<Masthead searching onSearchChange={() => {}} />);
    expect(document.querySelector('[aria-current="page"]')).toBeNull();
  });

  it("leaves search and settings when a destination is chosen", () => {
    const onSearchChange = vi.fn();
    useUi.setState({ settingsOpen: true });
    render(<Masthead searching onSearchChange={onSearchChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Watchlist" }));
    expect(onSearchChange).toHaveBeenCalledWith(false);
    expect(useUi.getState().settingsOpen).toBe(false);
    expect(useUi.getState().destination).toBe("watchlist");
  });

  it("renders inert window controls outside the Tauri shell rather than crashing", () => {
    render(<Masthead searching={false} onSearchChange={() => {}} />);
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
  });
});
