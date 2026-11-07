import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const option = (id: number, title: string, year: number, kind = "series") => ({
  media_item_id: id,
  kind,
  title,
  year,
  runtime_minutes: 24,
  made_by: "Masayuki Kojima",
  synopsis: "A surgeon hunts a killer he once saved.",
});
const ITEMS = [
  {
    file_id: 1,
    name: "Monster - 01 [DVD].mkv",
    folders: "Anime / Monster",
    minutes: 24,
    height: 480,
    read_title: "Monster",
    read_year: null,
    read_episode: "Episode 1",
    same_name: 4,
    options: [option(10, "Monster", 2004), option(11, "Monster", 2004)],
  },
  {
    file_id: 2,
    name: "Halloween.mkv",
    folders: "",
    minutes: 91,
    height: 1080,
    read_title: "Halloween",
    read_year: null,
    read_episode: "",
    same_name: 0,
    options: [option(20, "Halloween", 1978, "film"), option(21, "Halloween", 2018, "film")],
  },
];
let queue = [...ITEMS];
const commands = {
  libraryReview: vi.fn(() => ok(queue)),
  resolveReview: vi.fn((fileId: number) => {
    queue = queue.filter((i) => i.file_id !== fileId);
    return ok(fileId === 1 ? 4 : 0);
  }),
  notATitle: vi.fn((fileId: number) => {
    queue = queue.filter((i) => i.file_id !== fileId);
    return ok(null);
  }),
  search: vi.fn(() => ok({ hits: [] })),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { ReviewScreen, facts, fileFacts } = await import("./ReviewScreen");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ReviewScreen />
    </QueryClientProvider>,
  );
}

describe("the review queue (SPEC.md Phase 12)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queue = [...ITEMS];
    useUi.setState({ settingsOpen: true, reviewOpen: true });
  });

  it("describes a candidate and a file in one line each", () => {
    expect(facts({ year: 1995, kind: "film", runtime_minutes: 170, made_by: "Michael Mann" })).toBe(
      "1995 · Film · 170 min · Michael Mann",
    );
    expect(fileFacts({ minutes: 112, height: 1080 })).toBe("1:52 · 1080p");
    expect(fileFacts({ minutes: 24, height: null })).toBe("24 min");
  });

  it("shows the file, how it was read, and the candidates with the first focused", async () => {
    renderIt();
    expect(await screen.findByText("Monster - 01 [DVD].mkv")).toBeInTheDocument();
    expect(screen.getByText("Episode 1")).toBeInTheDocument();
    expect(screen.getByText("Your answer also settles 4 more named the same way.")).toBeInTheDocument();
    const cards = screen.getAllByRole("button", { name: /Monster/ });
    await waitFor(() => expect(document.activeElement).toBe(cards[0]));
  });

  it("a number answers, says what else it settled, and moves on", async () => {
    renderIt();
    await screen.findByText("Monster - 01 [DVD].mkv");
    fireEvent.keyDown(window, { key: "2" });
    await waitFor(() => expect(commands.resolveReview).toHaveBeenCalledWith(1, 11));
    expect(await screen.findByText("Settled 4 more named the same way.")).toBeInTheDocument();
    expect(await screen.findByText("Halloween.mkv")).toBeInTheDocument();
  });

  it("N says it is not a film, L leaves it for later", async () => {
    renderIt();
    await screen.findByText("Monster - 01 [DVD].mkv");
    fireEvent.keyDown(window, { key: "l" });
    expect(await screen.findByText("Halloween.mkv")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "n" });
    await waitFor(() => expect(commands.notATitle).toHaveBeenCalledWith(2));
    expect(await screen.findByText("Monster - 01 [DVD].mkv")).toBeInTheDocument();
  });

  it("an empty queue says so, and Escape returns to Settings", async () => {
    queue = [];
    renderIt();
    expect(await screen.findByText(/Every file in your library is identified/)).toBeInTheDocument();
    act(() => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    expect(useUi.getState().reviewOpen).toBe(false);
    expect(useUi.getState().settingsOpen).toBe(true);
  });
});
