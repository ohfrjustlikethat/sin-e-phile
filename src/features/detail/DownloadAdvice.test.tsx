import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const GB = 1024 * 1024 * 1024;

const STREAM = {
  choice: "stream",
  sentence: "The swarm is fast enough to stream it without pausing (6.2 MB/s against the 0.9 MB/s it needs).",
  stall: 0.01,
  size_bytes: 4.2 * GB,
  download: null,
};

const commands = {
  downloadAdvice: vi.fn(() => ok<unknown>(STREAM)),
  downloadTitle: vi.fn(() => ok({ id: 3 })),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
const { DownloadAdvice } = await import("./DownloadAdvice");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <DownloadAdvice id={7} plan="http://127.0.0.1:1/s" />
    </QueryClientProvider>,
  );
}

describe("stream or download, on a film's page (SPEC.md Phase 13)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("says it is measuring, then gives one sentence, with Download one press away", async () => {
    renderIt();
    expect(screen.getByText("Measuring how well it would stream…")).toBeInTheDocument();
    expect(await screen.findByText(STREAM.sentence)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Download · 4.2 GB" }));
    await waitFor(() => expect(commands.downloadTitle).toHaveBeenCalledWith(7, false, STREAM.sentence));
  });

  it("offers tonight first when tonight is the advice", async () => {
    commands.downloadAdvice.mockImplementationOnce(() =>
      ok({ ...STREAM, choice: "download-tonight", sentence: "It's too slow to stream now, and this connection is 3 times faster overnight, so it can be ready by morning." }),
    );
    renderIt();
    const tonight = await screen.findByRole("button", { name: "Download tonight" });
    expect(screen.getByRole("button", { name: "Download now · 4.2 GB" })).toBeInTheDocument();
    fireEvent.click(tonight);
    await waitFor(() => expect(commands.downloadTitle).toHaveBeenCalledWith(7, true, expect.stringMatching(/overnight/)));
  });

  it("says a title is already downloading, and leads to the Downloads page", async () => {
    commands.downloadAdvice.mockImplementationOnce(() => ok({ ...STREAM, download: { id: 3, state: "active" } }));
    useUi.setState({ downloadsOpen: false, detail: 7 });
    renderIt();
    expect(await screen.findByText("Downloading.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "See downloads" }));
    expect(useUi.getState().downloadsOpen).toBe(true);
    expect(useUi.getState().detail).toBeNull();
  });

  it("shows nothing for a copy that cannot be downloaded", async () => {
    commands.downloadAdvice.mockImplementationOnce(() => ok(null));
    const { container } = renderIt();
    await waitFor(() => expect(container).toBeEmptyDOMElement());
  });
});
