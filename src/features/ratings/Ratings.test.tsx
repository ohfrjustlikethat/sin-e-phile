import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const card = (id: number, title: string) => ({
  id, title, year: 1994, made_by: "Frank Darabont", runtime_minutes: 142, note: null, progress: null, is_show: false,
});
const commands = {
  toRate: vi.fn((_n: number) => ok([card(1, "The Shawshank Redemption"), card(2, "Pulp Fiction")])),
  skipRating: vi.fn((_id: number) => ok(null)),
  ratedCount: vi.fn(() => ok(3)),
  titleRating: vi.fn((_id: number) => ok<number | null>(null)),
  rateTitle: vi.fn((_id: number, _h: number | null) => ok(null)),
  showRatings: vi.fn((_s: number) => ok<{ seasons: { season: number; halves: number }[]; average: number | null }>({ seasons: [], average: null })),
  rateSeason: vi.fn((_s: number, _season: number, _h: number | null) => ok(null)),
  inContinueWatching: vi.fn((_id: number) => ok(true)),
  removeFromContinue: vi.fn((_id: number) => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { RateScreen } = await import("./RateScreen");
const { RateCard } = await import("./RateCard");
const { ContinueRemoval, RateRow, SeasonHeader, ShowAverage } = await import("./TitleRating");

function renderIt(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

/** ADR-0056, the author's answers: ratings for any title. */
describe("ratings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUi.setState({ rateOpen: false, detail: null });
  });

  it("rate films one after another: stars first, Next once rated, or Haven't seen it", async () => {
    renderIt(<RateScreen />);
    expect(await screen.findByRole("heading", { name: "The Shawshank Redemption" })).toBeInTheDocument();
    expect(screen.getByText("1 of 2")).toBeInTheDocument();
    expect(screen.getByLabelText("Rated so far")).toHaveTextContent("3 rated");
    const stars = screen.getByRole("slider", { name: "Your rating" });
    expect(document.activeElement).toBe(stars);
    const next = screen.getByRole("button", { name: "Next" });
    expect(next).toBeDisabled();
    fireEvent.change(stars, { target: { value: "9" } });
    expect(await screen.findByText("4½ stars")).toBeInTheDocument();
    fireEvent.click(next);
    expect(await screen.findByRole("heading", { name: "Pulp Fiction" })).toBeInTheDocument();
    await waitFor(() => expect(commands.rateTitle).toHaveBeenCalledWith(1, 9));
    fireEvent.click(screen.getByRole("button", { name: "Haven’t seen it" }));
    await waitFor(() => expect(commands.skipRating).toHaveBeenCalledWith(2));
    // The batch is done: the next is asked for, leaving out what was rated or passed over.
    await waitFor(() => expect(commands.toRate).toHaveBeenCalledTimes(2));
  });

  it("Home offers it under ten ratings, and not at ten", async () => {
    const { unmount } = renderIt(<RateCard />);
    const offer = await screen.findByRole("region", { name: "Rate films" });
    expect(offer).toHaveTextContent("3 of 10 rated");
    fireEvent.click(screen.getByRole("button", { name: "Rate films" }));
    expect(useUi.getState().rateOpen).toBe(true);
    unmount();
    commands.ratedCount.mockImplementationOnce(() => ok(10));
    renderIt(<RateCard />);
    await waitFor(() => expect(commands.ratedCount).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("region", { name: "Rate films" })).toBeNull();
  });

  it("a film's page: stars on their own line, Rate until rated", async () => {
    commands.titleRating.mockImplementationOnce(() => ok(7));
    renderIt(<RateRow id={7} />);
    expect(await screen.findByText("3½ stars")).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Your rating" })).toHaveAttribute("aria-valuetext", "3½ stars");
  });

  it("a show's page: a season rated on its header, the show showing the average", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    commands.showRatings.mockImplementation(() => ok({ seasons: [{ season: 1, halves: 10 }], average: 10 }));
    renderIt(
      <>
        <ShowAverage id={5} />
        <SeasonHeader series={5} season={2} name="Season 2" episodes={10} />
      </>,
    );
    expect(await screen.findByRole("img", { name: "Your average: 5 stars" })).toBeInTheDocument();
    expect(screen.getByText("10 episodes")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("slider", { name: "Your rating of Season 2" }), { target: { value: "6" } });
    expect(await screen.findByRole("img", { name: "Your average: 4 stars" })).toBeInTheDocument();
    await act(async () => {
      vi.advanceTimersByTime(400);
    });
    vi.useRealTimers();
    expect(commands.rateSeason).toHaveBeenCalledWith(5, 2, 6);
    commands.showRatings.mockReset();
  });

  it("a title in Continue watching can be removed from its page", async () => {
    renderIt(<ContinueRemoval id={9} />);
    fireEvent.click(await screen.findByRole("button", { name: "Remove from Continue watching" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Removed from Continue watching");
    expect(commands.removeFromContinue).toHaveBeenCalledWith(9);
  });
});
