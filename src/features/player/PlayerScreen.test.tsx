import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });

const commands = {
  playerOpen: vi.fn(() => ok(null)),
  playerClose: vi.fn(() => ok(null)),
  playerLayout: vi.fn(() => ok(null)),
  playerAction: vi.fn(() => ok(null)),
  playerPauseStill: vi.fn(() => ok("data:image/jpeg;base64,AAAA")),
  playerResumeVideo: vi.fn(() => ok(null)),
  savePosition: vi.fn(() => ok(null)),
  subtitleNudge: vi.fn((ms: number) => ok(ms * 3)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
// Inert unless a test marks the page as inside Tauri.
const events = vi.hoisted(() => ({ on: {} as Record<string, (e: { payload: unknown }) => void> }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, cb: (e: { payload: unknown }) => void) => {
    events.on[name] = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ setFullscreen: () => Promise.resolve() }),
}));
const { PlayerScreen } = await import("./PlayerScreen");
const { sendPlayerIntent } = await import("./intentBus");

beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});

describe("PlayerScreen (SPEC.md Phase 8)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUi.setState({ playing: { target: "https://films.example/a.mp4", title: "Nosferatu", mediaItemId: 1, start: 42 } });
  });

  it("opens what was asked for, from where it was left", () => {
    render(<PlayerScreen />);
    expect(commands.playerOpen).toHaveBeenCalledWith("https://films.example/a.mp4", 42);
  });

  it("says what is opening until the first frame, not a black screen", () => {
    render(<PlayerScreen />);
    expect(screen.getByText("Resuming from 0:42")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Nosferatu" })).toBeInTheDocument();
  });

  it("cuts both chrome panels out of the video while they show (ADR-0020)", () => {
    render(<PlayerScreen />);
    const calls = commands.playerLayout.mock.calls as unknown as unknown[][];
    const cutouts = calls.at(-1)?.[5] as unknown[];
    expect(cutouts).toHaveLength(2);
  });

  it("pauses into a still frame and resumes from it (ADR-0021)", async () => {
    render(<PlayerScreen />);
    fireEvent.keyDown(window, { key: " " });
    await waitFor(() => expect(commands.playerPauseStill).toHaveBeenCalled());
    expect(await screen.findByText(/Paused/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    await waitFor(() => expect(commands.playerResumeVideo).toHaveBeenCalled());
  });

  it("maps the spec's keys and the controller's intents to actions", async () => {
    render(<PlayerScreen />);
    fireEvent.keyDown(window, { key: "l" });
    expect(commands.playerAction).toHaveBeenCalledWith({ kind: "seek_by", seconds: 10 });
    fireEvent.keyDown(window, { key: "." });
    expect(commands.playerAction).toHaveBeenCalledWith({ kind: "frame_step", forward: true });
    act(() => {
      sendPlayerIntent({ kind: "seek", seconds: -30 });
    });
    expect(commands.playerAction).toHaveBeenCalledWith({ kind: "seek_by", seconds: -30 });
  });

  it("saves the position it reached on the way out (Phase 8 E4)", async () => {
    events.on = {};
    const w = window as unknown as Record<string, unknown>;
    w.__TAURI_INTERNALS__ = {};
    try {
      const { unmount } = render(<PlayerScreen />);
      await waitFor(() => expect(events.on["player-state"]).toBeDefined());
      act(() =>
        events.on["player-state"]?.({
          payload: {
            position: 2712.4, duration: 5700, paused: false, buffered_until: null, volume: 100,
            muted: false, speed: 1, hwdec: null, ended: false, started: true, tracks: [],
          },
        }),
      );
      unmount();
      expect(commands.savePosition).toHaveBeenCalledWith(1, 2712.4, 5700);
    } finally {
      delete w.__TAURI_INTERNALS__;
    }
  });

  it("leaves on Escape and closes the player", async () => {
    const { unmount } = render(<PlayerScreen />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(useUi.getState().playing).toBeNull();
    unmount();
    expect(commands.playerClose).toHaveBeenCalled();
  });

  it("says so when no copy would start, after trying them all (Phase 9)", async () => {
    const w = window as unknown as { __TAURI_INTERNALS__?: object };
    w.__TAURI_INTERNALS__ = {};
    try {
      render(<PlayerScreen />);
      await waitFor(() => expect(events.on["player-failed"]).toBeDefined());
      act(() => events.on["player-failed"]?.({ payload: "no copy would start: echo: did not load: 404" }));
      expect(await screen.findByText("no copy would start: echo: did not load: 404")).toBeInTheDocument();
      expect(screen.queryByText("Resuming from 0:42")).not.toBeInTheDocument();
    } finally {
      delete w.__TAURI_INTERNALS__;
    }
  });

  it("G nudges the subtitles and says where they are now (Phase 10)", async () => {
    render(<PlayerScreen />);
    fireEvent.keyDown(window, { key: "g" });
    expect(commands.subtitleNudge).toHaveBeenCalledWith(-100);
    expect(await screen.findByText("Subtitles −0.3 s")).toBeInTheDocument();
  });
});
