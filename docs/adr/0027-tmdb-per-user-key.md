# 0027 — TMDB runs on the user's own key, and no key ever ships

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 3 (decided at close)
- **Amends:** `SPEC.md` §14 (spec_version 1.5.0)
- **Closes:** P2 · **Completes:** ADR-0013, ADR-0018

## Context

ADR-0013 made TMDB optional: the app is fully functional on the offline
IMDb + MovieLens catalogue with no key at all. ADR-0018 recorded the reading of
TMDB's terms regarding AI/ML use — inference, not training — and a draft enquiry to
TMDB was written and left unsent at `docs/correspondence/tmdb-ai-clause.md`.

That enquiry has been open since Phase 0. The author has closed it.

## Decision

**No TMDB key ever ships with this application, in any form, on any channel.**

Each user optionally supplies **their own personal TMDB key**, in settings, under
their own acceptance of TMDB's terms. It unlocks artwork and richer detail. The app
is complete and good-looking without one.

**The enquiry is not sent and is no longer tracked.** The posture stands on its own:
users operate under their own keys and their own acceptance, and nothing derived is
redistributed. Revisit only if TMDB says otherwise.

## What this requires of the build

1. **The key is per-profile**, stored in `settings`, enterable and removable at any
   time. Not a build-time constant, not an environment variable the app depends on,
   not a value in any shipped file.
2. **Every TMDB-dependent surface degrades to the typographic treatment**, never
   breaks and never shows a grey rectangle. Phase 2 already built that state —
   `PosterCard`'s artwork-free card is *designed*, and §9.4 requires it to be
   "genuinely beautiful rather than a fallback."
3. **Removing the key is a supported action**, not an edge case: the app returns to
   the artwork-free state cleanly, and cached artwork obtained under the old key is
   discarded rather than retained.
4. **The onboarding step stays optional and never a dead end** (ADR-0013), framed as
   *unlock artwork and rich detail*.

## Why this is the right posture and not merely the cautious one

**It is the only arrangement where the legal question has a clean answer.** A shipped
key means the project is the API consumer, and every user's traffic is the project's
traffic under the project's acceptance. A per-user key means each user is their own
consumer under their own acceptance. There is no aggregation and no redistribution.

**It survives the AI clause without needing a ruling.** ADR-0018's reading —
inference on a user's own machine, over data the user fetched with their own key,
producing nothing that is redistributed — does not depend on TMDB agreeing with any
interpretation the project has published, because the project is not the party making
the calls.

**It costs the user very little and the project nothing.** Roughly thirty seconds,
offered once, skippable, and skippable permanently.

**It is already what the design assumes.** ADR-0013 made the artwork-free state a
designed state rather than a fallback, and Phase 2 built it. This ADR does not
introduce a degraded mode; it names the mode that already exists as the default one.

## Consequences

- `TMDB_API_KEY` remains an *optional* `tools/doctor` check for development
  convenience only. It must never become required, and the app must never read it at
  runtime in a shipped build — the runtime value comes from `settings`.
- Phase 4's live API client takes the key as a parameter from settings and returns a
  clearly-typed "no key configured" state rather than an error.
- Phase 14's onboarding and Phase 18's settings screen both own part of this and
  should be checked against points 1–4 above.
- `docs/correspondence/tmdb-ai-clause.md` is retained as a record of the reasoning,
  marked closed and unsent. It is not deleted: the analysis is the useful part, and
  the Phase 27 case study may want it.
- P2 is closed in `docs/DECISIONS_PENDING.md` and `PROJECT_STATE.json`.
