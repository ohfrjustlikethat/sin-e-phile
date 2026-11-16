import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useCallback, type ReactElement } from "react";
import { backLabel } from "./BackBar";
import { forgetScroll, useScrollMemory } from "@/lib/scrollMemory";
import type { Tab } from "@/lib/store";

const empty = { home: [], films: [], tv: [], watchlist: [], live: [], search: [] } as Record<Tab, number[]>;
const base = {
  tab: "home" as Tab,
  destination: "home" as const,
  trails: empty,
  settingsOpen: false,
  downloadsOpen: false,
  reviewOpen: false,
  tasteOpen: false,
};
const titles: Record<number, string> = { 1: "Heat", 2: "Thief" };
const titleOf = (id: number) => titles[id];

describe("The back arrow says where it goes (Phase 16)", () => {
  it("names the tab, the previous title, or the page under Settings", () => {
    expect(backLabel(base, titleOf)).toBeNull();
    expect(backLabel({ ...base, trails: { ...empty, home: [1] } }, titleOf)).toBe("Home");
    expect(backLabel({ ...base, trails: { ...empty, home: [1, 2] } }, titleOf)).toBe("Heat");
    expect(backLabel({ ...base, tab: "search", trails: { ...empty, search: [2] } }, titleOf)).toBe("Search results");
    expect(backLabel({ ...base, tab: "search", destination: "films" } as never, titleOf)).toBe("Films");
    expect(backLabel({ ...base, settingsOpen: true, trails: { ...empty, home: [2] } }, titleOf)).toBe("Thief");
    expect(backLabel({ ...base, downloadsOpen: true, tab: "tv" }, titleOf)).toBe("TV Shows");
    expect(backLabel({ ...base, settingsOpen: true, reviewOpen: true }, titleOf)).toBe("Settings");
  });
});

/** A scroller whose scrollTop is kept, as a browser would (jsdom has no layout). */
function Scroller({ page, seq, shown = true }: { page: string; seq: number; shown?: boolean }): ReactElement {
  const remember = useScrollMemory(page, seq);
  const attach = useCallback(
    (el: HTMLDivElement | null) => {
      if (el && !Object.getOwnPropertyDescriptor(el, "scrollTop")) {
        let top = 0;
        Object.defineProperty(el, "scrollTop", { get: () => top, set: (v: number) => (top = v), configurable: true });
      }
      remember(el);
    },
    [remember],
  );
  // `shown` stands for App's scroll area, removed while the full-screen player is up.
  return <>{shown && <div ref={attach} data-testid="scroller" />}</>;
}

describe("Each page keeps its scroll position (Phase 16)", () => {
  it("puts a page back where it was left, and a tab pressed again at its start", () => {
    const { rerender } = render(<Scroller page="home|" seq={0} />);
    const el = screen.getByTestId("scroller");
    el.scrollTop = 640;
    el.dispatchEvent(new Event("scroll"));
    rerender(<Scroller page="home|7" seq={1} />);
    expect(el.scrollTop).toBe(0);
    rerender(<Scroller page="home|" seq={2} />);
    expect(el.scrollTop).toBe(640);
    forgetScroll("home|");
    rerender(<Scroller page="home|" seq={3} />);
    expect(el.scrollTop).toBe(0);
  });

  it("keeps recording when the scroll area is a new element (after the full-screen player)", () => {
    const { rerender } = render(<Scroller page="films|" seq={0} />);
    rerender(<Scroller page="films|" seq={0} shown={false} />);
    rerender(<Scroller page="films|" seq={0} />);
    const el = screen.getByTestId("scroller");
    el.scrollTop = 300;
    el.dispatchEvent(new Event("scroll"));
    rerender(<Scroller page="films|9" seq={1} />);
    rerender(<Scroller page="films|" seq={2} />);
    expect(el.scrollTop).toBe(300);
  });
});
