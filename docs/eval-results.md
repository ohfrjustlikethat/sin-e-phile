# Evaluation results

**Every eval-harness metric and performance measurement, recorded the moment it is
produced.** Metric, value, date, commit, and the command that produced it.

This file is the one documentation output that is **never deferred** (ADR-0016).
Prose can be regenerated from the code later; a number cannot. Re-running a harness
at Phase 27 gives *today's* number, not the number at that commit, which makes
retrospective regression detection impossible.

`SPEC.md` §10.12 depends entirely on this table: before merging any phase, every
harness runs and is compared against the previous phase's numbers here. **A quality
regression blocks a merge exactly as a failing test does.**

Append; never edit a past row. A corrected number is a new row saying so.

---

## Eval harnesses

Targets from `SPEC.md` §12.2.

| Harness | Metric | Target | Value | Date | Commit | Command |
|---|---|---|---|---|---|---|
| *(Phase 5)* Search relevance | nDCG@10 | > 0.75 | — | — | — | `cargo run -p eval -- search --report` |
| *(Phase 5)* Search relevance | exact-title top-1 | 100% | — | — | — | `cargo run -p eval -- search --report` |
| *(Phase 10)* Subtitle alignment | within 150 ms | > 90% | — | — | — | `cargo run -p eval -- subtitles --report` |
| *(Phase 12)* Filename identification | top-1 accuracy | > 95% | — | — | — | `cargo run -p eval -- filenames --report` |
| *(Phase 12)* Filename identification | false-confident rate | **< 1%** | — | — | — | `cargo run -p eval -- filenames --report` |
| *(Phase 16)* Recommender | Recall@20 vs popularity | **+40% relative** | — | — | — | `cargo run -p eval -- recommender --report` |
| *(Phase 16)* Recommender | catalogue coverage | > 15% | — | — | — | `cargo run -p eval -- recommender --report` |
| *(Phase 16)* Recommender | intra-list diversity | documented target | — | — | — | `cargo run -p eval -- recommender --report` |
| *(Phase 17)* Discovery | novelty (mean inverse popularity) | documented target | — | — | — | `cargo run -p eval -- discovery --report` |

## Performance measurements

Budgets from `SPEC.md` §2.3. **Tier 0 governs**; phase-level dev-machine numbers are
Tier 2 and are labelled as such.

| Metric | Budget | Tier | Value | Date | Commit | Command |
|---|---|---|---|---|---|---|
| *(Phase 1, Spike C)* Query embedding p95 | **escalate if > ~30 ms** | 0 | — | — | — | spike harness |
| *(Phase 1, Spike C)* Query embedding p95 | — | 2 | — | — | — | spike harness |
| *(Phase 1, Spike B)* Time to first usable bytes | escalate if > 20 s | — | — | — | — | spike harness |
| *(Phase 1)* Cold start to interactive | < 4 s (T0) / < 2 s (T2) | — | — | — | — | — |
| *(Phase 1)* Idle RAM | < 250 MB (T0) / < 200 MB (T2) | — | — | — | — | — |
| *(Phase 3)* Indexed lookup, 500k rows | < 100 ms | — | — | — | — | — |
| *(Phase 5)* Keystroke → results p95 | < 80 ms *incl. embedding* | 0 | — | — | — | — |
| *(Phase 7)* Play → first frame, healthy swarm | < 8 s | — | — | — | — | — |
| *(Phase 8)* Play → first frame, local | < 500 ms | — | — | — | — | — |
| *(Phase 16)* Rank 1,000 candidates | < 150 ms | 0 | — | — | — | — |
| *(Phase 18)* Home render, 20 rails | < 800 ms | 0 | — | — | — | — |
| *(Phase 12)* Scan 10,000 files | < 3 min | — | — | — | — | — |

## Artefact sizes

| Artefact | Budget | Value | Date | Commit |
|---|---|---|---|---|
| Embedding artefact | ~77 MB / 200k titles (ADR-0014) | — | — | — |
| Catalogue database | < 4 GB (R4 trigger) | — | — | — |
| Installed size | < 120 MB excl. optional downloads | — | — | — |

---

## Phase 1 — Spike A (libmpv in Tauri, risk R1)

Dev machine (Tier 2). libmpv `20260830-git-e8673660ab` x86_64, loaded dynamically.
Test clip generated locally with FFmpeg (`testsrc2` 1280x720 h264 + aac, 15 s) — no
download, no posture question.

| Measurement | Value | Date | Commit | Command |
|---|---|---|---|---|
| libmpv load + `mpv_create` | 14.1 ms | 2026-08-31 | spike | `step1-own-window` |
| `mpv_initialize` | 1.5 ms | 2026-08-31 | spike | `step1-own-window` |
| file-loaded, own window | 216 ms | 2026-08-31 | spike | `step1-own-window` |
| **First frame, own window** | **1353 ms** | 2026-08-31 | spike | `step1-own-window` |
| **First frame, into a child HWND (`wid`)** | **970 / 1014 / 1023 ms** (3 runs) | 2026-08-31 | spike | `step2-child-hwnd` |
| Hardware decode selected | **d3d11va** | 2026-08-31 | spike | `hwdec-current` property |
| Video output driver | gpu-next, `d3d11[nv12]` | 2026-08-31 | spike | mpv log |
| Playback survives parent resize | yes — `time-pos` advanced 2.2 s through a mid-playback resize | 2026-08-31 | spike | `step2-child-hwnd` |

Note: first-frame figures include window creation and process start, and are **not**
the §2.3 "play → first frame < 500 ms" budget, which is measured from a warm player
in the real app. Recorded as a Spike A baseline only.

## Phase 0

No harnesses exist yet. Recorded for completeness, since the guard's self-test is the
only measured thing in the repository so far:

| Metric | Value | Date | Commit | Command |
|---|---|---|---|---|
| Posture guard self-test | 31/31 pass (12 must-fire, 17 must-not-fire, 2 structural) | 2026-08-31 | `66fd304` | `python tools/guard/guard.py --selftest` |
| Guard false positives on tree | 0 (from 118 at first run) | 2026-08-31 | `66fd304` | `python tools/guard/guard.py --tree` |
| Exit criteria met with evidence | 6 of 8 | 2026-08-31 | `8d86175` | `python tools/state/validate_state.py --check` |
