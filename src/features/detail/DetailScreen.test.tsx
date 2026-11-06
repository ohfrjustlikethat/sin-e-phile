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

const NO_OFFERS = { languages: [], original_language: null, qualities: [], unmet: null };
const PLAN = { title: "Sunrise", open: "https://films.example/s.mp4", start: null, from: "Internet Archive", offered: 3 };

let resolvePlan: (v: unknown) => void = () => {};
const commands = {
  itemDetail: vi.fn(() => ok(DETAIL)),
  preparePlay: vi.fn(() => new Promise((r) => (resolvePlan = r))),
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
    usePlaybackChoice.setState({ audio: null, quality: null });
  });

  it("asks the sources the moment it opens, before Play is pressed", async () => {
    renderIt();
    expect(commands.preparePlay).toHaveBeenCalledWith(7, null, null);
    expect(await screen.findByText("Sunrise")).toBeInTheDocument();
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

  it("says why nothing can play", async () => {
    renderIt();
    await screen.findByText("Sunrise");
    resolvePlan({ status: "error", error: "none of your sources has this" });
    expect(await screen.findByText("none of your sources has this")).toBeInTheDocument();
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
    expect(commands.preparePlay).toHaveBeenCalledWith(7, "hi", null);
    await screen.findByText("Sunrise");
    resolvePlan({ status: "ok", data: { ...PLAN, languages: ["en"], qualities: [], unmet: "carries en, not hi", original_language: null } });
    expect(await screen.findByText("None of your sources has Hindi audio.")).toBeInTheDocument();
    // Still playable, and the choice stays visible so it can be changed.
    expect(screen.getByRole("button", { name: "Play" })).toBeEnabled();
    expect(screen.getByRole("radio", { name: "Hindi" })).toHaveAttribute("aria-checked", "true");
  });
});
