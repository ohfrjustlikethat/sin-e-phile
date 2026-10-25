# ADR-0032 — IMDb's files sort as text, so the refresh filters rather than seeks

**Status:** accepted
**Date:** 2026-09-04
**Phase:** 4 (subtask 4.13)
**Corrects:** [ADR-0030](0030-catalogue-freshness.md) layer 1, `SPEC.md` §15 Phase 4,
`docs/specs/catalogue-freshness.md`.

---

## Context

ADR-0030 specified layer 1 of catalogue freshness as:

> IMDb's files are sorted by id and new titles get higher ids, so a refresh is
> `seek_past(highest_id_we_have)` and insert the tail. That is the resumption machinery
> already built, used for a second purpose.

It is a good idea and it does not work. **`title.basics` is sorted
lexicographically**, and lexicographic order stopped agreeing with numeric order the
moment IMDb issued its ten-millionth id. Measured over the real 12,761,311-row file:

```text
sorted lexicographically: true
sorted numerically      : false
first numeric decrease at row 967,458:   tt10001008 -> tt1000101
last id in the file:                     tt9916880   (not the largest)
```

`"tt10001008" < "tt1000101" < "tt9916880"` as text. So a newly issued `tt45000000`
lands in the `tt4…` block, roughly *half way through the file*, not at the end.

The catalogue's highest id was `tt44917931`. Seeking past it walked into the
`tt5…`–`tt9…` rows already held, and the first insert died on
`UNIQUE constraint failed: external_ids.source, external_ids.external_id`. The bug was
not subtle once it ran; it was invisible until then, because every fixture used ids
short enough for the two orderings to agree.

## Decision

**The watermark is a number to compare against, never a position to seek to.**

`refresh::watermark` returns `Option<u32>` — the highest numeric IMDb id the catalogue
holds, excluding episodes — and `load_titles` takes `above_id: Option<u32>`, skipping
any row whose numeric id is at or below it. The check happens before any other field is
parsed, because on a real refresh it rejects 12.7 million of 12.8 million rows.

`TsvReader::seek_past` stays exactly where it was, for **crash resumption**, where it
remains correct: a job cursor is the last id processed *in file order*, and seeking to
a position in the order the file is actually in is precisely what it does.

## What this costs, measured

| | |
|---|---|
| Titles added | 1,542 |
| Ratings re-applied | 1,086,185 |
| **Refresh** | **71 s** |
| Full re-ingest, for comparison | 219 s |

**The saving ADR-0030 was actually after survives.** Its reasoning was that the
expensive half is inserting 2.7 million rows, not reading them — that is still true,
and nothing already held is inserted or even built into a row. What is lost is the seek
itself, which was never the costly part. A three-fold saving on a weekly job is ample.

## Consequences

- ADR-0030's decision stands in full; only the mechanism of layer 1 changes. Layers 2,
  3 and 4 are untouched.
- `docs/specs/catalogue-freshness.md` and `SPEC.md` §15 Phase 4 are corrected, and
  `SPEC.md` gains amendment A24 recording that the original text was wrong rather than
  merely superseded.
- **A rule for every future dataset integration**: check how a file is actually ordered
  before building on an assumption about it. The check is four lines and would have
  saved this.
- Fixture ids must exceed seven digits somewhere, or they cannot reproduce the class of
  bug at all. `a_new_id_sorted_before_ids_we_hold_is_still_found` uses the exact
  transition from row 967,458 of the real file.

## Alternatives considered

**`INSERT … ON CONFLICT DO NOTHING` and re-read everything.** Would have made the crash
go away without making the mechanism correct: it still re-parses and re-attempts every
row, and it would have hidden the ordering discovery entirely. Rejected — the symptom
was the useful part.

**Sort our own copy of the file numerically.** 12.7 million rows re-sorted on every
refresh, to save a scan that takes seconds. Rejected as strictly worse.
