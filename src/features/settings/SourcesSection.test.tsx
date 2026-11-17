import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AddonPreview, SourcesView } from "@/lib/ipc";

/** SPEC.md Phase 6: paste a URL, see what it declares, confirm, enable — no raw JSON. */

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });

const VIEW: SourcesView = {
  sources: [
    { id: "local", name: "Your files", blurb: "Films already on this computer.", addon_host: null,
      enabled: true, health: "Not asked yet.", last_error: null, version: null, serves: null, protocol: null,
      configurable: false },
    { id: "internet-archive", name: "Internet Archive", blurb: "Public-domain feature films.",
      addon_host: null, enabled: true, health: "Answered 3 of 4 searches, usually in 340 ms.",
      last_error: "timed out", version: null, serves: null, protocol: null, configurable: false },
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
  addonConfigure: vi.fn(() => ok(null)),
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

  describe("add-ons as cards, and every change said (Phase 16)", () => {
    const STREMIO = {
      id: "addon:org.example.films", name: "Example Films", blurb: "Films from an example source.",
      addon_host: "addon.example", enabled: true, health: "Answered 2 of 2 searches.", last_error: null,
      version: "2.0.0", serves: "Films and series", protocol: "Stremio add-on", configurable: true,
    };

    it("shows an add-on's card: its version, what it serves, how it speaks, and its own settings", async () => {
      const { useToasts } = await import("@/lib/toast");
      commands.sourcesOverview.mockImplementation(() => ok({ ...VIEW, sources: [...VIEW.sources, STREMIO] }));
      renderIt();
      const card = await screen.findByRole("listitem", { name: "Example Films" });
      expect(card).toHaveTextContent("v2.0.0");
      expect(card).toHaveTextContent("Stremio add-on · Films and series · asks addon.example");
      fireEvent.click(screen.getByRole("button", { name: "Configure Example Films" }));
      await waitFor(() => expect(commands.addonConfigure).toHaveBeenCalledWith("addon:org.example.films"));
      await waitFor(() =>
        expect(useToasts.getState().toasts.at(-1)?.text).toBe("Example Films's own settings page is open in your browser."),
      );
      fireEvent.click(screen.getByRole("button", { name: "Remove Example Films" }));
      await waitFor(() => expect(useToasts.getState().toasts.at(-1)?.text).toBe("Example Films was removed."));
      commands.sourcesOverview.mockImplementation(() => ok(VIEW));
    });

    it("says an add-on was added, on its card and in a toast", async () => {
      const { useToasts } = await import("@/lib/toast");
      renderIt();
      fireEvent.change(await screen.findByLabelText("Source address"), { target: { value: PREVIEW.pasted } });
      fireEvent.click(screen.getByRole("button", { name: "Look at it" }));
      commands.sourcesOverview.mockImplementation(() => ok({ ...VIEW, sources: [...VIEW.sources, STREMIO] }));
      fireEvent.click(await screen.findByRole("button", { name: "Add source" }));
      const card = await screen.findByRole("listitem", { name: "Example Films" });
      await waitFor(() => expect(card).toHaveTextContent("Added ✓"));
      expect(useToasts.getState().toasts.at(-1)?.text).toMatch(/^Add-on added: Example Films\./);
      commands.sourcesOverview.mockImplementation(() => ok(VIEW));
    });
  });
});
