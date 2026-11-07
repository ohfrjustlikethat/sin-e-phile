import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const JON = { id: 1, name: "Jon", is_default: true, avatar_colour: "slate", is_kids: false, has_pin: false, active: true };
const MINA = { id: 2, name: "Mina", is_default: false, avatar_colour: "ochre", is_kids: true, has_pin: false, active: false };
let list = [JON, MINA];
const commands = {
  profiles: vi.fn(() => ok(list)),
  createProfile: vi.fn(() => ok({ ...MINA, id: 3, name: "Ada" })),
  updateProfile: vi.fn(() => ok(null)),
  deleteProfile: vi.fn(() => ok(true)),
  setProfilePin: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
const { ProfilesSection } = await import("./ProfilesSection");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <ProfilesSection />
    </QueryClientProvider>,
  );
}

describe("Settings → Profiles (Phase 14)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    list = [JON, MINA];
  });

  it("adds a profile with a colour, kids, and a PIN", async () => {
    renderIt();
    fireEvent.click(await screen.findByRole("button", { name: "Add a profile" }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Ada" } });
    fireEvent.click(screen.getByRole("radio", { name: "plum" }));
    fireEvent.click(screen.getByRole("switch", { name: /Kids profile/ }));
    fireEvent.change(screen.getByLabelText("PIN (optional, four digits)"), { target: { value: "12a34" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => expect(commands.createProfile).toHaveBeenCalledWith("Ada", "plum", true, "1234"));
  });

  it("asks before deleting, and says what goes with it", async () => {
    renderIt();
    const row = (await screen.findByText("Mina")).closest("li") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Delete Mina" }));
    expect(within(row).getByText("Delete Mina, with its history and watchlist?")).toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: "Keep" }));
    expect(commands.deleteProfile).not.toHaveBeenCalled();
    fireEvent.click(within(row).getByRole("button", { name: "Delete Mina" }));
    fireEvent.click(within(row).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(commands.deleteProfile).toHaveBeenCalledWith(2));
  });

  it("a kids profile sees the list but cannot change it", async () => {
    list = [{ ...JON, active: false }, { ...MINA, active: true }];
    renderIt();
    await screen.findByText("Jon");
    expect(screen.queryByRole("button", { name: "Add a profile" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Delete/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Edit/ })).toBeNull();
  });

  it("the last profile cannot be deleted", async () => {
    list = [JON];
    renderIt();
    await screen.findByText("Jon");
    expect(screen.queryByRole("button", { name: "Delete Jon" })).toBeNull();
  });
});
