import { useEffect, useLayoutEffect, useRef, useState } from "react";

/**
 * Where each page was scrolled to (Phase 16, the author: "going back to an earlier page takes me back to the default
 * of that page"). One scroll area holds every page, so positions are kept by page key and put back when a page comes
 * back. Pages that load their content after they appear (Home's rails) are not tall enough at first, so the position
 * is retried for a moment, and given up the instant the person scrolls themselves.
 */

const positions = new Map<string, number>();
/**
 * Which titled thing had focus on each page (Phase 16, the author: "after closing something… focus is not where it
 * was"): the `data-focus-key` of the closest element that carries one (a title card, a search result). Coming back
 * to the page, focus goes back to it, so B from a title page lands on the card that opened it.
 */
const focused = new Map<string, string>();

/** Forget every page whose key starts with `prefix`: a tab pressed again goes back to its start. */
export function forgetScroll(prefix: string): void {
  for (const key of [...positions.keys()]) if (key.startsWith(prefix)) positions.delete(key);
  for (const key of [...focused.keys()]) if (key.startsWith(prefix)) focused.delete(key);
}

/** Focus the element marked `focusKey` inside `root` (its own focusable, or the first one in it). */
function refocus(root: HTMLElement, focusKey: string): boolean {
  const marked = root.querySelector<HTMLElement>(`[data-focus-key="${CSS.escape(focusKey)}"]`);
  if (!marked) return false;
  const target = marked.matches("button, a, input, [tabindex]")
    ? marked
    : marked.querySelector<HTMLElement>("button, a, input, [tabindex]");
  if (!target) return false;
  target.focus({ preventScroll: true });
  return document.activeElement === target;
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
    const onFocus = (e: FocusEvent) => {
      const marked = (e.target as HTMLElement | null)?.closest<HTMLElement>("[data-focus-key]");
      if (marked?.dataset["focusKey"]) focused.set(current.current, marked.dataset["focusKey"]);
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    el.addEventListener("focusin", onFocus);
    return () => {
      el.removeEventListener("scroll", onScroll);
      el.removeEventListener("focusin", onFocus);
    };
  }, [el]);

  // Focus back where it was, when nothing else has taken it: tried as the page fills in (a rail's cards arrive
  // after the page), for the same short while as the scroll position.
  useEffect(() => {
    if (!el) return;
    const want = focused.get(key);
    if (!want) return;
    const free = () => {
      const at = document.activeElement;
      return !at || at === document.body || !at.isConnected || !el.contains(at) || at === el;
    };
    if (!free()) return;
    let frame = 0;
    const until = performance.now() + RETRY_MS;
    const step = () => {
      if (!free() || refocus(el, want) || performance.now() > until) return;
      frame = requestAnimationFrame(step);
    };
    step();
    return () => cancelAnimationFrame(frame);
  }, [el, key, seq]);

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
