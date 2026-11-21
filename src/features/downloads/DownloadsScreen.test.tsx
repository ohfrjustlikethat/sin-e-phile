import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const MB = 1024 * 1024;

const view = (over: Record<string, unknown>) => ({
  id: 1, media_item_id: 7, title: "Sunrise", state: "active", backend: "librqbit", priority: 0,
  done_bytes: 420 * MB, total_bytes: 1000 * MB, limit_kib: null, not_before: null,
  reason: "It would pause to buffer if streamed (1.8 MB/s against the 2.0 MB/s it needs), so download it first, about a minute.",
  error: null, file_path: null, ...over,
});

const ROWS = [
  { download: view({}), speed: 4 * MB, peers: 12 },
  { download: view({ id: 2, title: "Tabu", state: "queued", done_bytes: 0, priority: -1, reason: null }), speed: 0, peers: 0 },
];

const commands = {
  streamCache: vi.fn(() => ok({ gib: 20, used_bytes: 3 * 1024 * MB })),
  setStreamCache: vi.fn((_gib: number) => ok(null)),
  downloadShow: vi.fn((_id: number) => ok(null)),
  downloads: vi.fn(() => ok(ROWS)),
  downloadsStorage: vi.fn(() =>
    ok({
      used_bytes: 6 * 1024 * MB,
      free_bytes: 200 * 1024 * MB,
      budget_bytes: 20 * 1024 * MB,
      items: [
        { download: view({ id: 9, title: "Nosferatu", state: "done", file_path: "D:/Downloads/Nosferatu/Nosferatu.mkv" }), bytes: 4 * 1024 * MB, age_days: 12, watched: true },
        { download: view({ id: 10, title: "Faust", state: "done" }), bytes: 2 * 1024 * MB, age_days: 1, watched: false },
      ],
    }),
  ),
  downloadsSettings: vi.fn(() =>
    ok({ max_active: 3, budget_gib: 20, qbittorrent_address: null, qbittorrent_user: null, qbittorrent_has_password: false }),
  ),
  downloadPause: vi.fn(() => ok(null)),
  downloadResume: vi.fn(() => ok(null)),
  downloadPriority: vi.fn(() => ok(null)),
  downloadTonight: vi.fn(() => ok(null)),
  downloadLimit: vi.fn(() => ok(null)),
  downloadCancel: vi.fn(() => ok(null)),
  downloadClear: vi.fn(() => ok(null)),
  setDownloadsSettings: vi.fn(() => ok(null)),
  setQbittorrent: vi.fn(() => ok(null)),
  qbittorrentCheck: vi.fn(() => ok(false)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
const { DownloadsScreen } = await import("./DownloadsScreen");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <DownloadsScreen />
    </QueryClientProvider>,
  );
}

