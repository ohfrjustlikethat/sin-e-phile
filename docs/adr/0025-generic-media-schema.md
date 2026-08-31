# 0025 — The schema is generic over media kind

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 3
- **Relates to:** `SPEC.md` §6.2, Phase 3 (E5), Phases 24–25

## Context

`SPEC.md` §6.2 requires "one canonical `MediaItem` type from Phase 3, generic
enough to represent a film, a TV episode, an anime season, and — later, without
migration — a manga chapter."

Phase 3's exit criterion E5 requires an ADR recording why. This is it.

The alternative — a `films` table, an `episodes` table, and later a `manga` table —
is the obvious design, and it is what most media applications actually do. It is
worth stating plainly why it was rejected, because "we made it generic" is the kind
of decision that reads as over-engineering unless the cost of the alternative is
made concrete.

## Decision

**One `media_items` table with a `kind` discriminator carrying all eight values
from the first migration**: `film`, `episode`, `series`, `anime_film`,
`anime_series`, `live_channel`, `manga_chapter`, `comic_issue`.

Two of those — `manga_chapter` and `comic_issue` — are used by nothing until
Phases 24 and 25, which are Tier D and may never be built at all.

Kind-specific data lives in **one-to-one extension tables** (`series`, `episodes`)
rather than in nullable columns on `media_items`.

## Why

**1. Everything that references media references one thing.** Search, the
recommender, the watchlist, watch history, playback positions, local file matches,
and collections all point at a media item. With separate tables per kind, every one
of those becomes either a polymorphic association (a `kind` column plus an
unenforceable id) or N parallel tables. `watch_events` would become
`film_watch_events` and `episode_watch_events`, and Phase 15's taste model — which
is precisely about noticing that someone who watches Ozu also reads certain manga —
would have to union them back together at query time.

**2. The cost is asymmetric and known.** Adding a value to a SQLite `CHECK`
constraint is not `ALTER TABLE`. SQLite cannot alter a constraint: the procedure is
create a new table, copy every row, drop the old, rename, and recreate every index —
against a catalogue of hundreds of thousands of rows, in a migration that must also
run backward. Including two unused values in the constraint today costs nothing
measurable. Adding them in Phase 24 costs a data migration on a database holding a
user's irreplaceable history.

**3. `SPEC.md` names it as the test of the design.** Phase 24's exit criteria
include "**No database migration was required** — proving the Phase 3 design was
right." That is a criterion this phase is graded against later, and it is only
achievable by deciding now.

**4. It is honest about what the app is.** The pitch is a unified media engine, not
a film app that grew a TV tab. A schema in which a film and an episode are different
kinds of the same thing is the structural expression of that, and one where they are
different tables quietly is not.

## What genericity does NOT mean here

It does not mean everything is nullable and nothing is constrained. Specifically:

- **Extension tables, not nullable columns.** `series` and `episodes` are separate
  one-to-one tables keyed on `media_item_id`. Putting `total_episodes`,
  `absolute_number` and the rest on `media_items` would carry a dozen always-`NULL`
  columns across every one of 500,000 film rows — and the Phase 3 lookup budget is
  measured over exactly that table.
- **The discriminator is constrained.** A `CHECK` restricts `kind` to the eight
  values, so an unknown kind is rejected by the database rather than discovered
  later in Rust.
- **Kind-specific integrity is still enforced.** Foreign keys are on
  (SQLite ignores them otherwise), so an episode cannot reference a series that
  does not exist.

## Consequences

- Phases 24 and 25 need no schema migration, which is a stated Phase 24 criterion.
- A query for "films" carries a `WHERE kind = 'film'`. `idx_media_items_kind_year`
  and the other kind-leading indexes exist for exactly this, and the measured
  worst-case indexed lookup over 500,000 rows is 0.179 ms p99 against a 100 ms
  budget.
- `MediaKind::is_playable()` exists because "is this something you watch in one
  sitting" is a real question with a non-obvious answer once a series is a media
  item and a manga chapter is too.
- A test asserts the Rust enum and the SQL `CHECK` agree on all eight values, so
  they cannot drift.

## Verification

`crates/persistence/tests/migrations.rs::all_eight_media_kinds_are_accepted`
inserts and reads back one item of every kind, including the two nothing uses yet.
If a future migration narrows the constraint, that test fails.
