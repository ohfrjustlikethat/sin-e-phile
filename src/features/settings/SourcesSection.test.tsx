import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AddonPreview, SourcesView } from "@/lib/ipc";

/** SPEC.md Phase 6: paste a URL, see what it declares, confirm, enable — no raw JSON. */

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });

const VIEW: SourcesView = {
  sources: [
    { id: "local", name: "Your files", blurb: "Films already on this computer.", addon_host: null,
      enabled: true, health: "Not asked yet.", last_error: null },
    { id: "internet-archive", name: "Internet Archive", blurb: "Public-domain feature films.",
      addon_host: null, enabled: true, health: "Answered 3 of 4 searches, usually in 340 ms.",
      last_error: "timed out" },
  ],
  refused_addons: [],
};

const PREVIEW: AddonPreview = {
  pasted: "https://addon.example/manifest.json",
  backend_id: "addon:com.example.films",
  name: "Example Films",
  version: "1.0.0",
  description: "Public-domain features.",
  finds: "Finds films.",
  delivers: ["Streams over the web."],
  host: "addon.example",
  homepage: null,
  replaces: false,
};

const commands = {
  sourcesOverview: vi.fn(() => ok(VIEW)),
  sourcesSetEnabled: vi.fn(() => ok(null)),
  sourcesReorder: vi.fn(() => ok(null)),
  addonPreview: vi.fn(() => ok(PREVIEW)),
  addonInstall: vi.fn(() => ok("addon:com.example.films")),
  addonRemove: vi.fn(() => ok(null)),
  catalogueOpen: vi.fn(),
  linksList: vi.fn(() => ok([])),
  linksImport: vi.fn(),
  linksAdd: vi.fn(),
  linksRemove: vi.fn(),
};
vi.mock("@/lib/ipc", () => ({ commands }));

const { SourcesSection } = await import("./SourcesSection");

function renderIt() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <SourcesSection />
    </QueryClientProvider>,
  );
}

describe("SourcesSection", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists sources in order, each with a plain health line", async () => {
    renderIt();
    expect(await screen.findByText("Your files")).toBeInTheDocument();
    expect(screen.getByText(/Answered 3 of 4 searches/)).toBeInTheDocument();
    expect(screen.getByText(/Last problem: timed out/)).toBeInTheDocument();
  });

  it("asks a source earlier by swapping it with the one above", async () => {
    renderIt();
    fireEvent.click(await screen.findByRole("button", { name: "Ask Internet Archive earlier" }));
    await waitFor(() =>
      expect(commands.sourcesReorder).toHaveBeenCalledWith(["internet-archive", "local"]),
    );
    expect(screen.getByRole("button", { name: "Ask Your files earlier" })).toBeDisabled();
  });

  it("shows what a pasted source declares, in sentences, before anything is stored", async () => {
    renderIt();
    fireEvent.change(screen.getByLabelText("Source address"), {
      target: { value: PREVIEW.pasted },
    });
    fireEvent.click(screen.getByRole("button", { name: "Look at it" }));

    const region = await screen.findByRole("region", { name: "About Example Films" });
    expect(region).toHaveTextContent("Finds films.");
    expect(region).toHaveTextContent("Streams over the web.");
    expect(region).toHaveTextContent("Your searches are sent to addon.example.");
    // No raw JSON in the primary path.
    expect(region.textContent).not.toMatch(/[{}"]/);
    expect(commands.addonInstall).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Add source" }));
    await waitFor(() => expect(commands.addonInstall).toHaveBeenCalledWith(PREVIEW.pasted));
    expect(await screen.findByText("Example Films was added and is on.")).toBeInTheDocument();
  });

  it("puts no address in any field", async () => {
    // §2.1: no default source or catalogue URL ships, so none may be pre-filled.
    renderIt();
    await screen.findByText("Your files");
    for (const input of screen.getAllByRole("textbox")) {
      expect((input as HTMLInputElement).value).toBe("");
    }
  });
});
