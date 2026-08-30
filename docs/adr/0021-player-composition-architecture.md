# 0021 — Player composition: still-frame on pause, region cutouts during playback

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 1 (binds 8, 11)
- **Resolves:** B2 · **Risk:** R1 (closed) · **Relates to:** ADR-0020

## Context

`SPEC.md` §4 requires custom player chrome drawn over the video with auto-hide, and
Phase 11's pause overlay is the project's signature screen. Spike A established that
neither can be done the obvious way:

- A native child HWND **always paints above** a sibling WebView2, so HTML cannot be
  composited over video. Three distinct attempts failed (`docs/RISKS.md` R1).
- The root cause is a Windows compositing property, not a Tauri bug: a transparent
  webview composites against what is behind the **window**, not against sibling
  child HWNDs.
- `ventic/ventic` — same architecture, shipping — states the same conclusion
  independently, and solves it by **inverting** the problem.

## Decision

**Two mechanisms, one per surface.**

**Paused — still-frame substitution.** Capture the frame with
`screenshot-raw window`, downscale to ~960×540, JPEG it, hand it to the page as a
data URI, render it as an `<img>` beneath the overlay, then hide the video child.
Playback is stopped, so nothing behind the overlay needs to be live, and everything
is HTML — normal compositing, full dim/blur, no constraints.

**Playing — region cutouts.** `SetWindowRgn` on the video child, full rect minus the
chrome silhouette. The page shows through the hole; painting *and* hit-testing both
follow the region. Shape rules in ADR-0020.

**`screenshot-raw window`, not `video`** — it captures at window resolution, so the
cost does not scale with a 4K source. That is what makes the budget hold.

## Consequences

Measured on the dev machine, **debug build** (numbers in `docs/eval-results.md`):

- Pause → overlay visible: **161 / 95 / 140 ms**, all within §11's 200 ms budget.
- `SetWindowRgn`: median **0.467 ms**, max 1.06 ms — free at the auto-hide cadence.
- **No flicker**: 758 samples at ~7 ms across 12 toggles, zero frames losing colour.
  Two defences make that hold and both are required: a **NULL class background
  brush** and swallowing `WM_ERASEBKGND`, plus `SetWindowRgn(redraw=false)`. Without
  them Windows erases the newly-exposed area before mpv repaints, which is exactly
  the strobe that would make auto-hide unusable.
- Hit-testing passes through the hole, verified structurally rather than by
  inference: `RealChildWindowFromPoint` reports `WRY_WEBVIEW` inside the hole and
  `SpikeVideoHost` outside, with a **pixel-exact** boundary.
- Region survives mid-playback resize; the hole tracks the new client size.

**Costs.** The §9.3 gradient scrim dies (ADR-0020). Chrome silhouette and region must
be kept in sync — a real invariant that Phase 11 must own, and a source of bugs if a
future change moves one without the other. Two different compositing paths exist in
one player, so a change to chrome has to be considered against both.

**Not required:** any patched or forked dependency. This runs on stock Tauri and
stock wry.

## Alternatives Considered

- **DirectComposition visual hosting** (wry PR #1762). The only path preserving §9.3
  fully, and it needs no `tauri-runtime-wry` changes, so it would be a
  `[patch.crates-io]` override rather than a fork. Rejected as the current answer:
  unreviewed after six weeks, in a contributor's fork, and the adjacent
  offscreen-rendering request has been open since 2021. **Retained as the upgrade
  path** — if it merges, revisit with a superseding ADR.
- **Layered top-level window for chrome.** DWM-composited, so per-pixel alpha would
  work. Not attempted: cutouts answered the question first, at lower complexity and
  with no second top-level window to keep positioned, focused and z-ordered.
- **HTML5 `<video>` + FFmpeg remux.** Compositing becomes free, but it trades away
  "plays everything", which is the more load-bearing promise. Last resort, unused.
- **Chrome beside the video.** Rejected by the author: amend §4 deliberately rather
  than back into it.
