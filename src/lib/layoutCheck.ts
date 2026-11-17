/**
 * What is wrong with the layout on screen now (Phase 16; the same rules as the pad-flow
 * audit's `layoutProblems`, for the real app through the debug probe): the page must not
 * scroll sideways; no text may be cut off inside its box; nothing may run off the right
 * edge except inside something that scrolls on purpose (a rail).
 */
export function layoutProblems(): string[] {
  const out: string[] = [];
  if (document.documentElement.scrollWidth > innerWidth + 1) {
    const culprit = [...document.body.querySelectorAll<HTMLElement>("*")].find(
      (el) => el.getBoundingClientRect().right > innerWidth + 1 && (el.parentElement?.getBoundingClientRect().right ?? 0) <= innerWidth + 1,
    );
    out.push(
      `the window scrolls sideways by ${document.documentElement.scrollWidth - innerWidth}px: ${
        culprit ? `${culprit.tagName.toLowerCase()}.${String(culprit.className).split(" ").slice(0, 5).join(".")}` : "unknown"
      }`,
    );
  }
  const pages = [...document.querySelectorAll<HTMLElement>('main, [role="dialog"][aria-modal="true"]')];
  for (const pg of pages) {
    if (pg.scrollWidth <= pg.clientWidth + 1) continue;
    // Name the outermost element that crosses the edge: the one to fix.
    const edge = pg.getBoundingClientRect().right + 1;
    const culprit = [...pg.querySelectorAll<HTMLElement>("*")].find(
      (el) => el.getBoundingClientRect().right > edge && (el.parentElement?.getBoundingClientRect().right ?? 0) <= edge,
    );
    const name = culprit
      ? `${culprit.tagName.toLowerCase()}.${String(culprit.className).split(" ").slice(0, 6).join(".")} (${Math.round(culprit.getBoundingClientRect().width)}px) "${(culprit.textContent ?? "").trim().slice(0, 30)}"`
      : "unknown";
    out.push(`the page scrolls sideways: ${name}`);
  }
  const inScroller = (el: Element) => {
    for (let a = el.parentElement; a && !pages.includes(a); a = a.parentElement) {
      const o = getComputedStyle(a).overflowX;
      if (o === "auto" || o === "scroll" || o === "hidden") return true;
    }
    return false;
  };
  for (const el of document.querySelectorAll<HTMLElement>("h1, h2, h3, p, button, a, label, li span")) {
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height) continue;
    const cs = getComputedStyle(el);
    const text = (el.textContent ?? "").trim().slice(0, 40);
    if (cs.overflow === "hidden" && cs.textOverflow !== "ellipsis" && el.scrollWidth > el.clientWidth + 2) {
      out.push(`cut off: ${text}`);
    }
    if (r.right > innerWidth + 1 && cs.position !== "fixed" && !inScroller(el)) out.push(`off the edge: ${text}`);
  }
  return [...new Set(out)].slice(0, 12);
}
