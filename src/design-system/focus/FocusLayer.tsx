import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { create } from "zustand";
import { step, type Spring } from "./spring";

/**
 * The TV focus language — ADR-0042, `docs/specs/tv-focus.md`.
 *
 * - `FocusRing`: one white ring that follows the focused element and glides between
 *   targets on a critically damped spring. It is drawn outside `#root`, so the 125% TV
 *   scale inside never distorts it.
 * - `HintsBar`: the button-hints footer while a pad is in use.
 * - `FocusLayer`: both, plus the input-mode bookkeeping that decides when focus is
 *   visible — keyboard or pad, never the mouse.
 *
 * This folder is the only place §9.0 lets shadow, glow, gradient and 3D transforms in.
 */

const root = () => document.documentElement;

/** Keyboard and pad show focus; the mouse hides it. */
export function setFocusVisible(on: boolean): void {
  root().dataset["focusVisible"] = on ? "true" : "false";
}

/** True while the last input came from a pad (`data-input="pad"`, set by `lib/gamepad.ts`). */
export function usePadMode(): boolean {
  return useSyncExternalStore(
    (notify) => {
      const mo = new MutationObserver(notify);
      mo.observe(root(), { attributes: true, attributeFilter: ["data-input"] });
      return () => mo.disconnect();
    },
    () => root().dataset["input"] === "pad",
  );
}

/** A move longer than this is a page change: the ring fades in rather than flying there. */
const FAR = 1.5;
/** The ring sits outside the element: a 2px gap, then the 3px ring. */
const OUTSET = 5;

export function FocusRing() {
  const ring = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ring.current;
    if (!el) return;
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    let x: Spring = { x: 0, v: 0 };
    let y: Spring = { x: 0, v: 0 };
    let w: Spring = { x: 0, v: 0 };
    let h: Spring = { x: 0, v: 0 };
    let shown = false;
    let last = performance.now();
    let raf = 0;

    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const a = document.activeElement as HTMLElement | null;
      const visible =
        root().dataset["focusVisible"] === "true" &&
        !!a &&
        a !== document.body &&
        !a.closest("[data-no-focus-ring]");
      if (!visible) {
        if (shown) el.style.opacity = "0";
        shown = false;
      } else {
        const r = a.getBoundingClientRect();
        const tx = r.left - OUTSET;
        const ty = r.top - OUTSET;
        const tw = r.width + OUTSET * 2;
        const th = r.height + OUTSET * 2;
        const far = Math.hypot(tx - x.x, ty - y.x) > window.innerHeight * FAR;
        if (!shown || far || reduce?.matches) {
          x = { x: tx, v: 0 };
          y = { x: ty, v: 0 };
          w = { x: tw, v: 0 };
          h = { x: th, v: 0 };
        } else {
          x = step(x, tx, dt);
          y = step(y, ty, dt);
          w = step(w, tw, dt);
          h = step(h, th, dt);
        }
        el.style.transform = `translate(${x.x}px, ${y.x}px)`;
        el.style.width = `${w.x}px`;
        el.style.height = `${h.x}px`;
        el.style.opacity = "1";
        shown = true;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  return createPortal(<div ref={ring} className="focus-ring" aria-hidden data-focus-ring-el />, document.body);
}

export type Hint = [button: "A" | "B" | "X" | "Y" | "LB" | "RB" | "View" | "Menu", action: string];

export const DEFAULT_HINTS: Hint[] = [
  ["A", "Select"],
  ["B", "Back"],
  ["Y", "Search"],
  ["Menu", "Settings"],
];

/** The hints a screen offers. The most recently mounted screen wins; unmounting restores. */
export const useHints = create<{ stack: Hint[][]; push: (h: Hint[]) => () => void }>((set) => ({
  stack: [],
  push: (h) => {
    set((s) => ({ stack: [...s.stack, h] }));
    return () => set((s) => ({ stack: s.stack.filter((x) => x !== h) }));
  },
}));

/** Declare this screen's hints while it is mounted. */
export function useScreenHints(hints: Hint[]): void {
  const push = useHints((s) => s.push);
  const [h] = useState(hints);
  useEffect(() => push(h), [push, h]);
}

export function HintsBar() {
  const pad = usePadMode();
  const hints = useHints((s) => s.stack.at(-1) ?? DEFAULT_HINTS);
  if (!pad) return null;
  return createPortal(
    <div className="hints-bar" aria-label="Controller buttons" role="note">
      {hints.map(([button, action]) => (
        <span key={button} className="flex items-center gap-2">
          <span className="hint-glyph" aria-hidden>
            {button === "Menu" ? "≡" : button === "View" ? "⧉" : button}
          </span>
          <span>{action}</span>
        </span>
      ))}
    </div>,
    document.body,
  );
}

export function FocusLayer() {
  useEffect(() => {
    root().dataset["focusRing"] = "";
    // Kept across remounts (browsing -> player): only the first mount decides.
    if (root().dataset["focusVisible"] === undefined) setFocusVisible(false);
    const mouse = () => setFocusVisible(false);
    const key = (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.altKey && !e.metaKey) setFocusVisible(true);
    };
    window.addEventListener("pointerdown", mouse, true);
    window.addEventListener("keydown", key, true);
    return () => {
      delete root().dataset["focusRing"];
      window.removeEventListener("pointerdown", mouse, true);
      window.removeEventListener("keydown", key, true);
    };
  }, []);
  return (
    <>
      <FocusRing />
      <HintsBar />
    </>
  );
}
