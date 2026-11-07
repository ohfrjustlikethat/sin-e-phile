import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const FOLDER = { id: 1, path: "D:\\Films", files: 1204, scanned_at: "2026-11-07T10:00:00Z", scanning: false };
const commands = {
  libraryFolders: vi.fn(() => ok([FOLDER])),
  libraryReviewCount: vi.fn(() => ok(31)),
  addLibraryFolder: vi.fn(() => ok(2)),
  removeLibraryFolder: vi.fn(() => ok(null)),
  rescanLibraryFolder: vi.fn(() => ok(null)),
  pickLibraryFolder: vi.fn(() => ok<string | null>("E:\\TV")),
};
vi.mock("@/lib/ipc", () => ({ commands }));

type Handler = (e: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, h: Handler) => {
    handlers[name] = h;
    return Promise.resolve(() => undefined);
  }),
}));
const { LibrarySection, scanLine, folderName } = await import("./LibrarySection");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <LibrarySection />
    </QueryClientProvider>,
  );
}

const event = (phase: string, done: number, total: number) => ({
  folder_id: 1,
  phase,
  done,
  total,
  files: 0,
  accepted: 0,
  asked: 0,
});

describe("Settings → Library (SPEC.md Phase 12)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("says what a scan is doing in words", () => {
    expect(scanLine(event("walking", 3400, 0))).toBe("Looking… 3,400 files so far");
    expect(scanLine(event("identifying", 120, 900))).toBe("Identifying 120 of 900");
    expect(scanLine(event("done", 900, 900))).toBeNull();
    expect(scanLine(undefined)).toBeNull();
  });

  it("names a folder by its last part, a drive by itself", () => {
    expect(folderName("D:\\Media\\Films\\")).toBe("Films");
    expect(folderName("E:\\")).toBe("E:\\");
  });

  it("lists folders with their files, and what waits for a check", async () => {
    renderIt();
    expect(await screen.findByText("Films")).toBeInTheDocument();
    expect(screen.getByText("D:\\Films")).toBeInTheDocument();
    expect(screen.getByText("1,204 files")).toBeInTheDocument();
    expect(await screen.findByText("31 files need a quick check.")).toBeInTheDocument();
  });

  it("shows a scan's progress as it happens", async () => {
    renderIt();
    await screen.findByText("D:\\Films");
    await waitFor(() => expect(handlers["library-scan"]).toBeDefined());
    act(() => handlers["library-scan"]?.({ payload: event("identifying", 120, 900) }));
    expect(screen.getByText("Identifying 120 of 900")).toBeInTheDocument();
  });

  it("adds a folder from Windows' picker, or a typed path", async () => {
    renderIt();
    await screen.findByText("D:\\Films");
    fireEvent.click(screen.getByRole("button", { name: "Add a folder…" }));
    await waitFor(() => expect(commands.addLibraryFolder).toHaveBeenCalledWith("E:\\TV"));

    fireEvent.change(screen.getByLabelText("Or type its path"), { target: { value: "  F:\\Anime " } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => expect(commands.addLibraryFolder).toHaveBeenCalledWith("F:\\Anime"));
  });

  it("a cancelled picker adds nothing", async () => {
    commands.pickLibraryFolder.mockImplementationOnce(() => ok(null));
    renderIt();
    await screen.findByText("D:\\Films");
    fireEvent.click(screen.getByRole("button", { name: "Add a folder…" }));
    await waitFor(() => expect(commands.pickLibraryFolder).toHaveBeenCalled());
    expect(commands.addLibraryFolder).not.toHaveBeenCalled();
  });

  it("rescans and removes a folder", async () => {
    renderIt();
    await screen.findByText("D:\\Films");
    fireEvent.click(screen.getByRole("button", { name: "Rescan D:\\Films" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove D:\\Films" }));
    await waitFor(() => expect(commands.rescanLibraryFolder).toHaveBeenCalledWith(1));
    await waitFor(() => expect(commands.removeLibraryFolder).toHaveBeenCalledWith(1));
  });
});
