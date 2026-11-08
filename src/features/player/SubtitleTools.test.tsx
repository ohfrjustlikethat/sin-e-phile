import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const settings = { languages: ["en"], always: false, font: "Segoe UI", size: 46, position: 95, background: false,
  colour: "#FFFFFF", outline: 3 };
const commands = {
  subtitleSettings: vi.fn(() => ok({ settings, has_key: true, used_today: 0, budget: 5, server_remaining: null })),
  setSubtitleSettings: vi.fn(() => ok(null)),
  subtitleOffers: vi.fn(() =>
    ok([
      { file_id: "3", language: "en", hash_match: true, downloads: 5, release: "Film.2019.BluRay" },
      { file_id: "2", language: "en", hash_match: false, downloads: 10, release: "Film.2019.WEB" },
    ]),
  ),
  subtitleChoose: vi.fn(() => ok("aligned: offset +1200 ms")),
  subtitleLoad: vi.fn(() => ok(null)),
  subtitleRealign: vi.fn(() => Promise.resolve({ status: "error" as const, error: "Only a subtitle file can be aligned again; this one is part of the film." })),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { SubtitleTools, nextFont } = await import("./SubtitleTools");

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <SubtitleTools />
    </QueryClientProvider>,
  );
}

describe("Subtitles in the player (Phase 16)", () => {
  it("changes the style live and keeps it for the profile", async () => {
    mount();
    fireEvent.click(await screen.findByRole("button", { name: "Larger subtitles" }));
    await waitFor(() => expect(commands.setSubtitleSettings).toHaveBeenCalledWith({ ...settings, size: 50 }));
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Subtitle colour" })).getByRole("radio", { name: "Yellow" }));
    await waitFor(() => expect(commands.setSubtitleSettings).toHaveBeenCalledWith({ ...settings, colour: "#FFD400" }));
    fireEvent.click(screen.getByRole("button", { name: "Thicker outline" }));
    await waitFor(() => expect(commands.setSubtitleSettings).toHaveBeenCalledWith({ ...settings, outline: 4 }));
    expect(nextFont("Calibri")).toBe("Segoe UI");
  });

  it("finds subtitles, shows what each is, and loads the one chosen", async () => {
    mount();
    fireEvent.click(await screen.findByRole("button", { name: /Find subtitles/ }));
    const web = await screen.findByRole("button", { name: /Film\.2019\.WEB/ });
    expect(screen.getByRole("button", { name: /Film\.2019\.BluRay · timed for this file/ })).toBeInTheDocument();
    fireEvent.click(web);
    await waitFor(() => expect(commands.subtitleChoose).toHaveBeenCalledWith(expect.objectContaining({ file_id: "2" })));
    expect(await screen.findByText(/Showing the chosen subtitles \(aligned: offset \+1200 ms\)/)).toBeInTheDocument();
  });

  it("says plainly why aligning again cannot be done", async () => {
    mount();
    fireEvent.click(await screen.findByRole("button", { name: "Align again" }));
    expect(await screen.findByText(/this one is part of the film/)).toBeInTheDocument();
  });
});
