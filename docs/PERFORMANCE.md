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
| Catalogue database | < 4 GB (R4 trigger) | — |

## Rules

- **Measure before optimising, and record the numbers** (§10.11 of CLAUDE.md, §2.3).
- A **> 10% regression** in any tracked benchmark fails CI from Phase 21.
- A quality regression in an earlier phase's eval metric blocks a merge exactly as a
  failing test does (§10.12).
