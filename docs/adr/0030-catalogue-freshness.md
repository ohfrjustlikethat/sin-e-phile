# 0030 — The catalogue refreshes in layers

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 4
- **Amends:** `SPEC.md` §7, Phase 4, Phase 6, Phase 11 (spec_version 1.7.0)
- **Specification:** `docs/specs/catalogue-freshness.md`

## Context

The IMDb datasets are a snapshot; IMDb republishes daily and we ingest once. The
author's concern: *"we need to find a way to keep latest titles updating, find a
solution that is smart and clever, I do not want my app to have an outdated
catalogue."*

Two failures follow, and they are different. A film released after ingestion is not
in the catalogue **at all** — it cannot be searched for. A weekly TV episode that
aired last night is **not listed**, so a series stops short and "next episode" points
at nothing. Neither is a torrent-layer problem: a source may exist and the app does
not know to look.

## Decision

Four layers, specified in `docs/specs/catalogue-freshness.md`.

**1. Incremental bulk refresh.** IMDb's files are sorted by id and new titles get
higher ids, so a refresh is `seek_past(highest_id_we_have)` and insert the tail. That
is the resumption machinery already built, used for a second purpose. Costs the
download, not the 2.7-million-row insert.

**2. Airing schedules.** AniList publishes exact air times through a public GraphQL
API with **no key**, which solves anime properly. General TV falls back to a daily
`title.episode` refresh, or `/tv/changes` when the user has supplied a TMDB key.

**3. Search-triggered backfill.** If a search misses and the resolver finds a
candidate anyway, create the entry from what the resolver learned. **This is the layer
that makes "outdated" stop being a dead end**, and nothing in the spec covered it.

**4. Full re-ingest.** Measured at 219 s for 2.7 million titles, which is the fact
that keeps layers 1–3 from needing to be clever.

## Why layer 3 is the important one

Layers 1, 2 and 4 all shrink the window between a title existing and being known.
None of them closes it, and none can — there will always be something newer than the
last refresh.

Layer 3 changes the shape of the failure rather than its size. The catalogue stops
being a snapshot and becomes something that fills in **where the user actually
looks**. The long tail nobody searches for costs nothing; the one film someone wanted
appears. A miss becomes self-healing instead of permanent.

It also composes with everything else: the filename parser needed for Phase 12
already extracts title, year, season and episode, which is enough for a minimal row.

## Consequences

- Phase 4 gains subtask 4.13, the incremental refresh.
- Phase 6 owns backfill, since it needs the resolver.
- Phase 11 must be able to show "S03E07 airs Thursday" **without an episode row** —
  an episode can be known to be airing before it is known to exist.
- Backfilled entries must be marked, so a later bulk refresh reconciles rather than
  duplicating, and a backfilled row that later matches a real IMDb id is merged.
- Backfill runs after results are shown. It must never block a search.
- Honest limits, stated in the spec rather than implied away: there is a window of
  hours to days; withdrawn titles are only removed by a full re-ingest; and without a
  TMDB key, general-TV episode freshness is bounded by IMDb's own publication lag.
