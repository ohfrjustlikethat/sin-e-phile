import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

/**
 * Rail — a horizontally scrolling row, virtualised.
 *
 * WHY VIRTUALISE. Phase 2's exit criterion is 60fps with 500 poster cards. Each
 * card is an image plus several text nodes, so 500 of them is thousands of DOM
 * nodes and 500 decoded images — enough to drop frames on Tier 0, which is the
 * hardware §2.3 is enforced against. Only the visible window plus a small
 * overscan is mounted.
 *
 * The virtualisation is deliberately simple: items are a KNOWN width, so the
 * visible range is arithmetic rather than measurement. A general
 * variable-height virtualiser would be far more code and this project does not
 * need one — §2.2 says prefer the clear implementation over the clever one.
 *
 * §9.4 requires cards NOT to be uniform, which sits awkwardly with fixed-width
 * virtualisation. Resolved by `leadWidth`: the first card may be wider, and the
 * arithmetic accounts for exactly that one exception rather than becoming general.
 *
 * KEYBOARD: A ROVING TABINDEX, NOT 500 TAB STOPS.
 * Virtualisation breaks plain Tab navigation, and it does so silently. Only the
 * mounted window exists in the DOM, so Tab walks the ~13 cards that happen to be
 * mounted and then leaves the rail entirely — the other 487 are unreachable, and
 * nothing about the page looks wrong. Measured, not assumed: a scripted Tab walk
 * cycled the same handful of cards and never advanced the rail.
 *
 * So the rail is ONE tab stop. Arrow keys move a "roving" focus through all 500,
 * scrolling the rail as they go, which mounts each card just before it is focused.
 * Home/End jump to the ends. This is the standard pattern for list-like widgets,
 * and it is better for keyboard users than 500 stops would have been even if
 * virtualisation had not forced the issue.
 */

interface Props<T> {
  label: string;
  /** Honest reason this rail exists, shown right-aligned. Phase 17 fills these. */
  why?: string;
  items: T[];
  /** Stable key per item. */
  keyOf: (item: T, index: number) => string;
  render: (item: T, index: number) => ReactNode;
  /** Width of a standard item in px, INCLUDING its right gap. */
  itemWidth?: number;
  /** Width of the first item, if it leads larger. Defaults to `itemWidth`. */
  leadWidth?: number;
  /** Items rendered beyond each edge, to cover fast scrolling. */
  overscan?: number;
  onSeeAll?: () => void;
}

