# Performance

Every budget, its current measurement, and how it was achieved.

> **Status: Phase 0.** No application exists yet, so there is nothing to measure.
> Phase 21 is the dedicated performance phase, but measurements are recorded here
> **as each phase produces them**, not deferred to the end — `SPEC.md` §10.12
> compares every phase against the previous phase's numbers, which requires the
> numbers to exist.

## The budgets

From `SPEC.md` §2.3. **Tier 0 governs** — under 8 GB RAM, no hardware decode, two
cores. Phase-level targets on the dev machine are Tier 2 numbers and are tracked
separately.

| Metric | Budget (Tier 0) | Current | Measured how |
|---|---|---|---|
| Cold start to interactive | < 4.0 s (< 2.0 s Tier 2) | — | Phase 1 |
| Search keystroke → results | < 80 ms p95, incl. query embedding | — | Phase 5 eval harness |
| Play → first frame, local | < 500 ms | — | Phase 8 |
| Play → first frame, healthy swarm | < 8 s | — | Phase 7 |
| Idle RAM | < 250 MB (< 200 MB Tier 2) | — | Phase 1 |
| Home screen scroll | 60 fps sustained | — | Phase 2, Phase 18 |
| Installed size | < 120 MB excl. optional downloads | — | Phase 27 |
| Rank 1,000 candidates | < 150 ms | — | Phase 16 |
| Home render, 20 rails | < 800 ms | — | Phase 18 |
| Scan 10,000 files | < 3 min | — | Phase 12 |

## Recorded artefact sizes

| Artefact | Budget | Actual |
|---|---|---|
| Embedding artefact | ~77 MB per 200k titles (ADR-0014) | — |
| Catalogue database | < 4 GB (R4 trigger) | 145.4 MB at 500k synthetic items (Phase 3) |

## Phase 3 — data layer

Measured on the dev machine (Tier 2), release build, 500,000 synthetic media items.
Full table and method in `docs/eval-results.md`.

| Operation | Budget | p50 | p99 |
|---|---|---|---|
| `by_id` | 100 ms (E3) | 0.032 ms | 0.098 ms |
| `by_exact_title` | 100 ms (E3) | 0.081 ms | 0.179 ms |
| `by_external_id` | 100 ms (E3) | 0.056 ms | 0.120 ms |
| Bulk insert 500k rows | none (amendment 15) | 43.7 s | — |

`by_exact_title` was 26.679 ms until the index was given `COLLATE NOCASE` to match
the query's comparison — SQLite silently full-scans when the collations differ. It
passed the criterion either way; it would not have survived Phase 5.

## Rules

- **Measure before optimising, and record the numbers** (§10.11 of CLAUDE.md, §2.3).
- A **> 10% regression** in any tracked benchmark fails CI from Phase 21.
- A quality regression in an earlier phase's eval metric blocks a merge exactly as a
  failing test does (§10.12).
