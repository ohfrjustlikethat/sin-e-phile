import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

type Handler = (e: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, h: Handler) => {
    handlers[name] = h;
    return Promise.resolve(() => undefined);
  }),
}));
const { DownloadNotices } = await import("./DownloadNotices");

describe("completion notices (SPEC.md Phase 13)", () => {
  it("says a download finished, or stopped, by title", async () => {
    render(<DownloadNotices />);
    await act(async () => {});
    act(() => handlers["download-finished"]?.({ payload: "Sunrise" }));
    expect(screen.getByText("Sunrise has downloaded, and is in your library.")).toBeInTheDocument();
    act(() => handlers["download-failed"]?.({ payload: "Tabu" }));
    expect(screen.getByText("Tabu stopped downloading. Downloads says why.")).toBeInTheDocument();
  });
});
