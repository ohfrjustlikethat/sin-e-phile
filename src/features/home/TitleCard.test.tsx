import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
let sources = { posters: false, tmdb: false };
const commands = {
  artworkSettings: vi.fn(() => ok(sources)),
  setShowPosters: vi.fn((on: boolean) => {
    sources = { ...sources, posters: on };
    return ok(null);
  }),
  poster: vi.fn((_id: number) => ok<string | null>("data:image/webp;base64,UklGRg==")),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { TitleCard } = await import("./TitleCard");
const { ArtworkSection } = await import("@/features/settings/ArtworkSection");

function show(ui: React.ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const film = { id: "7", title: "Nosferatu", year: 1922, director: "F. W. Murnau" };

describe("Posters, only when chosen (ADR-0048)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sources = { posters: false, tmdb: false };
  });

  it("asks for nothing and keeps the designed text card when no source is on", async () => {
    const { container } = show(<TitleCard id={7} film={film} onOpen={() => {}} />);
    await waitFor(() => expect(commands.artworkSettings).toHaveBeenCalled());
    expect(commands.poster).not.toHaveBeenCalled();
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("Nosferatu")).toBeInTheDocument();
  });

  it("shows the poster once posters are on", async () => {
    sources = { posters: true, tmdb: false };
    const { container } = show(<TitleCard id={7} film={film} onOpen={() => {}} />);
    await waitFor(() => expect(commands.poster).toHaveBeenCalledWith(7));
    await waitFor(() => expect(container.querySelector("img")?.getAttribute("src")).toMatch(/^data:image\/webp/));
  });

  it("the switch says what it sends, saves, and says it is on", async () => {
    const { useToasts } = await import("@/lib/toast");
    show(<ArtworkSection />);
    const toggle = await screen.findByRole("switch", { name: /Show posters/ });
    expect(screen.getByText(/sees which titles you look at, and nothing else/)).toBeInTheDocument();
    await waitFor(() => expect(toggle).toBeEnabled());
    fireEvent.click(toggle);
    await waitFor(() => expect(commands.setShowPosters).toHaveBeenCalledWith(true));
    await waitFor(() => expect(useToasts.getState().toasts.at(-1)?.text).toMatch(/^Posters on/));
  });
});
