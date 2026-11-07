import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TorrentSettingsView } from "@/lib/ipc";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });

const VIEW: TorrentSettingsView = {
  download_limit_kib: null,
  upload_limit_kib: null,
  peer_limit: 128,
  keep_seeding: false,
  bind_interface: null,
  adapters: [
    { name: "VPN", connected: true },
    { name: "Ethernet", connected: false },
  ],
  engine_running: true,
};

const commands = {
  torrentSettings: vi.fn(() => ok(VIEW)),
  setTorrentSettings: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { NetworkSection } = await import("./NetworkSection");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <NetworkSection />
    </QueryClientProvider>,
  );
}

describe("NetworkSection", () => {
  beforeEach(() => vi.clearAllMocks());

  it("defaults to stopping sharing at completion, and says so", async () => {
    renderIt();
    const toggle = await screen.findByRole("switch", { name: "Stop sharing when a download finishes" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("saves a VPN binding and empty limits as no limit", async () => {
    renderIt();
    fireEvent.change(await screen.findByLabelText("Use only this network adapter"), {
      target: { value: "VPN" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(commands.setTorrentSettings).toHaveBeenCalledWith(null, null, 128, false, "VPN"),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("The limits apply now");
  });

  it("marks an adapter that is not connected", async () => {
    renderIt();
    expect(await screen.findByRole("option", { name: "Ethernet (not connected)" })).toBeInTheDocument();
  });
});
