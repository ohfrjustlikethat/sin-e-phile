import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useToasts } from "@/lib/toast";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const commands = {
  dismiss: vi.fn((_id: number, _kind: string, _hour: number | null) => ok(null)),
  undismiss: vi.fn((_id: number) => ok(null)),
  artworkSettings: vi.fn(() => ok({ posters: false, tmdb: false })),
  poster: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { RecommendedCard } = await import("./RecommendedCard");

function renderCard() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <RecommendedCard
        id={42}
        title="Close-Up"
        film={{ id: "42", title: "Close-Up", year: 1990, director: "Like Stalker, but from Iran", spine: 1 }}
        onOpen={() => {}}
      />
    </QueryClientProvider>,
  );
}

describe("a recommended card (Phase 18)", () => {
  it("the context-menu key (X on a pad) offers three answers; one is kept, with an undo", async () => {
    useToasts.setState({ toasts: [] });
    renderCard();
    const card = screen.getByRole("button", { name: /Close-Up/ });
    card.focus();
    fireEvent.keyDown(card, { key: "ContextMenu" });
    const menu = screen.getByRole("dialog", { name: "Close-Up: not interested?" });
    const first = screen.getByRole("button", { name: "Not for me" });
    expect(document.activeElement).toBe(first);
    expect(menu).toContainElement(screen.getByRole("button", { name: "Not right now" }));
    expect(menu).toContainElement(screen.getByRole("button", { name: "Already seen" }));

    fireEvent.click(screen.getByRole("button", { name: "Not right now" }));
    await waitFor(() => expect(commands.dismiss).toHaveBeenCalledWith(42, "not_right_now", expect.any(Number)));
    const undo = await screen.findByRole("button", { name: "Undo" });
    expect(screen.getByText("Not right now")).toBeInTheDocument();
    expect(document.activeElement).toBe(undo);
    expect(useToasts.getState().toasts.at(-1)?.text).toBe("Close-Up: it's off Home for two weeks.");

    fireEvent.click(undo);
    await waitFor(() => expect(commands.undismiss).toHaveBeenCalledWith(42));
    expect(await screen.findByRole("button", { name: /Close-Up/ })).toBeInTheDocument();
    expect(useToasts.getState().toasts.at(-1)?.text).toBe("Close-Up is back.");
  });

  it("B on a pad (Escape at the window) closes the menu and is not also Back", () => {
    renderCard();
    fireEvent.contextMenu(screen.getByRole("button", { name: /Close-Up/ }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    const back = new KeyboardEvent("keydown", { key: "Escape", cancelable: true });
    act(() => {
      window.dispatchEvent(back);
    });
    expect(back.defaultPrevented).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
