# ADR-0031 — Absolute episode numbering is not published by any free source

**Status:** accepted
**Date:** 2026-09-04
**Phase:** 4 (subtask 4.4)
**Supersedes:** nothing. **Amends:** `SPEC.md` §6.2, §11 (AniList row).

---

## Context

`SPEC.md` §6.2 states that anime "specifically requires: absolute vs seasonal episode
numbering reconciliation", and §11 lists AniList as providing "absolute/seasonal
numbering". Migration 0003 was built on that: `episodes` carries both
`episode_number` and `absolute_number`, and `episode_numbering` records what each
source calls an episode so that Phase 12 can resolve a filename by lookup.

The seasonal half is now loaded and correct — 539,817 episodes from IMDb
`title.episode`, with `episode_numbering.source = 'imdb'`.

**The absolute half cannot be loaded, because no source we have publishes it.**

AniList publishes an episode *count* and an airing schedule per **entry**, and an
AniList entry is one cour. Its episode numbers therefore restart at 1 each season, in
exactly the same place IMDb's do. Queried against the live API and confirmed: there is
no absolute field, and no query returns one. The §11 row was wrong.

Two further obstacles, found while looking for a way round it:

1. **An absolute number could only be derived**, by ordering a series' AniList entries
   and cumulating their episode counts. Migration 0003 exists to prevent exactly this,
   in its own words: *"the conversions are not arithmetic, because cours split
   unevenly, recaps are numbered by some sources and not others, and specials
   interleave"*. A derived number would be silently wrong for precisely the
   long-running series the feature is for.

2. **We map one AniList entry per catalogue series.** Seasons 2 and later resolve to
   `already_claimed` and carry no AniList id — 409 of them. Even a derivation has
   nothing to iterate over without first redesigning the claim rule.

## Decision

**`absolute_number` stays NULL, and the specification is corrected to describe what is
actually stored rather than what was assumed to be available.**

The columns and the reconciliation table stay exactly as they are. They were designed
to hold what each source *says*, and holding nothing for a number no source says is
the schema working as intended, not a gap in it.

`SPEC.md` §6.2 is amended to state that the schema **supports** absolute numbering and
that it is populated when a source publishing one is added. §11's AniList row is
corrected to "per-entry episode numbering and airing schedules".

The problem is recorded against **Phase 12**, which is where it bites: a filename
reading `Series - 59` cannot be resolved from the catalogue for anime, and Phase 12
must fall back to its review queue rather than assuming a lookup will succeed.

## Options rejected

**Derive and store it now.** One line of arithmetic, silently wrong for any series
with a recap, a split cour, or an interleaved special — most long-running anime. A
confidently wrong number is worse than a NULL, because nothing downstream can tell it
is wrong. This is the option migration 0003 was written to make unnecessary, and
taking it would defeat the design.

**Redesign the claim rule now** so each AniList season keeps its own id and its own
`episode_numbering` rows, making an absolute number derivable from stored facts rather
than computed from guesses. This is the *right* eventual answer, and it is roughly a
session's work that also rewrites data already written. Deferred to Phase 12, which is
the code that will actually need it and can drive the design rather than guessing at
it eight phases early.

**TVDB**, which does publish absolute numbering. A fifth external dependency and a
per-user key, for one field, eight phases before anything reads it. Reconsider in
Phase 12 alongside the redesign.

## Consequences

- Phase 4 does not claim absolute numbering as delivered. Its exit criteria are
  unaffected; none of them named it.
- Phase 12's filename matcher inherits a known gap, recorded rather than discovered.
  Its `< 1% false-confident` budget is unaffected — the fallback is the review queue,
  which is a correct outcome, not a wrong answer.
- `episode_numbering.absolute_number` remains in the schema, unused, deliberately.
  Removing it would cost a migration to add back.

## Evidence

- 539,817 episodes and 21,218 seasons loaded; every `absolute_number` NULL.
- AniList's schema, queried live: no absolute-number field on `Media` or
  `AiringSchedule`.
- Measured episode costs and the full scope curve: `docs/eval-results.md`.
- The decision as it was put to the author, with options and costs:
  `docs/DECISIONS_PENDING.md` P10. Answered "your call" on 2026-09-04, which by the
  §10 protocol takes the stated default — this option.
