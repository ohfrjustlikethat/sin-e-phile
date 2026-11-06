import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const SETTINGS = { languages: ["en", "fr"], always: false, font: "Segoe UI", size: 46, position: 95, background: false };
const commands = {
  subtitleSettings: vi.fn(() =>
    ok({ settings: SETTINGS, has_key: true, used_today: 2, budget: 5, server_remaining: null as number | null }),
  ),
  setSubtitleSettings: vi.fn(() => ok(null)),
  setOpensubtitlesKey: vi.fn(() => ok(null)),
  clearOpensubtitlesKey: vi.fn(() => ok(true)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { SubtitlesSection, parseLanguages } = await import("./SubtitlesSection");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <SubtitlesSection />
    </QueryClientProvider>,
  );
}

describe("Settings → Subtitles (SPEC.md Phase 10)", () => {
  beforeEach(() => vi.clearAllMocks());

  it("reads languages as codes or names, in order, once each", () => {
    expect(parseLanguages("en, French de")).toEqual(["en", "fr", "de"]);
    expect(parseLanguages("Japanese, ja, ")).toEqual(["ja"]);
    expect(parseLanguages("")).toEqual([]);
  });

  it("shows how much of today's quota is left", async () => {
    renderIt();
    expect(await screen.findByText(/3 of 5 downloads left today/)).toBeInTheDocument();
  });

  it("believes OpenSubtitles when it says fewer are left", async () => {
    commands.subtitleSettings.mockImplementationOnce(() =>
      ok({ settings: SETTINGS, has_key: true, used_today: 2, budget: 5, server_remaining: 1 }),
    );
    renderIt();
    expect(await screen.findByText(/1 of 5 downloads left today/)).toBeInTheDocument();
  });

  it("saves the languages as codes", async () => {
    renderIt();
    const field = await screen.findByLabelText("Languages you read, in order");
    fireEvent.change(field, { target: { value: "Hindi, en" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(commands.setSubtitleSettings).toHaveBeenCalledWith({ ...SETTINGS, languages: ["hi", "en"] }),
    );
  });
});
