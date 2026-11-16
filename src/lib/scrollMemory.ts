import { useEffect, useLayoutEffect, useRef, useState } from "react";

/**
 * Where each page was scrolled to (Phase 16, the author: "going back to an earlier page takes me back to the default
 * of that page"). One scroll area holds every page, so positions are kept by page key and put back when a page comes
 * back. Pages that load their content after they appear (Home's rails) are not tall enough at first, so the position
 * is retried for a moment, and given up the instant the person scrolls themselves.
 */

const positions = new Map<string, number>();

/** Forget every page whose key starts with `prefix`: a tab pressed again goes back to its start. */
export function forgetScroll(prefix: string): void {
  for (const key of [...positions.keys()]) if (key.startsWith(prefix)) positions.delete(key);
}

/** What is remembered for a page (for tests). */
export function rememberedScroll(key: string): number | undefined {
  return positions.get(key);
}

const RETRY_MS = 1500;

/**
 * Keep a scroll area's position per `key`; returns the ref to give the scroll area. `seq` changes on every move
 * between pages, so pressing the tab already showing (same key, position forgotten) still goes back to the top.
 *
 * A callback ref, not a ref object: the scroll area is a new element after the full-screen player (it is removed
 * while the film plays), and a listener bound once stayed on the old one, so nothing was recorded after a film
 * had played (found by the pad audit in real layout). A new element also gets its page's position back.
 */
export function useScrollMemory(key: string, seq: number): (el: HTMLElement | null) => void {
  const [el, setEl] = useState<HTMLElement | null>(null);
  const current = useRef(key);
  current.current = key;

  // Recorded as it happens: by the time a new page renders, the old one's height is gone and its scroll
  // position with it.
  useEffect(() => {
    if (!el) return;
    const onScroll = () => positions.set(current.current, el.scrollTop);
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [el]);

  useLayoutEffect(() => {
    if (!el) return;
    const target = positions.get(key) ?? 0;
    el.scrollTop = target;
    if (target === 0 || Math.abs(el.scrollTop - target) < 2) return;
    // Not tall enough yet: try again as the page fills in, until the person takes over.
    let frame = 0;
    const until = performance.now() + RETRY_MS;
    const stop = () => cancelAnimationFrame(frame);
    const step = () => {
      el.scrollTop = target;
      if (Math.abs(el.scrollTop - target) < 2 || performance.now() > until) return;
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    el.addEventListener("wheel", stop, { once: true, passive: true });
    el.addEventListener("keydown", stop, { once: true });
    el.addEventListener("pointerdown", stop, { once: true });
    return () => {
      stop();
      el.removeEventListener("wheel", stop);
      el.removeEventListener("keydown", stop);
      el.removeEventListener("pointerdown", stop);
    };
  }, [el, key, seq]);

  return setEl;
}
