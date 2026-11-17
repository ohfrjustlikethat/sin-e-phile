import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Slider } from "@/design-system";
import type { CropView, PicturePreview } from "@/lib/ipc";

/**
 * Adjusting a profile picture (Phase 16, the author: "automatically cropped… then adjustable (drag and zoom)"). The
 * picture fills a rounded-square frame, starting from the square the app chose; dragging moves it, the slider, the
 * mouse wheel and + / − zoom it, and the arrow keys (or the D-pad on the frame) nudge it. It always covers the frame.
 */

export const FRAME = 280;

/** Where the picture sits in the frame: its zoom over "just covers", and its top-left in frame pixels. */
export interface View {
  zoom: number;
  left: number;
  top: number;
}

const cover = (w: number, h: number) => FRAME / Math.min(w, h);

/** Kept covering the frame: never a gap at an edge. */
export function clamp(v: View, w: number, h: number): View {
  const zoom = Math.min(4, Math.max(1, v.zoom));
  const s = cover(w, h) * zoom;
  return {
    zoom,
    left: Math.min(0, Math.max(FRAME - w * s, v.left)),
    top: Math.min(0, Math.max(FRAME - h * s, v.top)),
  };
}

/** The square the frame shows, in fractions of the picture (what the backend cuts). */
export function cropOf(v: View, w: number, h: number): CropView {
  const s = cover(w, h) * v.zoom;
  return { x: -v.left / (w * s), y: -v.top / (h * s), side: FRAME / (w * s) };
}

/** The view that shows `crop` (the app's automatic square, to start from). */
export function viewOf(crop: CropView, w: number, h: number): View {
  // Specta types a float as possibly null; a missing part of the crop is the whole picture.
  const [x, y, side] = [crop.x ?? 0, crop.y ?? 0, crop.side ?? 1];
  const zoom = Math.min(w, h) / (side * w);
  const s = cover(w, h) * zoom;
  return clamp({ zoom, left: -x * w * s, top: -y * h * s }, w, h);
}

export function PictureEditor({
  preview,
  busy,
  onSave,
  onCancel,
}: {
  preview: PicturePreview;
  busy: boolean;
  onSave: (crop: CropView) => void;
  onCancel: () => void;
}) {
  const { width: w, height: h } = preview;
  const [view, setView] = useState<View>(() => viewOf(preview.crop, w, h));
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const frame = useRef<HTMLDivElement>(null);
  const move = (next: View) => setView(clamp(next, w, h));
  // Zoom about the frame's centre, so the face in the middle stays in the middle.
  const zoomTo = (zoom: number) => {
    const z = Math.min(4, Math.max(1, zoom));
    const k = z / view.zoom;
    const c = FRAME / 2;
    move({ zoom: z, left: c - (c - view.left) * k, top: c - (c - view.top) * k });
  };
  useEffect(() => frame.current?.focus(), []);
  const s = cover(w, h) * view.zoom;

  return (
    <Dialog
      open
      onClose={onCancel}
      title="Your picture"
      footer={
        <>
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => onSave(cropOf(view, w, h))} disabled={busy}>
            {busy ? "Saving…" : "Use this"}
          </Button>
        </>
      }
    >
      <p className="mb-4 text-[13px] text-ink-muted">Drag to move it; zoom with the slider, the wheel, or + and −.</p>
      <div
        ref={frame}
        tabIndex={0}
        role="img"
        aria-label="The picture, as it will show. Arrow keys move it."
        className="relative mx-auto cursor-grab touch-none overflow-hidden rounded-[22%] border border-line-strong bg-void"
        style={{ width: FRAME, height: FRAME }}
        onPointerDown={(e) => {
          (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
          drag.current = { x: e.clientX, y: e.clientY, left: view.left, top: view.top };
        }}
        onPointerMove={(e) => {
          const d = drag.current;
          if (d) move({ ...view, left: d.left + e.clientX - d.x, top: d.top + e.clientY - d.y });
        }}
        onPointerUp={() => (drag.current = null)}
        onWheel={(e) => zoomTo(view.zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1))}
        onKeyDown={(e) => {
          const step = 12;
          const by: Record<string, [number, number]> = {
            ArrowLeft: [step, 0],
            ArrowRight: [-step, 0],
            ArrowUp: [0, step],
            ArrowDown: [0, -step],
          };
          if (by[e.key]) {
            e.preventDefault();
            e.stopPropagation();
            move({ ...view, left: view.left + by[e.key]![0], top: view.top + by[e.key]![1] });
          } else if (e.key === "+" || e.key === "=") zoomTo(view.zoom * 1.1);
          else if (e.key === "-") zoomTo(view.zoom / 1.1);
        }}
      >
        <img
          src={preview.url}
          alt=""
          draggable={false}
          className="pointer-events-none absolute max-w-none select-none"
          style={{ left: view.left, top: view.top, width: w * s, height: h * s }}
        />
      </div>
      <div className="mx-auto mt-4" style={{ width: FRAME }}>
        <Slider
          label="Zoom"
          min={1}
          max={4}
          step={0.05}
          value={view.zoom}
          onChange={(z) => zoomTo(z)}
          display={`${Math.round(view.zoom * 100)}%`}
        />
      </div>
    </Dialog>
  );
}
