import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const MODEL = { name: "Search model", purpose: "understands what you type", bytes: 34_000_000, state: "absent", unusable_reason: null };
const INDEX = { name: "Meaning index", purpose: "every title, by meaning", bytes: 347_000_000, state: "absent", unusable_reason: null };
const PLAN = { assets: [MODEL, INDEX], outstanding_bytes: 381_000_000, complete: false, running: false };
const plan = { current: PLAN as typeof PLAN };
const catalogue = { ready: true };

const commands = {
  optionalAssets: vi.fn(() => Promise.resolve(plan.current)),
  downloadOptionalAssets: vi.fn(() => ok(null)),
  catalogueStatus: vi.fn(() =>
    ok({ refreshed_at: null, titles_added: null, episodes_added: null, running: false, ready: catalogue.ready, building: null, percent: null }),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const events = vi.hoisted(() => ({ on: {} as Record<string, (e: { payload: unknown }) => void> }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, cb: (e: { payload: unknown }) => void) => {
    events.on[name] = cb;
    return Promise.resolve(() => {});
  }),
}));
const { MeaningDownload } = await import("./MeaningDownload");

function renderIt(setup = false) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MeaningDownload setup={setup} />
    </QueryClientProvider>,
  );
}

/** ADR-0050 (the author, round 8): a new install downloads it during setup; an existing one is offered it. */
describe("Search by meaning", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    plan.current = PLAN;
    catalogue.ready = true;
    events.on = {};
  });

  it("is offered to an existing install with its size, and only a press starts it", async () => {
    renderIt();
    const button = await screen.findByRole("button", { name: "Download 363 MB" });
    expect(commands.downloadOptionalAssets).not.toHaveBeenCalled();
    fireEvent.click(button);
    expect(commands.downloadOptionalAssets).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("status")).toHaveTextContent("Downloading · 363 MB");
    expect(screen.queryByRole("button", { name: /^Download/ })).toBeNull();
  });

  it("in a new install's setup, says it downloads by itself once the catalogue can be searched", async () => {
    catalogue.ready = false;
    renderIt(true);
    expect(await screen.findByText(/Downloads by itself, 363 MB, as soon as your catalogue can be searched/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Download/ })).toBeNull();
    expect(commands.downloadOptionalAssets).not.toHaveBeenCalled();
  });

  it("in setup, with the files the installer carried, says it comes with the app and downloads nothing (ADR-0053)", async () => {
    catalogue.ready = false;
    plan.current = { ...PLAN, assets: [{ ...MODEL, state: "present" }, { ...INDEX, state: "present" }], outstanding_bytes: 0 };
    renderIt(true);
    expect(await screen.findByText(/Comes with the app\. Its index is built on this PC/)).toBeInTheDocument();
    expect(screen.queryByText(/MB/)).toBeNull();
    expect(commands.downloadOptionalAssets).not.toHaveBeenCalled();
  });

  it("shows setup's download while it runs, never a second button, and goes once it works", async () => {
    plan.current = { ...PLAN, running: true };
    renderIt(true);
    expect(await screen.findByRole("status")).toHaveTextContent("Downloading as part of setting up · 363 MB");
    expect(screen.queryByRole("button", { name: /^Download/ })).toBeNull();
    await waitFor(() => expect(events.on["meaning-ready"]).toBeDefined());
    plan.current = { ...PLAN, complete: true, running: false };
    act(() => events.on["meaning-ready"]?.({ payload: true }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Search by meaning" })).toBeNull());
  });

  it("with everything downloaded and no index, offers to build it, not to download 0 MB", async () => {
    plan.current = { ...PLAN, assets: [{ ...MODEL, state: "present" }, { ...INDEX, state: "present" }], outstanding_bytes: 0 };
    renderIt();
    fireEvent.click(await screen.findByRole("button", { name: "Build the index" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Building the index on this PC");
    expect(screen.queryByText(/0 MB/)).toBeNull();
  });

  it("a download setup starts after the offer was drawn replaces the button", async () => {
    renderIt();
    await screen.findByRole("button", { name: "Download 363 MB" });
    await waitFor(() => expect(events.on["asset-progress"]).toBeDefined());
    act(() => events.on["asset-progress"]?.({ payload: { name: "Search model", phase: "downloading", done_bytes: 1_048_576, total_bytes: 34_000_000 } }));
    expect(screen.queryByRole("button", { name: /^Download/ })).toBeNull();
    expect(screen.getByText(/Search model — 1 MB of 32 MB/)).toBeInTheDocument();
  });
});
