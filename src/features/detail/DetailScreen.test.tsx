import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";
import { usePlaybackChoice } from "@/lib/playbackChoice";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });

const DETAIL = {
  id: 7, title: "Sunrise", year: 1927, kind: "film", runtime_minutes: 94,
  synopsis: "A married farmer falls under the spell of a woman from the city.",
  credits: [
    { name: "F. W. Murnau", role: "director", character: null },
    { name: "George O'Brien", role: "actor", character: "The Man" },
  ],
};

const NO_OFFERS = { languages: [], original_language: null, qualities: [], unmet: null, why: "", candidates: [] };
const PLAN = { title: "Sunrise", open: "https://films.example/s.mp4", start: null, from: "Internet Archive", offered: 3, why: "", candidates: [] };
const term = (name: string, value: number, weight: number, why: string) => ({ name, value, weight, why });
const RANKED = [
  {
    title: "Sunrise (1927)", from: "Internet Archive", transport: "http", quality: "720p", codec: "h264", total: 5.5, sunk: null, unmet: null,
    local: false, chosen: true,
    terms: [term("language", 1, 4, "en audio"), term("quality", 0.5, 3, "720p, below the 1080p this machine plays")],
  },
  {
    title: "Sunrise", from: "Your folder", transport: "local", quality: "1080p", codec: "unknown", total: -3, sunk: "not a video file (.exe)", unmet: null,
    local: true, chosen: false, terms: [term("encoding", -1, 1, "not a video file (.exe)"), term("quality", -0.67, 3, "x")],
  },
];

let resolvePlan: (v: unknown) => void = () => {};
/** The fake watchlist: whether the film on the page is saved. */
let saved = false;
const commands = {
  itemDetail: vi.fn(() => ok(DETAIL)),
  preparePlay: vi.fn(() => new Promise((r) => (resolvePlan = r))),
  downloadAdvice: vi.fn(() => ok(null)),
  seriesDetail: vi.fn(() =>
    ok({ seasons: [{ number: 0, episodes: 1 }, { number: 1, episodes: 2 }, { number: 2, episodes: 1 }], next_airing: { episode: 7, airs_at: 4102444800 }, watched: 1, on_this_pc: 0, also_here: [] as { path: string; season: number | null; episode: number | null; title: string | null }[] }),
  ),
  titleWatched: vi.fn(() => ok(false)),
  titleWatchlisted: vi.fn(() => ok(saved)),
  setWatchlisted: vi.fn((_id: number, on: boolean) => {
    saved = on;
    return ok(null);
  }),
  setWatched: vi.fn((_scope: unknown, _watched: boolean) => ok(1)),
  seasonEpisodes: vi.fn((_id: number, season: number | null) =>
    ok(
      season === 1
        ? [{ id: 71, season: 1, episode: 1, title: "Pilot", air_date: "1990-04-08", watched: true, on_this_pc: false }, { id: 72, season: 1, episode: 2, title: "Traces to Nowhere", air_date: null, watched: false, on_this_pc: true }]
        : [{ id: 81, season: 2, episode: 1, title: "May the Giant Be with You", air_date: null, watched: false, on_this_pc: false }],
    ),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { DetailScreen } = await import("./DetailScreen");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <DetailScreen />
    </QueryClientProvider>,
  );
}

