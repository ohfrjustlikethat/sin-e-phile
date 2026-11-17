import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { clamp, cropOf, FRAME, viewOf } from "./PictureEditor";

const ok = <T,>(data: T) => Promise.resolve({ status: "ok" as const, data });
const PREVIEW = { url: "data:image/webp;base64,UklGRg==", width: 600, height: 900, crop: { x: 0, y: 0.1, side: 1 } };
const commands = {
  pickPicture: vi.fn(() => ok<string | null>("C:\\Users\\me\\Pictures\\me.heic.png")),
  picturePreview: vi.fn(() => ok(PREVIEW)),
  setProfilePicture: vi.fn((_p: number, _path: string, _crop: unknown) => ok(null)),
  profilePicture: vi.fn(() => ok<string | null>(null)),
  clearProfilePicture: vi.fn(() => ok(null)),
};
vi.mock("@/lib/ipc", () => ({ commands }));
const { ChoosePicture } = await import("./ChoosePicture");

describe("A profile picture: squared, then adjusted (Phase 16)", () => {
  it("shows the app's square first, and gives back exactly the square shown", () => {
    const view = viewOf(PREVIEW.crop, 600, 900);
    const crop = cropOf(view, 600, 900);
    expect(crop.x).toBeCloseTo(0);
    expect(crop.y).toBeCloseTo(0.1);
    expect(crop.side).toBeCloseTo(1);
  });

  it("never leaves a gap: zoomed out or dragged past an edge, the picture still covers the frame", () => {
    const v = clamp({ zoom: 0.2, left: 500, top: -5000 }, 600, 900);
    expect(v.zoom).toBe(1);
    expect(v.left).toBe(0);
    // 900 tall at a cover scale of FRAME / 600: the lowest the top can go.
    expect(v.top).toBeCloseTo(FRAME - 900 * (FRAME / 600));
  });

  it("chooses a file, opens the editor on the app's square, and keeps it", async () => {
    const { useToasts } = await import("@/lib/toast");
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <ChoosePicture profile={3} />
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Choose a picture…" }));
    expect(await screen.findByRole("dialog", { name: "Your picture" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Use this" }));
    await waitFor(() => expect(commands.setProfilePicture).toHaveBeenCalled());
    const [profile, path, crop] = commands.setProfilePicture.mock.calls[0]!;
    expect([profile, path]).toEqual([3, "C:\\Users\\me\\Pictures\\me.heic.png"]);
    expect((crop as { y: number }).y).toBeCloseTo(0.1);
    await waitFor(() => expect(useToasts.getState().toasts.at(-1)?.text).toMatch(/^Picture saved/));
  });
});
