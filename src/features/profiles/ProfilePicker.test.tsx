import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUi } from "@/lib/store";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const PROFILES = [
  { id: 1, name: "Jon", is_default: true, avatar_colour: "slate", is_kids: false, has_pin: true, active: true },
  { id: 2, name: "Mina", is_default: false, avatar_colour: "ochre", is_kids: true, has_pin: false, active: false },
];
const commands = {
  profiles: vi.fn(() => ok(PROFILES)),
  switchProfile: vi.fn((id: number, pin: string | null) =>
    id === 1 && pin !== "2468"
      ? Promise.resolve({ status: "error" as const, error: "that PIN is not right" })
      : ok(PROFILES.find((p) => p.id === id)),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
const { ProfilePicker } = await import("./ProfilePicker");
const { initialOf, colourOf } = await import("./Avatar");

function renderIt(onDone = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <ProfilePicker onDone={onDone} />
    </QueryClientProvider>,
  );
  return onDone;
}

describe("Who's watching? (Phase 14)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUi.setState({ settingsOpen: false });
  });

  it("names each profile with its initial, and opens one without a PIN at once", async () => {
    const done = renderIt();
    const mina = await screen.findByRole("button", { name: "Mina, kids" });
    expect(mina).toHaveTextContent("M");
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Jon, PIN" })));
    fireEvent.click(mina);
    await waitFor(() => expect(commands.switchProfile).toHaveBeenCalledWith(2, null));
    expect(done).toHaveBeenCalled();
  });

  it("asks for a PIN, says when it is wrong, and opens with the right one", async () => {
    const done = renderIt();
    fireEvent.click(await screen.findByRole("button", { name: "Jon, PIN" }));
    expect(await screen.findByText("Jon's PIN")).toBeInTheDocument();
    for (const d of "1111") fireEvent.click(screen.getByRole("button", { name: d }));
    expect(await screen.findByText("that PIN is not right")).toBeInTheDocument();
    expect(done).not.toHaveBeenCalled();
    // The keyboard's digits work too, and Delete takes one back.
    for (const k of ["2", "4", "9", "Backspace", "6", "8"]) {
      act(() => void window.dispatchEvent(new KeyboardEvent("keydown", { key: k })));
    }
    await waitFor(() => expect(commands.switchProfile).toHaveBeenLastCalledWith(1, "2468"));
    await waitFor(() => expect(done).toHaveBeenCalled());
  });

  it("Escape keeps whoever is watching; Manage profiles opens Settings", async () => {
    const done = renderIt();
    await screen.findByRole("button", { name: "Mina, kids" });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(done).toHaveBeenCalledTimes(1);
    expect(commands.switchProfile).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Manage profiles" }));
    expect(useUi.getState().settingsOpen).toBe(true);
  });

  it("an avatar is an initial on a known colour", () => {
    expect(initialOf("  mina")).toBe("M");
    expect(initialOf("")).toBe("?");
    expect(colourOf("ochre")).toBe("ochre");
    expect(colourOf(null)).toBe("slate");
    expect(colourOf("#ff00ff")).toBe("slate");
  });
});
