# 0049 — The author's fix list: what Phase 16 built, and what later phases deepen

- **Status:** Accepted 2026-11-17.
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** `SPEC.md` Appendix F (ADR-0046), spec_version 4.3.0.
- **Source:** `docs/design/author-brief-2026-11-17.md`, the author's instruction: "if any of these issues fit into any of
  the next phases, write it down in spec.md file, and those issues that dont fit need to be addressed right now."

## Context

The author tested the installed app and listed what was broken. All fifteen items of the resulting plan were fixed in
Phase 16 (commits in the brief and `SESSION_LOG.md`). Where a later phase already owns a feature, the version built now
is the usable core; this records what those phases add, so nothing the author asked for is lost between phases.

## Decision

Eight improvements join Appendix F, each assigned to the phase that owns its area:

- **I79** (Phase 19) Films and TV Shows: browsing by decade, country and director, and the genres too slow to read from
  the votes index today (documentaries, westerns, war), from a genre-and-votes index.
- **I80** (Phase 19) With the profile's TMDB key: backdrops on title pages and cast photos (ADR-0048 built posters).
- **I81** (Phase 19) Add-on logos on the add-on cards, fetched by the backend as posters are (a monogram stands in).
- **I82** (Phase 22) TMDB's poster address cached per title on disk (today one `find` call per title per session).
- **I83** (Phase 22) HEIC and AVIF profile pictures, through Windows' own decoder.
- **I84** (Phase 22) Two episodes of one season pack at once (known debt D87).
- **I85** (Phase 22) Logs and Downloads follow the data folder the app opened (known debt D88).
- **I86** (Phase 19) The keyless poster switch and the TMDB key as one "Artwork" choice in onboarding, once Phase 19's
  mockups settle how the first run looks.

## Consequences

Each is a deliverable of its phase, done or carried by name when the phase closes (ADR-0046). No exit criterion changes.
