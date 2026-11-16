import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

type Status = { ready: boolean; building: string | null; percent: number };
const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
let next: () => Promise<unknown> = () => ok<Status>({ ready: true, building: null, percent: 100 });
const commands = {
  catalogueStatus: vi.fn(() =>
    next().then((s) => (s && typeof s === "object" && "status" in s ? s : { status: "ok", data: s })),
  ),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { BuildStrip, GettingReady, useCatalogueBuild } = await import("./CatalogueBuild");

function Probe() {
  const build = useCatalogueBuild();
  return build.ready ? <p>Home</p> : <GettingReady build={build} />;
}

function show(ui: React.ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

describe("The first run's catalogue (Phase 16)", () => {
  it("keeps Home back while it builds, saying what it is doing and how far", async () => {
    next = () => ok<Status>({ ready: false, building: "Building search", percent: 72 });
    show(<Probe />);
    expect(await screen.findByText("Getting ready… 72%")).toBeInTheDocument();
    expect(screen.getByText(/Building search\./)).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Catalogue" })).toHaveAttribute("aria-valuenow", "72");
    expect(screen.queryByText("Home")).toBeNull();
  });

  it("opens Home once it is ready", async () => {
    next = () => ok<Status>({ ready: true, building: null, percent: 100 });
    show(<Probe />);
    expect(await screen.findByText("Home")).toBeInTheDocument();
  });

  it("never keeps anyone out when the status cannot be read", async () => {
    next = () => Promise.resolve({ status: "error", error: "the library is still opening" });
    show(<Probe />);
    await waitFor(() => expect(commands.catalogueStatus).toHaveBeenCalled());
    expect(await screen.findByText("Home")).toBeInTheDocument();
  });

  it("shows the build in onboarding's footer, and says when search works", () => {
    const { rerender } = show(<BuildStrip build={{ ready: false, building: "Adding alternate titles", percent: 41 }} />);
    expect(screen.getByText("Getting the catalogue ready · Adding alternate titles · 41%")).toBeInTheDocument();
    rerender(<BuildStrip build={{ ready: true, building: null, percent: 100 }} />);
    expect(screen.getByText("Catalogue ready · search works")).toBeInTheDocument();
  });
});
