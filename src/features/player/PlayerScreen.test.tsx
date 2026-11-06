import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";
import { usePlaybackChoice } from "@/lib/playbackChoice";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement } from "react";

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
  pauseDetails: vi.fn(() =>
    ok({
      cast: [{ person_id: 5, name: "Max Schreck", part: "Count Orlok" }],
      crew: [{ person_id: 6, name: "F. W. Murnau", part: "director" }],
      ratings: [{ source: "IMDb", score: 7.9, votes: 120000 }],
      more_like_this: [{ id: 9, title: "Vampyr", year: 1932, kind: "film", part: null }],
    }),
  ),
  filmography: vi.fn(() => ok([{ id: 11, title: "Faust", year: 1926, kind: "film", part: "Mephisto" }])),
  playerChapters: vi.fn(() => ok([])),
  episodeNeighbours: vi.fn(() =>
    ok({ previous: null, next: null } as { previous: unknown; next: unknown }),
  ),
  preparePlay: vi.fn(() => ok({ title: "Next", open: "https://films.example/next.mp4", start: null })),
  rateTitle: vi.fn(() => ok(null)),
  titleRating: vi.fn(() => ok(null)),
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
const { PlayerScreen: Bare } = await import("./PlayerScreen");
/** The player inside the query client the app gives it. */
function PlayerScreen(): ReactElement {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={client}>
      <Bare />
    </QueryClientProvider>
  );
}
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

  it("the pause panel shows who is in it and who made it, before the still arrives (Phase 11)", async () => {
    commands.playerPauseStill.mockImplementationOnce(() => new Promise(() => {}));
    render(<PlayerScreen />);
    await waitFor(() => expect(commands.pauseDetails).toHaveBeenCalledWith(1));
    fireEvent.keyDown(window, { key: " " });
    // No still yet: the overlay is already there.
    const panel = await screen.findByRole("complementary", { name: "About this film" });
    expect(panel).toHaveTextContent("Max Schreck");
    expect(panel).toHaveTextContent("Count Orlok");
    expect(panel).toHaveTextContent("Directed by F. W. Murnau");
    expect(panel).toHaveTextContent("IMDb 7.9");
    expect(panel).toHaveTextContent("Vampyr");
  });

  it("a cast member opens their films, and a film leaves the player for its page", async () => {
    render(<PlayerScreen />);
    await waitFor(() => expect(commands.pauseDetails).toHaveBeenCalled());
    fireEvent.keyDown(window, { key: " " });
    fireEvent.click(await screen.findByRole("button", { name: /Max Schreck/ }));
    expect(commands.filmography).toHaveBeenCalledWith(5);
    fireEvent.click(await screen.findByRole("button", { name: /Faust/ }));
    expect(useUi.getState().playing).toBeNull();
    expect(useUi.getState().detail).toBe(11);
  });

  const NEXT = { id: 42, season: 1, episode: 2, title: "Traces to Nowhere", air_date: null };
  const STATE = {
    position: 0, duration: 1440, paused: false, buffered_until: null, volume: 100, muted: false, speed: 1,
    hwdec: null, ended: false, started: true, chapter: null, tracks: [],
  };
  async function inTauri(run: () => Promise<void>) {
    const w = window as unknown as Record<string, unknown>;
    w.__TAURI_INTERNALS__ = {};
    try {
      await run();
    } finally {
      delete w.__TAURI_INTERNALS__;
    }
  }

  it("N plays the next episode (Phase 11)", async () => {
    commands.episodeNeighbours.mockImplementation(() => ok({ previous: null, next: NEXT }));
    render(<PlayerScreen />);
    await waitFor(() => expect(commands.episodeNeighbours).toHaveBeenCalledWith(1));
    await waitFor(() => {
      fireEvent.keyDown(window, { key: "n" });
      expect(commands.preparePlay).toHaveBeenCalledWith(42, null, null);
    });
    await waitFor(() => expect(useUi.getState().playing?.mediaItemId).toBe(42));
    commands.episodeNeighbours.mockImplementation(() => ok({ previous: null, next: null }));
  });

  it("offers the next episode in the last 30 s; B says not now, A plays it", async () => {
    commands.episodeNeighbours.mockImplementation(() => ok({ previous: null, next: NEXT }));
    usePlaybackChoice.setState({ autoplay: false });
    await inTauri(async () => {
      render(<PlayerScreen />);
      await waitFor(() => expect(events.on["player-state"]).toBeDefined());
      await waitFor(() => expect(commands.episodeNeighbours).toHaveBeenCalled());
      act(() => events.on["player-state"]?.({ payload: { ...STATE, position: 1300 } }));
      expect(screen.queryByRole("dialog", { name: "Next episode" })).not.toBeInTheDocument();
      act(() => events.on["player-state"]?.({ payload: { ...STATE, position: 1415 } }));
      expect(await screen.findByRole("dialog", { name: "Next episode" })).toHaveTextContent("Traces to Nowhere");
      // Autoplay off: no countdown, so nothing plays by itself.
      expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
      expect(screen.queryByText(/Play in/)).not.toBeInTheDocument();
      fireEvent.keyDown(window, { key: "Escape" });
      expect(screen.queryByRole("dialog", { name: "Next episode" })).not.toBeInTheDocument();
      // B dismisses the card; it does not leave the player.
      expect(useUi.getState().playing).not.toBeNull();
    });
    usePlaybackChoice.setState({ autoplay: true });
    commands.episodeNeighbours.mockImplementation(() => ok({ previous: null, next: null }));
  });

  it("autoplay plays the next episode when the countdown ends", async () => {
    commands.episodeNeighbours.mockImplementation(() => ok({ previous: null, next: NEXT }));
    await inTauri(async () => {
      render(<PlayerScreen />);
      await waitFor(() => expect(events.on["player-state"]).toBeDefined());
      await waitFor(() => expect(commands.episodeNeighbours).toHaveBeenCalled());
      vi.useFakeTimers();
      act(() => events.on["player-state"]?.({ payload: { ...STATE, position: 1430 } }));
      expect(screen.getByRole("dialog", { name: "Next episode" })).toHaveTextContent("Play in 10");
      for (let i = 0; i < 10; i++) act(() => vi.advanceTimersByTime(1000));
      vi.useRealTimers();
      await waitFor(() => expect(commands.preparePlay).toHaveBeenCalledWith(42, null, null));
    });
    commands.episodeNeighbours.mockImplementation(() => ok({ previous: null, next: null }));
  });

  it("a film's credits play out, then the end card asks for a rating", async () => {
    await inTauri(async () => {
      render(<PlayerScreen />);
      await waitFor(() => expect(events.on["player-state"]).toBeDefined());
      act(() => events.on["player-state"]?.({ payload: { ...STATE, position: 1440, ended: true } }));
      expect(await screen.findByRole("dialog", { name: "Finished" })).toHaveTextContent("Nosferatu");
      expect(commands.playerClose).toHaveBeenCalled();
      fireEvent.click(screen.getByRole("radio", { name: "4 out of 5" }));
      expect(commands.rateTitle).toHaveBeenCalledWith(1, 4);
    });
  });
});

