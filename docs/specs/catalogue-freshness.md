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

**What makes it cheap — corrected by ADR-0032.** This document originally said IMDb's
files are sorted by id so a refresh could `seek_past` our maximum and take the tail.
**They are sorted as TEXT.** Measured over the real 12,761,311-row `title.basics`:

```text
sorted lexicographically: true
sorted numerically      : false
first numeric decrease at row 967,458:   tt10001008 -> tt1000101
last id in the file:                     tt9916880   (not the largest)
```

`"tt10001008" < "tt1000101"` as text, so a newly issued `tt45000000` lands in the
middle of the file. There is no position to seek to. Seeking past our maximum walked
into rows already held and died on a UNIQUE violation the first time it ran.

So the refresh **scans and filters by numeric id**:

```
for each row: if numeric_id(tconst) <= highest_id_already_stored { skip }
… insert the rest …
```

The saving that mattered survives, because it was never the seek. The expensive half is
inserting 2.7 million rows, and nothing already held is inserted or even parsed into
one. `TsvReader::seek_past` remains in use for crash resumption, where a cursor IS a
position in the file's own order and it is correct.

**What it costs:** the download (216 MB for `title.basics`), because gzip cannot be
seeked and IMDb publishes no changelog. It does **not** cost the database work of
re-inserting 2.7 million rows, which is the expensive half.

**What it misses:** a *revision* to an existing title — a corrected year, a rating
that moved. `title.ratings` is only 8 MB, so ratings can simply be re-applied in
full. Other field revisions are accepted as stale until a full re-ingest.

**Cadence:** weekly is ample for bulk. `title.episode` is far smaller and can run
daily.

**Measured**, 2026-09-04, `ingest refresh`: 1,542 titles added, 1,086,185 ratings
re-applied, **71 s** — against 219 s for a full re-ingest.

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

**Built in Phase 6, minimally; completed in Phase 12.** Phase 6 builds it from what a
backend *reports*: a candidate's title and year (the Internet Archive and the user's
own links both give them). `crates/sources/src/backfill.rs` holds the rules — called
by the search screen only after an empty result is shown, a year required, never a
second copy of a title already in that year, one row per search, marked with the
reporting backend in `media_items.backfilled_from` (migration 0017). Phase 12 adds
the two things that need its filename parser: reading a title and year out of a
candidate that carries neither, and merging a backfilled row into an IMDb match.

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
| 3 — search-triggered backfill | Phase 6 from backend-reported titles; Phase 12 adds the parser and the IMDb merge |
| 4 — full re-ingest | Phase 4 (already exists — `ingest imdb` is idempotent) |

---

## Phase 16 — the layers made automatic, and two bugs that kept them from running

The author, 2026-11-08: *"the problem of having latest tv shows and their latest episodes, as well
as latest movies still persists, they are not updated like i said they should be, at all."*

**What was wrong.** The launch-time `refresh_if_stale` remembers IMDb's validator as soon as the
216 MB is on disk, so an interrupted load never downloads it again. But only a later refresh resumed
the load, and a later refresh never came: the next launch found the validator current and stopped.
The author's catalogue had a refresh job left `running` since 2026-11-07, and the one before it took
twelve days to finish across launches. Episodes were never refreshed at all: `title.episode` was
loaded once, for series over 5,000 votes and anime.

**What it does now** (`freshness::refresh`, called by `src-tauri/src/commands/catalogue.rs`):

- **An unfinished refresh is finished first**, before the publisher is asked anything.
- **The resume reads past the watermark it started from**, kept in the `catalogue.refresh_watermark`
  setting. Taken afresh it would include the new ids the interrupted run already stored, and skip for
  good the new titles below them (the file is ordered as text). A refresh batch also drops ids it
  already holds, so a resume can never die on a duplicate.
- **Episodes are part of every refresh**: `title.episode` (52 MB) is fetched with the other two, and
  every episode not yet held is added for anime, a series over 5,000 votes, or **a series the user
  follows** (matched in the library, on a watchlist, or with a watched episode) whatever its votes.
- **The app checks every hour**, launch included: one HEAD request, and a download only when IMDb has
  republished (about daily). **AniList's next airing** is fetched every six hours for every followed
  anime. Settings shows when the catalogue last changed and what it added, with "Refresh now";
  Home refetches when a refresh adds something.
- `ingest keep` runs the same function from the command line.

**Not covered**: a show that IMDb splits where other sources do not. Twin Peaks' 2017 season is a
separate IMDb series (its 18 parts are that series' season 1), so files named `S03E01` under the 1990
show are placed on the 1990 series without an episode row. Mapping one numbering onto another needs a
source that publishes it (D85).

