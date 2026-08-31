# Catalogue freshness

How the catalogue stops being a snapshot. Four layers, cheapest first.

Written 2026-09-01 (Phase 4), after the author's concern: *"we need to find a way to
keep latest titles updating… I do not want my app to have an outdated catalogue."*
Decided in ADR-0030.

---

## The problem, stated precisely

The IMDb datasets are a **snapshot**. IMDb republishes them daily; we ingest once.
From the moment ingestion finishes, the catalogue is out of date and gets worse.

Two distinct failures follow, and they need different answers:

- **A film released after the last ingestion is not in the catalogue at all.** It
  cannot be searched for, so as far as the app is concerned it does not exist.
- **A weekly TV episode that aired last night is not listed**, so a series page shows
  a season that stops short and "next episode" has nothing to point at.

Neither is solved by the torrent layer. A source may well exist; the app does not
know there is anything to look for.

---

## Layer 1 — Incremental bulk refresh

**What:** re-fetch `title.basics`, `title.ratings` and `title.episode`, and insert
only what is new.

**The trick that makes it cheap.** IMDb's files are **sorted by id**, and new titles
receive higher ids. So a refresh is:

```
seek_past("tconst", highest_tconst_already_stored)
… insert the tail …
```

That is the `TsvReader::seek_past` already built for resumption
(`tools/ingest/src/tsv.rs`), used for a second purpose. No new machinery.

**What it costs:** the download (216 MB for `title.basics`), because gzip cannot be
seeked and IMDb publishes no changelog. It does **not** cost the database work of
re-inserting 2.7 million rows, which is the expensive half.

**What it misses:** a *revision* to an existing title — a corrected year, a rating
that moved. `title.ratings` is only 8 MB, so ratings can simply be re-applied in
full. Other field revisions are accepted as stale until a full re-ingest.

**Cadence:** weekly is ample for bulk. `title.episode` is far smaller and can run
daily.

---

## Layer 2 — Airing schedules

Layer 1 tells you an episode exists once IMDb lists it, which is not immediate.

**Anime is solved properly.** AniList publishes exact airing schedules through a
public GraphQL API **with no key**, including the next episode's air time. That is
already in Phase 4's scope for the anime catalogue; the schedule query is a small
addition to a client that has to exist anyway.

**General TV needs a key, or patience.** TMDB's `/tv/{id}/changes` gives changed
ids per day but requires the user's key (ADR-0027 makes that optional and per-user).
Without one, Layer 1's daily `title.episode` refresh is the fallback — later than
AniList, but not by much.

**Consequence for the UI:** an episode may be *known to be airing* before it is
*known to exist*. A series page should be able to show "S03E07 airs Thursday"
without an episode row, which is a display concern for Phase 11.

---

## Layer 3 — Search-triggered backfill

**This is the layer that makes "outdated" stop being a dead end**, and it is the one
nothing in `SPEC.md` covers today.

If a user searches for something the catalogue does not contain, and the source
resolver finds a candidate for it, **create the catalogue entry from what the
resolver learned.** The filename parser (Phase 12) already extracts a title, a year
and frequently a season and episode; that is enough for a minimal `media_items` row,
which live enrichment can fill out later if a key is present.

The catalogue stops being a fixed snapshot and becomes something that **fills in
where the user actually looks**. A long tail nobody searches for costs nothing;
the one film someone wanted appears.

**Constraints:**

- Entries created this way must be **marked as such**, so a later bulk refresh can
  reconcile them against the authoritative dataset rather than duplicating them.
- Matching on title and year is a *guess*. A backfilled entry that later matches a
  real IMDb id should be merged, not left as a second copy of the same film.
- This runs at search time and must not block the search. It is a background write
  after results are shown.

**Depends on Phase 6** (the resolver) and reads best alongside Phase 12's parser, so
it is specified here and built there.

---

## Layer 4 — Full re-ingest

Occasionally, everything. Slow, complete, and the only thing that catches deletions
and field revisions. Measured at **219 s for 2.7 million titles**, so "slow" here
means minutes, not hours — which makes this far less painful than it sounds and is
itself an argument against over-engineering layers 1–3.

---

## What this does not do

- It does not make the catalogue live. There is a window — hours to days — between a
  title existing in the world and existing here.
- It does not remove titles IMDb has withdrawn, except at Layer 4.
- Without a TMDB key, general-TV episode freshness is bounded by IMDb's own
  publication lag.

Saying so plainly is better than implying a freshness the design cannot deliver.

---

## Ownership

| Layer | Built in |
|---|---|
| 1 — incremental bulk refresh | Phase 4 (subtask 4.13) |
| 2 — airing schedules | Phase 4 for AniList; Phase 11 for the UI |
| 3 — search-triggered backfill | Phase 6, using Phase 12's parser |
| 4 — full re-ingest | Phase 4 (already exists — `ingest imdb` is idempotent) |
