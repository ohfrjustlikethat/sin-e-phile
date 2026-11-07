# 0044 — Release polish, reading formats, and an unsigned release

- **Status:** Accepted 2026-11-07, decided by the author question by question after asking
  "how can I make this app suited and ready and perfected for a public release … what edge
  can I provide … subtle animations, for example exiting the video player", and "the manga
  and comics section should extend to reading epub and other formats, with UI and GUI
  navigation rivalling apps like Webtoon and Marvel Unlimited and ComiXology … integrated
  well with the Xbox controller."
- **Date:** 2026-11-07
- **Phase:** 18 (the player exit, shared-element motion, seamless resume); 21 (memory measured
  across the whole process tree); 25 (reading formats, Guided View, controller reading);
  27 (an unsigned release; no catalogue redistributed)
- **Amends:** SPEC.md §15 (Phases 18, 21, 25, 27). Builds on [ADR-0042](0042-tv-focus-language.md)
  and [ADR-0043](0043-tv-keyboard-and-interface-decisions.md).

## Context

The author asked what would make the app feel finished to a general public on first use,
and fixed the reading section's ambitions. Several suggestions turned out to be planned
already: Windows media controls (System Media Transport Controls, Phase 20) and importing a
Letterboxd or Trakt history (Phases 14 and 19). This ADR records what is new and confirms
those.

Two facts surfaced while answering, and both constrain a public release:

- **The IMDb datasets are licensed for personal, non-commercial use**, and MovieLens does
  not generally permit redistribution (RISKS.md R-terms). So a release can never ship a
  pre-built catalogue: each installation downloads and builds its own on first run, as it
  does today.
- **The idle-memory number on record (42.2 MB, Phase 1) counts the main process only.**
  WebView2's own processes draw the interface and are usually the larger share; they were
  never measured.

## Decision

**Leaving the player: the video shrinks into its tile.** On exit, the playing picture
shrinks (about 350 ms, a critically damped spring) into the poster or card it was launched
from, while the page fades in behind it. Where no tile is on screen (played from search, a
deep link, or a dropped file), it shrinks to the page's centre and dissolves. Reduced motion
replaces it with a 200 ms dissolve.

**Shared-element motion.** A poster grows into its film page, and the page's artwork into the
player, so each step reads as one continuous object. The motion lives with the focus
language in `src/design-system/focus/`, ADR-0042's one exception to §9.0's bans; reduced
motion removes every transform.

**Seamless resume.** Relaunching returns to where the viewer was: the page, its scroll and
focus, and an offer to resume the film at its position ("Resume *Heat* at 1:12:04?"). After a
crash the same state is restored and the crash is said plainly, once.

**Confirmed, already planned:** Windows media controls with now-playing metadata and artwork
(Phase 20), and importing a Letterboxd export or a Trakt history (Phases 14 and 19).

**Reading (Phase 25), beyond CBZ, CBR, CB7, PDF and image folders:**
- **EPUB**: fixed-layout EPUB comics in the comic reader, and reflowable EPUB books and light
  novels in a typographic reader (font, size, margins, line height, night and sepia
  themes, chapter navigation).
- **MOBI and AZW3**, DRM-free files only. DRM is never removed or circumvented.
- **Guided View**: A steps panel by panel with a smooth zoom to each, as ComiXology does.
  Panels are detected automatically; a page where detection is unsure falls back to page
  view rather than guessing.
- **The controller reads as well as it browses**: A turns or steps, LB and RB move by page,
  the triggers by chapter, the right stick zooms and pans, the left stick scrolls a webtoon,
  B returns to the shelf, Menu opens the reader's options. The bar is Webtoon, Marvel
  Unlimited and ComiXology.

**Distribution: unsigned.** The author chose not to pay for code signing. Phase 27 releases
an unsigned build, and the README and download page say plainly what Windows SmartScreen will
show on first launch ("More info", then "Run anyway") and why. This is revisitable at any
release (signing would be ADR-0040-reserved, since it costs money).

**Measured honestly.** Phase 21's memory budget is measured over the whole process tree,
WebView2's processes included, in the release build.

## Placement

| What | Phase |
|---|---|
| Player exit into its tile; shared-element motion; seamless resume and crash restore | 18 |
| Idle memory measured across the whole process tree | 21 |
| EPUB (fixed and reflowable), MOBI/AZW3 (DRM-free), Guided View, controller reading | 25 |
| Unsigned release with honest SmartScreen guidance; no pre-built catalogue distributed | 27 |
| Windows media controls (confirmed) | 20 |
| Letterboxd / Trakt history import (confirmed) | 14, 19 |

## Consequences

- Phase 18 gains three deliverables and two exit criteria; Phase 25 gains formats, a mode,
  a controller scheme and three exit criteria; Phases 21 and 27 gain one criterion each.
  No exit criterion is weakened and no decision is reversed.
- Guided View's panel detection is image analysis; it is built to degrade (page view)
  rather than to be perfect, and its accuracy is measured on a fixture set.
- An unsigned release will lose some first-time users at the SmartScreen prompt. Accepted.
