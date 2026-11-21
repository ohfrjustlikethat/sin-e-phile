import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useToasts } from "@/lib/toast";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  dismiss: vi.fn((_id: number, _kind: string, _hour: number | null) => ok(null)),
  undismiss: vi.fn((_id: number) => ok(null)),
  removeFromContinue: vi.fn((_id: number) => ok(null)),
  titleRating: vi.fn((_id: number) => ok<number | null>(null)),
  rateTitle: vi.fn((_id: number, _halves: number | null) => ok(null)),
  artworkSettings: vi.fn(() => ok({ posters: false, tmdb: false })),
  poster: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { MenuCard } = await import("./MenuCard");

function renderCard(extra: { isShow?: boolean; dismissible?: boolean; continuing?: boolean } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MenuCard
        id={42}
        title="Close-Up"
        isShow={extra.isShow ?? false}
        dismissible={extra.dismissible ?? false}
        continuing={extra.continuing ?? false}
        film={{ id: "42", title: "Close-Up", year: 1990, director: "Like Stalker, but from Iran", spine: 1 }}
        onOpen={() => {}}
      />
    </QueryClientProvider>,
  );
}

describe("a card's menu (Phase 18, ADR-0056)", () => {
  it("a recommendation's menu (X on a pad) rates it and offers three answers; one is kept, with an undo", async () => {
    useToasts.setState({ toasts: [] });
    renderCard({ dismissible: true });
    const card = screen.getByRole("button", { name: /^Close-Up/ });
    card.focus();
    fireEvent.keyDown(card, { key: "ContextMenu" });
    const menu = screen.getByRole("dialog", { name: "Close-Up: options" });
    const stars = screen.getByRole("slider", { name: "Your rating" });
    expect(document.activeElement).toBe(stars);
    expect(menu).toContainElement(screen.getByRole("button", { name: "Not for me" }));
    expect(menu).toContainElement(screen.getByRole("button", { name: "Already seen" }));
    expect(screen.queryByRole("button", { name: "Remove from Continue watching" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Not right now" }));
    await waitFor(() => expect(commands.dismiss).toHaveBeenCalledWith(42, "not_right_now", expect.any(Number)));
    const undo = await screen.findByRole("button", { name: "Undo" });
    expect(screen.getByText("Not right now")).toBeInTheDocument();
    expect(document.activeElement).toBe(undo);
    expect(useToasts.getState().toasts.at(-1)?.text).toBe("Close-Up: it's off Home for two weeks.");

    fireEvent.click(undo);
    await waitFor(() => expect(commands.undismiss).toHaveBeenCalledWith(42));
    expect(await screen.findByRole("button", { name: /^Close-Up/ })).toBeInTheDocument();
    expect(useToasts.getState().toasts.at(-1)?.text).toBe("Close-Up is back.");
  });

  it("any film's menu rates it in half stars, saved once the stars settle", async () => {
    vi.useFakeTimers();
    renderCard();
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Close-Up/ }));
    expect(screen.queryByRole("button", { name: "Not for me" })).toBeNull();
    const stars = screen.getByRole("slider", { name: "Your rating" });
    fireEvent.change(stars, { target: { value: "8" } });
    fireEvent.change(stars, { target: { value: "9" } });
    await act(async () => {
      vi.advanceTimersByTime(400);
    });
    vi.useRealTimers();
    expect(commands.rateTitle).toHaveBeenCalledTimes(1);
    expect(commands.rateTitle).toHaveBeenCalledWith(42, 9);
  });

  it("a show's menu sends its seasons to its page", () => {
    useUi.setState({ detail: null });
    renderCard({ isShow: true });
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Close-Up/ }));
    expect(screen.queryByRole("slider")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Rate seasons…" }));
    expect(useUi.getState().detail).toBe(42);
  });

  it("in Continue watching, the menu and the × remove it from the row", async () => {
    useToasts.setState({ toasts: [] });
    renderCard({ continuing: true });
    fireEvent.click(screen.getByRole("button", { name: "Remove Close-Up from Continue watching" }));
    await waitFor(() => expect(commands.removeFromContinue).toHaveBeenCalledWith(42));
    expect(useToasts.getState().toasts.at(-1)?.text).toMatch(/removed from Continue watching/);
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Close-Up/ }));
    fireEvent.click(screen.getByRole("button", { name: "Remove from Continue watching" }));
    await waitFor(() => expect(commands.removeFromContinue).toHaveBeenCalledTimes(2));
  });

  it("B on a pad (Escape at the window) closes the menu and is not also Back", () => {
    renderCard({ dismissible: true });
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Close-Up/ }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    const back = new KeyboardEvent("keydown", { key: "Escape", cancelable: true });
    act(() => {
      window.dispatchEvent(back);
    });
    expect(back.defaultPrevented).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