describe("Downloads (SPEC.md Phase 13)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUi.setState({ downloadsOpen: true, detail: null });
  });

  it("shows what is downloading, how fast, and why", async () => {
    renderIt();
    expect(await screen.findByText("42% · 4.0 MB/s from 12 people · about 3 minutes left")).toBeInTheDocument();
    expect(screen.getByText(/so download it first/)).toBeInTheDocument();
    expect(screen.getByText("Waiting its turn")).toBeInTheDocument();
    expect(await screen.findByText("1 downloading · 4.0 MB/s · 200.0 GB free")).toBeInTheDocument();
  });

  it("pauses, reorders, moves to tonight and limits a download", async () => {
    renderIt();
    const first = (await screen.findByText("Sunrise")).closest("li") as HTMLElement;
    fireEvent.click(within(first).getByRole("button", { name: "Pause" }));
    expect(commands.downloadPause).toHaveBeenCalledWith(1);
    expect(within(first).getByRole("button", { name: "Move Sunrise up" })).toBeDisabled();
    fireEvent.click(within(first).getByRole("button", { name: "Move Sunrise down" }));
    // Renumbered so it is exactly one place down: Tabu 2, Sunrise 1.
    await waitFor(() => expect(commands.downloadPriority).toHaveBeenCalledWith(1, 1));
    expect(commands.downloadPriority).toHaveBeenCalledWith(2, 2);
    fireEvent.click(within(first).getByRole("button", { name: "Tonight" }));
    expect(commands.downloadTonight).toHaveBeenCalledWith(1, true);
    fireEvent.change(within(first).getByLabelText("Speed limit for Sunrise"), { target: { value: "2048" } });
    expect(commands.downloadLimit).toHaveBeenCalledWith(1, 2048);
  });

  it("asks before cancelling, and deletes only when confirmed", async () => {
    renderIt();
    const first = (await screen.findByText("Sunrise")).closest("li") as HTMLElement;
    fireEvent.click(within(first).getByRole("button", { name: "Cancel…" }));
    fireEvent.click(within(first).getByRole("button", { name: "Keep it" }));
    expect(commands.downloadCancel).not.toHaveBeenCalled();
    fireEvent.click(within(first).getByRole("button", { name: "Cancel…" }));
    fireEvent.click(within(first).getByRole("button", { name: "Cancel download" }));
    expect(commands.downloadCancel).toHaveBeenCalledWith(1, true);
  });

  it("lists what is kept, watched first, and clears one on confirmation", async () => {
    renderIt();
    const kept = await screen.findByRole("region", { name: "Kept on this PC" });
    expect(within(kept).getByText("6.0 GB of the 20.0 GB you set aside. Watched films come first.")).toBeInTheDocument();
    const top = within(kept).getAllByRole("listitem")[0] as HTMLElement;
    expect(within(top).getByText("Nosferatu")).toBeInTheDocument();
    expect(within(top).getByText("Watched")).toBeInTheDocument();
    expect(within(top).getByText("12 days ago")).toBeInTheDocument();
    fireEvent.click(within(top).getByRole("button", { name: "Delete Nosferatu" }));
    fireEvent.click(within(top).getByRole("button", { name: "Delete 4.0 GB" }));
    expect(commands.downloadClear).toHaveBeenCalledWith(9);
  });

  it("saves the queue settings and the user's own qBittorrent", async () => {
    renderIt();
    fireEvent.change(await screen.findByLabelText("Web UI address"), { target: { value: "http://localhost:8080" } });
    fireEvent.change(screen.getByLabelText("User name"), { target: { value: "me" } });
    fireEvent.change(screen.getByLabelText("Downloading at once"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(commands.setQbittorrent).toHaveBeenCalledWith("http://localhost:8080", "me", null));
    expect(commands.setDownloadsSettings).toHaveBeenCalledWith(2, 20);
    expect(await screen.findByText("Saved.")).toBeInTheDocument();
  });

  it("says what to do when nothing is downloading", async () => {
    commands.downloads.mockImplementationOnce(() => ok([]));
    renderIt();
    expect(await screen.findByText("Nothing downloading")).toBeInTheDocument();
  });

  it("plays a finished file, shows it in its folder, and opens its page (Phase 16)", async () => {
    renderIt();
    fireEvent.click(await screen.findByRole("button", { name: "Show Nosferatu in its folder" }));
    await waitFor(() => expect(commands.downloadShow).toHaveBeenCalledWith(9));
    fireEvent.click(screen.getByRole("button", { name: "Play Nosferatu" }));
    expect(useUi.getState().playing).toEqual({
      target: "D:/Downloads/Nosferatu/Nosferatu.mkv", title: "Nosferatu", mediaItemId: 7, start: null,
    });
    fireEvent.click(screen.getByRole("button", { name: "Open Nosferatu's page" }));
    expect(useUi.getState().detail).toBe(7);
    // Not finished, nothing to play: Faust has no file yet.
    expect(screen.queryByRole("button", { name: "Play Faust" })).toBeNull();
  });
});

/** ADR-0057, the author's answers after 524 GB of streams were found kept. */
describe("the stream cache", () => {
  it("shows the cap and what streams hold, and a new cap is saved at once", async () => {
    renderIt();
    const cap = await screen.findByLabelText("Streamed films kept, at most");
    expect(cap).toHaveValue("20");
    expect(screen.getByText(/Streams hold 3\.0 GB now\./)).toBeInTheDocument();
    expect(screen.getByText(/Downloads are never deleted this way/)).toBeInTheDocument();
    fireEvent.change(cap, { target: { value: "50" } });
    await waitFor(() => expect(commands.setStreamCache).toHaveBeenCalledWith(50));
  });
});