export function Rail<T>({
  label,
  why,
  items,
  keyOf,
  render,
  itemWidth = 194,
  leadWidth,
  overscan = 3,
  onSeeAll,
}: Props<T>) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const [scrollLeft, setScrollLeft] = useState(0);
  const [viewportWidth, setViewportWidth] = useState(0);
  /** The one card in the rail that is reachable with Tab. */
  const [activeIndex, setActiveIndex] = useState(0);
  /** Set when a key moved `activeIndex`, so focus follows once the card mounts. */
  const wantsFocus = useRef(false);

  const lead = leadWidth ?? itemWidth;

  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      if (entry) setViewportWidth(entry.contentRect.width);
    });
    ro.observe(el);
    setViewportWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  // rAF-throttled: a scroll handler that sets state on every event will itself
  // cause the dropped frames this component exists to avoid.
  const ticking = useRef(false);
  const onScroll = useCallback(() => {
    if (ticking.current) return;
    ticking.current = true;
    requestAnimationFrame(() => {
      setScrollLeft(viewportRef.current?.scrollLeft ?? 0);
      ticking.current = false;
    });
  }, []);

  /** Left offset of item `i`, accounting for the wider lead card. */
  const offsetOf = (i: number) => (i === 0 ? 0 : lead + (i - 1) * itemWidth);

  /** Width of item `i` — only the lead card differs. */
  const widthOf = (i: number) => (i === 0 ? lead : itemWidth);

  /** First focusable element inside the mounted wrapper for item `i`, if mounted. */
  const cardAt = (i: number): HTMLElement | null =>
    trackRef.current?.querySelector<HTMLElement>(
      `[data-rail-index="${i}"] button, [data-rail-index="${i}"] a[href]`,
    ) ?? null;

  // Apply the roving tabindex, and hand focus over once the target has mounted.
  //
  // Done imperatively rather than by threading a prop through `render`, because
  // `render` returns arbitrary caller-owned markup — the Rail should not require
  // every card component in the app to know it is being virtualised.
  useEffect(() => {
    const track = trackRef.current;
    if (!track) return;
    for (const el of track.querySelectorAll<HTMLElement>("[data-rail-index]")) {
      const i = Number(el.dataset["railIndex"]);
      const focusable = el.querySelector<HTMLElement>("button, a[href]");
      if (focusable) focusable.tabIndex = i === activeIndex ? 0 : -1;
    }
    if (wantsFocus.current) {
      const target = cardAt(activeIndex);
      // Not mounted yet: the scroll that will mount it has not been committed.
      // Leave the flag set and focus on a later pass.
      if (target) {
        target.focus({ preventScroll: true });
        wantsFocus.current = false;
      }
    }
  });

  /** Scroll the minimum distance that brings item `i` fully into view. */
  const revealIndex = (i: number) => {
    const el = viewportRef.current;
    if (!el) return;
    const x = offsetOf(i);
    const right = x + widthOf(i);
    if (x < el.scrollLeft) el.scrollLeft = x;
    else if (right > el.scrollLeft + el.clientWidth) el.scrollLeft = right - el.clientWidth;
    // Keep React's copy in step now, so the slice is recomputed even if the
    // rAF-throttled scroll handler has not fired yet.
    setScrollLeft(el.scrollLeft);
  };

  const moveTo = (i: number) => {
    const next = Math.max(0, Math.min(items.length - 1, i));
    wantsFocus.current = true;
    setActiveIndex(next);
    revealIndex(next);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key as "ArrowRight" | "ArrowLeft"];
    if (step !== undefined) {
      e.preventDefault();
      moveTo(activeIndex + step);
    } else if (e.key === "Home") {
      e.preventDefault();
      moveTo(0);
    } else if (e.key === "End") {
      e.preventDefault();
      moveTo(items.length - 1);
    }
  };

  const total = items.length === 0 ? 0 : offsetOf(items.length);

  let first = 0;
  if (scrollLeft > lead) first = Math.floor((scrollLeft - lead) / itemWidth) + 1;
  first = Math.max(0, first - overscan);

  const visibleCount = Math.ceil(viewportWidth / itemWidth) + overscan * 2 + 1;
  const last = Math.min(items.length, first + visibleCount);
  const slice = items.slice(first, last);

  return (
    <section className="border-b border-line-subtle">
      {/* The index column is a layout invariant (ADR-0024): every surface sits in
          a 96px | 1fr grid, and the hairline running down it is the spine of the
          whole design. Breaking it on one screen breaks the vertical rule. */}
      <div className="grid" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
        <div className="border-r border-line-subtle" />
        {/* min-w-0 is load-bearing. A grid item defaults to `min-width: auto`,
            which refuses to shrink below its content — so the 97,060px-wide
            virtualisation track pushed this `1fr` column to 97,060px, the page
            scrolled sideways, and the ResizeObserver reported a viewport wide
            enough to mount all 500 cards. Virtualisation silently did nothing. */}
        <div className="min-w-0 py-8 pl-8 pr-14">
          <header className="mb-5 flex items-baseline gap-4">
            <h2 className="font-display text-[15px] font-extrabold uppercase tracking-[-0.01em] text-ink">
              {label}
            </h2>
            <span className="h-px flex-1 bg-line-subtle" />
            {why && <span className="label max-w-[280px] text-right">{why}</span>}
            {onSeeAll && (
              <button
                type="button"
                onClick={onSeeAll}
                className="label transition-colors hover:text-ink"
              >
                See all
              </button>
            )}
          </header>

          <div
            ref={viewportRef}
            onScroll={onScroll}
            onKeyDown={onKeyDown}
            // Stable hook for tests and the 60fps measurement harness. Selecting
            // the scroll container by its utility classes would break the moment
            // the styling changed.
            data-rail-viewport
            // Native momentum and snap (§9.3) — never a jump-by-page carousel.
            className="overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
            style={{ scrollSnapType: "x proximity" }}
            role="list"
            aria-label={label}
          >
            {/* The track is a flex row in NORMAL FLOW, not absolutely positioned
                items. Absolute children contribute no height, so the track
                collapsed to zero and the rail rendered invisible. A leading
                spacer of exactly `offsetOf(first)` puts the mounted slice at the
                right scroll offset, and the cards give the row its height. */}
            <div ref={trackRef} className="flex" style={{ width: total || "100%" }}>
              <div className="shrink-0" style={{ width: offsetOf(first) }} aria-hidden />
              {slice.map((item, n) => {
                const index = first + n;
                return (
                  <div
                    key={keyOf(item, index)}
                    role="listitem"
                    data-rail-index={index}
                    className="shrink-0"
                    // Width is set here, not left to the card, so the offset
                    // arithmetic above cannot drift from what is actually drawn.
                    style={{ width: index === 0 ? lead : itemWidth, scrollSnapAlign: "start" }}
                  >
                    {render(item, index)}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
