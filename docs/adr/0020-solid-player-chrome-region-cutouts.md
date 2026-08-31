# 0020 — Solid player chrome, cut out of the video window

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 1 (binds 8, 11)
- **Amends:** `SPEC.md` §9.3 → spec_version 1.3.0
- **Resolves:** B2 (with ADR-0021's still-frame path) · **Risk:** R1

## Context

- On Windows a native child HWND **always paints above** a sibling WebView2. Three
  distinct attempts to composite HTML over video failed; a transparent webview
  composites against what is behind the *window*, not against sibling children.
- Independently confirmed by `ventic/ventic`, shipping the same architecture:
  *"DOM controls can never be composited on top of the video."*
- Their inversion works and is measured here: **cut the chrome rectangle out of the
  video window** with `SetWindowRgn`, and the page shows through — painting and
  hit-testing both.
- **A window region is binary.** There is no per-pixel alpha through a hole. So
  §9.3's gradient scrim and blur-behind under the player chrome are not
  implementable on this path.

## Decision

Player chrome during playback is a **solid, opaque panel with a hard edge**.

- **Removed from §9.3:** the gradient scrim under player chrome, and blur-behind.
- **Retained:** rounded corners — but **the region must be rounded to match the
  panel silhouette** (`CreateRoundRectRgn`). A rectangular hole under a rounded
  panel leaves a corner gap, and because the window is transparent that gap shows
  **the desktop**, not the video. Measured: a visible pale wedge at each corner.
- The rule generalises: **the hole and the opaque chrome must be the same shape.**
  Any part of a hole the chrome does not paint is a window into whatever is behind
  the application.
- The **pause overlay is unaffected** and keeps its dim/blur, because it uses the
  still-frame path (ADR-0021), where everything is HTML and normal compositing
  applies.

## Consequences

- Playback chrome and pause overlay now have deliberately different visual
  treatments: crisp opaque panel while playing, dimmed and blurred still when
  paused. This should read as intentional, and the design should lean into it.
- Chrome geometry becomes load-bearing rather than cosmetic: every change to the
  panel's silhouette needs a matching region. Chrome that animates its shape (a
  slide-up, a width change) needs the region re-cut per frame — cheap (median
  0.467 ms) but no longer free.
- Irregular or soft-edged chrome is off the table. Circular buttons floating over
  video, feathered edges, and drop shadows onto the video are all unbuildable this
  way.
- Losing the scrim costs legibility: white text over a bright frame was previously
  protected by the gradient. The opaque panel replaces that protection entirely,
  which is arguably a better guarantee than a gradient that could be defeated by a
  bright shot.

## Alternatives Considered

- **Keep the gradient; accept that chrome cannot overlay video.** Rejected: it makes
  the promise unbuildable rather than the design different.
- **DirectComposition visual hosting** (wry PR #1762) would restore per-pixel alpha
  and the full §9.3 design. Rejected as the *current* answer — unreviewed after six
  weeks, in a contributor's fork — but retained as the upgrade path. If it merges,
  §9.3 can be revisited by a new ADR superseding this one.
- **Render chrome into the video with mpv's OSD/libass.** Rejected: it abandons
  React for the player UI, which is most of the Phase 11 surface.
- **Chrome beside the video, never over it.** Rejected by the author explicitly:
  amend §4 deliberately rather than back into it. Not needed now anyway.