describe("DetailScreen (SPEC.md Phase 8)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUi.setState({ detail: 7, playing: null });
    usePlaybackChoice.setState({ audio: null, quality: null, details: false });
  });

  it("asks the sources the moment it opens, before Play is pressed", async () => {
    renderIt();
    expect(await screen.findByText("Sunrise")).toBeInTheDocument();
    expect(commands.preparePlay).toHaveBeenCalledWith(7, null, null);
    expect(screen.getByText("Asking your sources…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Play" })).toBeDisabled();
  });

  it("offers to resume, says where from, and starts the player", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { title: "Sunrise", open: "https://films.example/s.mp4", start: 1200, from: "Internet Archive", offered: 3, ...NO_OFFERS } });
    const button = await screen.findByRole("button", { name: "Resume from 20:00" });
    expect(screen.getByText("From Internet Archive")).toBeInTheDocument();
    fireEvent.click(button);
    await waitFor(() =>
      expect(useUi.getState().playing).toEqual({ target: "https://films.example/s.mp4", title: "Sunrise", mediaItemId: 7, start: 1200 }),
    );
  });

  it("offers to start over beside Resume, and only when there is a place to resume from", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { title: "Sunrise", open: "https://films.example/s.mp4", start: 1200, from: "Internet Archive", offered: 3, ...NO_OFFERS } });
    fireEvent.click(await screen.findByRole("button", { name: "Start over" }));
    await waitFor(() =>
      expect(useUi.getState().playing).toEqual({ target: "https://films.example/s.mp4", title: "Sunrise", mediaItemId: 7, start: null }),
    );
  });

  it("offers no Start over for a film that has not been started", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { ...PLAN, ...NO_OFFERS } });
    await screen.findByRole("button", { name: "Play" });
    expect(screen.queryByRole("button", { name: "Start over" })).not.toBeInTheDocument();
  });

  it("says why nothing can play", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "error", error: "none of your sources has this" });
    expect(await screen.findByText("none of your sources has this")).toBeInTheDocument();
    // Phase 16: said plainly, with the way to fix it one press away.
    expect(screen.getByText(/comes with no sources of its own/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add a source" }));
    expect(useUi.getState().settingsOpen).toBe(true);
    expect(screen.getByRole("button", { name: "Play" })).toBeDisabled();
  });

  it("offers the languages and qualities the copies carry, original and dub told apart", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({
      status: "ok",
      data: { ...PLAN, languages: ["ja", "en"], original_language: "ja", qualities: ["full_hd", "hd"], unmet: null },
    });
    const audio = await screen.findByRole("radiogroup", { name: "Audio" });
    expect(within(audio).getAllByRole("radio").map((b) => b.textContent)).toEqual([
      "Auto",
      "Japanese · original",
      "English · dub",
    ]);
    const quality = screen.getByRole("radiogroup", { name: "Quality" });
    expect(within(quality).getAllByRole("radio").map((b) => b.textContent)).toEqual(["Best", "1080p", "720p"]);
    expect(within(audio).getByRole("radio", { name: "Auto" })).toHaveAttribute("aria-checked", "true");

    fireEvent.click(within(audio).getByRole("radio", { name: "English · dub" }));
    expect(commands.preparePlay).toHaveBeenLastCalledWith(7, "en", null);
    expect(usePlaybackChoice.getState().audio).toBe("en");
  });

  it("says plainly when no copy has the chosen language", async () => {
    usePlaybackChoice.setState({ audio: "hi", quality: null });
    renderIt();
    await screen.findByText("Sunrise");
    expect(commands.preparePlay).toHaveBeenCalledWith(7, "hi", null);
    resolvePlan({ status: "ok", data: { ...PLAN, languages: ["en"], qualities: [], unmet: "carries en, not hi", original_language: null } });
    expect(await screen.findByText("None of your sources has Hindi audio.")).toBeInTheDocument();
    // Still playable, and the choice stays visible so it can be changed.
    expect(screen.getByRole("button", { name: "Play" })).toBeEnabled();
    expect(screen.getByRole("radio", { name: "Hindi" })).toHaveAttribute("aria-checked", "true");
  });

  it("says why this source, under Play", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { ...PLAN, ...NO_OFFERS, why: "1080p · plays everywhere · 120 seeders" } });
    expect(await screen.findByLabelText("Why this source")).toHaveTextContent("1080p · plays everywhere · 120 seeders");
  });

  it("Ctrl+Shift+S shows every source with the arithmetic of its score (E5)", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { ...PLAN, ...NO_OFFERS, candidates: RANKED } });
    await screen.findByText("From Internet Archive");
    expect(screen.queryByRole("region", { name: "Sources, ranked" })).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "S", ctrlKey: true, shiftKey: true });
    const panel = await screen.findByRole("region", { name: "Sources, ranked" });
    const items = within(panel).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    // Value × weight = points, and the points sum to the total the backend ranked by.
    const first = items[0]!;
    const points = [...first.querySelectorAll("[data-points]")].map((e) => e.textContent);
    expect(points).toEqual(["+4.00", "+1.50"]);
    expect(first.querySelector("[data-total]")).toHaveTextContent("+5.50");
    expect(first).toHaveTextContent("Plays");
    expect(items[1]).toHaveTextContent("Last: not a video file (.exe)");

    fireEvent.keyDown(window, { key: "S", ctrlKey: true, shiftKey: true });
    expect(screen.queryByRole("region", { name: "Sources, ranked" })).not.toBeInTheDocument();
  });

  it("the Settings switch keeps the panel on", async () => {
    usePlaybackChoice.setState({ details: true });
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { ...PLAN, ...NO_OFFERS, candidates: RANKED } });
    expect(await screen.findByRole("region", { name: "Sources, ranked" })).toBeInTheDocument();
  });

  it("a series lists its seasons and episodes, says when the next airs, and asks no sources", async () => {
    commands.itemDetail.mockImplementationOnce(() => ok({ ...DETAIL, title: "Twin Peaks", kind: "series" }));
    renderIt();
    expect(await screen.findByText("Pilot")).toBeInTheDocument();
    expect(screen.getByLabelText("Next episode")).toHaveTextContent(/^Episode 7 airs/);
    expect(commands.preparePlay).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Play" })).not.toBeInTheDocument();
    // Season 1 first, not the specials; season 2 on choosing it.
    expect(commands.seasonEpisodes).toHaveBeenCalledWith(7, 1);
    fireEvent.click(screen.getByRole("radio", { name: "Season 2" }));
    expect(await screen.findByText("May the Giant Be with You")).toBeInTheDocument();
    fireEvent.click(screen.getByText("May the Giant Be with You"));
    expect(useUi.getState().detail).toBe(81);
  });

  it("a series says what is on this PC, and lists its own files the catalogue cannot place, in order", async () => {
    // Phase 16: the author's Twin Peaks, whose third season IMDb lists as another series.
    commands.itemDetail.mockImplementationOnce(() => ok({ ...DETAIL, title: "Twin Peaks", kind: "series" }));
    commands.seriesDetail.mockImplementationOnce(() =>
      ok({
        seasons: [{ number: 1, episodes: 2 }], next_airing: null as never, watched: 0, on_this_pc: 1,
        also_here: [
          { path: "D:/TV/Twin Peaks/Season 3/Twin Peaks - S03E01 - Part 1.mkv", season: 3, episode: 1, title: "Part 1" },
          { path: "D:/TV/Twin Peaks/Season 3/Twin Peaks - S03E02 - Part 2.mkv", season: 3, episode: 2, title: null },
        ],
      }),
    );
    renderIt();
    expect(await screen.findByLabelText("On this PC")).toHaveTextContent("3 episodes on this PC");
    const traces = (await screen.findByText("Traces to Nowhere")).closest("button") as HTMLElement;
    expect(traces).toHaveTextContent("On this PC");
    expect(screen.getByText("Pilot").closest("button")).not.toHaveTextContent("On this PC");
    const also = screen.getByRole("region", { name: "Also on this PC" });
    const rows = within(also).getAllByRole("button");
    expect(rows.map((r) => r.textContent)).toEqual(["S3 · E1Part 1Play", "S3 · E2Twin Peaks - S03E02 - Part 2Play"]);
    fireEvent.click(rows[1] as HTMLElement);
    expect(useUi.getState().playing?.target).toBe("D:/TV/Twin Peaks/Season 3/Twin Peaks - S03E02 - Part 2.mkv");
  });

  it("marks a film watched, and Resume no longer offers a place that is gone (Appendix F, I21)", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { title: "Sunrise", open: "https://films.example/s.mp4", start: 1200, from: "Internet Archive", offered: 3, ...NO_OFFERS } });
    await screen.findByRole("button", { name: "Resume from 20:00" });
    fireEvent.click(await screen.findByRole("button", { name: "Mark as watched" }));
    await waitFor(() => expect(commands.setWatched).toHaveBeenCalledWith({ kind: "title", id: 7 }, true));
    expect(await screen.findByRole("button", { name: "Play" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start over" })).not.toBeInTheDocument();
  });

  it("a film already watched offers the way back", async () => {
    commands.titleWatched.mockImplementationOnce(() => ok(true));
    renderIt();
    await screen.findByText("Sunrise");
    fireEvent.click(await screen.findByRole("button", { name: "Watched · mark as not watched" }));
    await waitFor(() => expect(commands.setWatched).toHaveBeenCalledWith({ kind: "title", id: 7 }, false));
  });

  it("a series says how much is seen, ticks watched episodes, and marks a season or the show", async () => {
    commands.itemDetail.mockImplementationOnce(() => ok({ ...DETAIL, title: "Twin Peaks", kind: "series" }));
    renderIt();
    const pilot = (await screen.findByText("Pilot")).closest("button")!;
    expect(within(pilot).getByText("Watched")).toBeInTheDocument();
    expect(within(screen.getByText("Traces to Nowhere").closest("button")!).queryByText("Watched")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Watched")).toHaveTextContent("Seen 1 of 4");
    // One of season 1's two is seen, so the season is offered as watched, not as unwatched.
    fireEvent.click(screen.getByRole("button", { name: "Mark this season as watched" }));
    await waitFor(() => expect(commands.setWatched).toHaveBeenCalledWith({ kind: "season", series: 7, season: 1 }, true));
    fireEvent.click(screen.getByRole("button", { name: "Mark the whole show as watched" }));
    await waitFor(() => expect(commands.setWatched).toHaveBeenCalledWith({ kind: "series", id: 7 }, true));
  });

  it("saves a film to the watchlist and takes it off again (Appendix F, I39)", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    fireEvent.click(await screen.findByRole("button", { name: "Add to watchlist" }));
    await waitFor(() => expect(commands.setWatchlisted).toHaveBeenCalledWith(7, true));
    fireEvent.click(await screen.findByRole("button", { name: "On your watchlist · remove" }));
    await waitFor(() => expect(commands.setWatchlisted).toHaveBeenCalledWith(7, false));
    expect(await screen.findByRole("button", { name: "Add to watchlist" })).toBeInTheDocument();
  });

  it("a series can be saved too", async () => {
    commands.itemDetail.mockImplementationOnce(() => ok({ ...DETAIL, title: "Twin Peaks", kind: "series" }));
    renderIt();
    await screen.findByText("Pilot");
    expect(await screen.findByRole("button", { name: "Add to watchlist" })).toBeInTheDocument();
  });
});
