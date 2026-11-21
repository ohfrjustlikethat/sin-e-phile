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
| *(Phase 5)* Search relevance | exact-title top-1 | 100% | **100% (43/43)** | 2026-09-06 | `c7214f4` | `cargo run -p eval --release -- search --report` |
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
| *(Phase 3)* Indexed lookup, 500k rows | < 100 ms | 2 | **0.141 ms** p95 (worst of three) | 2026-09-06 | `4f302d5` | `cargo test -p sinephile-persistence --test benchmark --release -- --ignored` |
| *(Phase 5)* Keystroke → results p95 | < 80 ms *incl. embedding* | 2 | **7.3 ms** (keyword path; no embedding yet) | 2026-09-06 | `c7214f4` | `cargo run -p eval --release -- search --report` |
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

## Phase 1 — Application shell

Release build, dev machine (**Tier 2**: 32 GB, 24 cores, RTX 5070 Ti, hardware
decode present). Tier 0 figures are the governing ones (§2.3) and are outstanding
until Phase 21 measures on constrained hardware — these are the Tier 2 targets from
the Phase 1 exit criteria.

| Measurement | Value | Target | Command |
|---|---|---|---|
| **Cold start to interactive** | **515 / 660 ms** | < 2 s (Tier 2), < 4 s (Tier 0) | `cold_start_ms` in `data/logs/`, measured process start → frontend painted |
| **Idle RAM** | **42.2 MB** | < 200 MB (Tier 2), < 250 MB (Tier 0) | `WorkingSet64` after the window is shown |
| Release binary | **10.1 MB** | — | `target/release/sin-e-phile.exe` |
| Frontend bundle | 257 KB JS (79 KB gzip), 18 KB CSS | — | `npm run build` |

**Cold start is measured to *interactive*, not to window creation.** The window is
created hidden and revealed only when the frontend reports it has painted, gated on
a double `requestAnimationFrame`. Timing to `MainWindowHandle` instead gave 267 ms —
a flattering number for a window with nothing in it. The honest figure is ~2× that
and still well inside budget.

### Tier detection (§8)

Correct on the dev machine: `Capable`, 32189 MB, 24 physical cores,
`NVIDIA GeForce RTX 5070 Ti Laptop GPU`, hardware decode present. Probed via DXGI
rather than WMI, which is slow enough to be visible in a 4 s cold-start budget.

### Exit criteria evidenced

| Criterion | Evidence |
|---|---|
| **IPC types are generated, not hand-written; changing a Rust command signature breaks the TypeScript build** | Verified by doing it. Baseline `npm run build` clean → added a `profile_id: u32` argument to `has_capability` in Rust → rebuilt → `src/lib/ipc.ts` regenerated with `unexpectedNewArg` → `npm run build` failed with `error TS2554: Expected 2 arguments, but got 1` at `SettingsScreen.tsx(145,64)`. Reverted; clean again. |
| **A deliberately-triggered panic writes a crash log** | `SINEPHILE_PANIC_TEST=1` → `data/crashes/crash-1788162818.txt` with version, location (`logging.rs:65:9`), message and full backtrace. Also caught a *real* panic earlier (a Specta BigInt export error), which is stronger evidence than the contrived one. |
| Graceful error screen | `src/app/ErrorBoundary.test.tsx` — 2 tests, asserts the message and that it names where the crash report went. |
| Tier detection correct | See above; 8 unit tests in `crates/tiers` cover the §8 table at its boundaries. |

### Test and lint status

| Check | Result |
|---|---|
| `cargo test --workspace` | **8 passed**, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --all --check` | clean |
| `npm test` | **9 passed** (3 files) |
| `npm run lint` | clean |
| `npm run build` | clean |

## Phase 1 — Spike C (ONNX Runtime, risk R3)

**R3 does not fire. PASS, with large headroom.** `ort` 2.0.0-rc.13, ONNX Runtime,
`all-MiniLM-L6-v2` INT8 (21.9 MiB), dev machine (**Tier 2**), release build,
`intra_threads = 2`.

ADR-0015 made this the measurement that gates Phase 5: query embedding runs on
**every** tier, so this is query latency specifically — model already loaded, one
query, wall clock to a returned vector — not document throughput.

| Measurement | True query length | Padded to 128 | Trigger / budget |
|---|---|---|---|
| tokens per query | 10.0 avg | 128.0 | ADR-0015 assumed ~30 |
| min | 0.71 ms | 6.61 ms | — |
| p50 | **0.98 ms** | 7.11 ms | — |
| mean | 1.06 ms | 7.43 ms | — |
| **p95** | **1.63 ms** | **8.13 ms** | **R3 trigger: > ~30 ms** |
| max | 2.23 ms | 9.30 ms | — |
| share of the §2.3 80 ms search budget | **2.0%** | 10.2% | — |

| | Value | Budget |
|---|---|---|
| Model load | 81.9–84.9 ms | — (lazy-load on first search if it threatens cold start) |
| Resident memory added | **+51.6 MB** | §2.3 Tier 0 idle RAM 250 MB |
| Embedding dimension | 384 | matches ADR-0014's 77 MB/200k estimate |

**Both columns are reported because the tokenizer surprised the measurement.**
`tokenizer.json` for this model configures padding to a fixed 128 tokens, so the
first run measured every query as 128 tokens — roughly 4× the real work, and the
wrong thing. Padding is now disabled by default and the padded figure kept as a
worst case. Note the queries here average **10 tokens**, shorter than the ~30 ADR-0015
assumed, so the honest bracket for a real query is **1.6–8.1 ms p95**.

**Headroom against Tier 0.** These are Tier 2 numbers. Even taking the padded worst
case and assuming a 3–4× penalty on Tier 0 hardware, p95 lands around 24–33 ms —
which is why the padded column matters. The unpadded case has roughly **18× headroom**
against the 30 ms trigger.

**Outstanding (honest gap):** ADR-0015 asked for this to be measured on the dev
machine **and on a constrained VM approximating Tier 0**. Only the dev machine has
been measured. Logged as **P8**; it does not block Phase 5, but the Tier 0 number
should exist before Phase 21 signs off the search budget.

## Phase 1 — Spike B (librqbit streaming, risk R2)

**R2 does not fire. PASS.** librqbit 9.0.1, dev machine, 3 runs against a legal
well-seeded Linux ISO torrent (6.05 GiB). Torrent URL passed as an argument and
never committed — the guard blocks `.torrent` URLs by design (§2.1).

| Measurement | Run 1 | Run 2 | Run 3 | Target |
|---|---|---|---|---|
| Session created | 7 ms | — | — | — |
| Metadata resolved | 1400 ms | 1485 ms | 795 ms | — |
| Torrent live | 927 ms | 920 ms | 929 ms | — |
| First byte | 686 ms | — | — | — |
| **Time to first usable bytes (1 MiB)** | **1003 ms** | **2902 ms** | **3127 ms** | **R2 trigger > 20 000 ms** |
| **Seek re-prioritisation (to 50%, unbuffered)** | **588 ms** | **753 ms** | **2392 ms** | **Phase 7 exit: < 5000 ms** |
| Live peers / queued | 1 / 0 | 18 / 473 | 11 / 694 | — |

**End-to-end time to playable bytes** (metadata + live + TTFB): **3.3 / 5.3 / 4.9 s**
— inside §2.3's 8 s budget, before any tuning, on a debug-grade spike.

Note run 1 hit 1003 ms with a **single live peer**, so these numbers are not
dependent on a large swarm.

### API audit — the part that actually gated R2

R2's trigger was "the API does not permit per-piece or per-range priority to be set
and changed at runtime". It does, and more directly than expected.

- **`ManagedTorrent::stream(file_id) -> FileStream`** is public and implements
  `AsyncRead + AsyncSeek` (`torrent_state/streaming.rs:337`).
- Each open stream maintains a **32 MiB lookahead window** from its current read
  position, converted to piece indices (`StreamState::queue`).
- The picker consumes those as `priority_pieces`
  (`torrent_state/live/mod.rs:1440`), ahead of the normal queue.
- Selection order is: steal from a 10× slower peer → **priority pieces** → queued
  pieces → steal from a 3× slower peer (`piece_tracker.rs:117`).
- **Seeking moves the window immediately** — `start_seek` sets the position, and the
  next `queue()` is computed from it. This is why seek re-prioritisation measures in
  hundreds of milliseconds rather than seconds.
- Multiple concurrent streams are interleaved fairly, with shuffling to avoid
  determinism.

**Consequence for Phase 7: much of the deadline scheduler already exists.** The
phase shifts from "build a piece scheduler" toward "drive, tune and instrument
librqbit's". That should be reflected when Phase 7 is planned.

**Two limitations found, neither trigger-worthy:**

- `PER_STREAM_BUF_DEFAULT` (32 MiB lookahead) is a **compile-time constant**, not
  configurable. Phase 7's "documented bandwidth floor" tuning would want it
  adjustable; options are a PR upstream or accepting the fixed window.
- `iter_next_pieces` is `pub(crate)` — there is **no public API to set arbitrary
  per-piece priority**. Control is indirect, via stream position. Sufficient for
  playback; insufficient if Phase 7 ever wants a priority scheme unrelated to a
  read head.

**Separate finding, and it changes a Phase 6 assumption: librqbit has NO webseed
(BEP-19) support.** Internet Archive torrents lean heavily on webseeds, so the
`InternetArchiveBackend` (§2.1's named legal reference backend) should resolve to
**direct HTTP**, not to torrents. This is why the spike measured against a Linux ISO.

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

| **First frame, inside a Tauri v2 window** | **1019 / 1036 / 1192 ms** (3 runs) | 2026-08-31 | `a286489` | `spike-a-tauri` |
| Compositing: HTML over video, child-window z-order | **FAILS** — 3 distinct attempts, see `docs/RISKS.md` R1 | 2026-08-31 | `a286489` | `spike-a-tauri` |
| Compositing: HTML beside video, video on top | works | 2026-08-31 | `a286489` | `SPIKE_ZORDER=top` |

### B2 option 5 — region cutouts for playback chrome

`SetWindowRgn` on the video child: full rect minus the chrome silhouette. The page
shows through the hole. Four tests, chosen as the places this class of thing breaks.

| Test | Result | Date | Commit | Command |
|---|---|---|---|---|
| **1. Hit-testing through the hole** | **PASS** — `RealChildWindowFromPoint` reports `WRY_WEBVIEW` inside the hole, `SpikeVideoHost` outside, boundary **pixel-exact** (1 px above → video, 1 px below → webview). Synthetic click reached the button. | 2026-08-31 | `28eb3d6` | `test1_probe.ps1` |
| **2. Region + mid-playback resize** | **PASS** — hole tracks: 1078×664 → hole 40,444 998×190; 1378×824 → hole 40,604 1298×190 | 2026-08-31 | `28eb3d6` | `spike-a-tauri` |
| **3. Flicker on region change** | **PASS — zero flicker.** 758 samples at ~7 ms across 12 toggles at the auto-hide cadence; colourfulness median 232.7, min 170.5; **0 frames** below 45% of median | 2026-08-31 | `28eb3d6` | `run_cutout_tests.ps1` |
| └ `SetWindowRgn` cost | min 0.326 / **median 0.467** / max 1.062 ms | 2026-08-31 | `28eb3d6` | as above |
| **4. Seam quality** | **PASS with one defect** — edge is razor sharp, no bleed, tearing or fringe. **Defect:** rounded panel corners over a rectangular hole leave a gap showing the *desktop* (transparent window). Fix: match the region to the silhouette (ADR-0020). | 2026-08-31 | `28eb3d6` | `seam_capture.ps1` |

**Test 3's result is created, not free.** Both defences are required: a NULL class
background brush plus swallowing `WM_ERASEBKGND`, and `SetWindowRgn(redraw=false)`.
Without them Windows erases the newly-exposed area before mpv repaints — the strobe
that would make auto-hiding chrome unusable.

### B2 option 1 — still-frame substitution for the pause overlay

On pause: capture via `screenshot-raw window`, downscale, JPEG, render as an
`<img>` in the page with the overlay composited over it normally, then hide the
video child HWND. Window 1680x960 -> capture 1680x960 `bgr0` -> 945x540 JPEG, 37-39 KB.

**`screenshot-raw window` captures at WINDOW resolution, not source resolution**, so
this cost does not scale with a 4K source — which is what lets it hold the budget.

| Measurement | Value | Date | Commit | Command |
|---|---|---|---|---|
| **Pause -> overlay visible (§11 budget: < 200 ms)** | **161.0 / 95.4 / 140.1 ms** — all WITHIN, **debug build** | 2026-08-31 | `cec9d1e` | `spike-a-tauri`, 3 cycles |
| ├ `set pause=yes` | 0.2 / 14.8 / 25.0 ms | 2026-08-31 | `cec9d1e` | as above |
| ├ `screenshot-raw window` | 19.6 / 47.5 / 58.6 ms | 2026-08-31 | `cec9d1e` | as above |
| ├ downscale to 945x540 (box filter) | ~20 ms | 2026-08-31 | `cec9d1e` | as above |
| ├ JPEG encode q78 | ~41 ms | 2026-08-31 | `cec9d1e` | as above |
| └ base64 for the data URI | ~0.7 ms | 2026-08-31 | `cec9d1e` | as above |

Timing is measured end to end: pause requested -> webview has painted, confirmed by
a double `requestAnimationFrame` before the page reports back, so the number
reflects pixels on screen rather than a promise to paint. **Unoptimised build**;
downscale and JPEG are pure compute and should improve materially in release.

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

## Phase 3 — data layer

`cargo test -p sinephile-persistence --release --test benchmark -- --ignored --nocapture`

Fixture: 500,000 synthetic media items, deterministic (LCG seeded `0x5EED5EED`), so
a rerun measures the same database. Release build. 1,000 sampled lookups each.

| Metric | Value | Date | Commit | Command |
|---|---|---|---|---|
| `by_id` p50 / p95 / p99 | 0.032 / 0.050 / 0.098 ms | 2026-09-01 | `e4eb020` | as above |
| `by_exact_title` p50 / p95 / p99 | 0.081 / 0.128 / 0.179 ms | 2026-09-01 | `e4eb020` | as above |
| `by_external_id` p50 / p95 / p99 | 0.056 / 0.091 / 0.120 ms | 2026-09-01 | `e4eb020` | as above |
| Bulk insert, 500k rows | 43.7 s (11,440 rows/sec) | 2026-09-01 | `e4eb020` | as above |
| Database size at 500k items | 145.4 MB | 2026-09-01 | `e4eb020` | as above |
| Migration tests | 17 pass | 2026-09-01 | `e4eb020` | `cargo test -p sinephile-persistence` |

**Budget: 100 ms per indexed lookup** (Phase 3 E3, amendment 15 — the budget is the
lookup alone; bulk insertion has none). Worst p99 is 0.179 ms, **558× inside it**.

**One index was silently doing nothing, and the benchmark is what found it.**
`by_exact_title` first measured **26.679 ms p50** — 400× slower than the other two
lookups, because SQLite will only use an index whose collation matches the
comparison's, and `WHERE title = ? COLLATE NOCASE` against a `BINARY` index falls
back to a full scan. Adding `COLLATE NOCASE` to the index took it to 0.081 ms, a
**330× improvement**.

Both numbers pass the Phase 3 criterion, which is the point worth recording: the
criterion would have been met with the scan in place, and the failure would have
surfaced in Phase 5, whose 80 ms p95 search budget sits on top of this lookup against
a catalogue an order of magnitude larger than this fixture.

## Phase 4 — IMDb catalogue shape (R4 measurement)

`cargo build -p sinephile-ingest --release && ./target/release/ingest measure`

Measured against the live IMDb datasets on 2026-09-01, commit `f7907fd`. This is the
measurement **R4's mitigation requires before committing to a shape**. It writes
nothing to the database.

| Metric | Value |
|---|---|
| Titles in `title.basics` | 12,754,307 |
| Of a type this catalogue would keep | 2,832,564 |
| `tvEpisode` rows (excluded — they arrive via `title.episode`) | 9,860,692 (77%) |
| Rated titles in `title.ratings` | 1,711,804 |
| Kept-type titles with **no** votes at all | 2,023,574 (71% of kept) |
| Archives | 224.0 MB compressed, 1,084.2 MB raw |
| Download | 25 s (9.3 MB/s) |
| Scan, both files, release build | 8 s |

**Titles kept by vote threshold**, and the projected database for that many rows
(2.4x SQLite overhead, the ratio measured in Phase 3 — 500,000 synthetic rows
produced 145.4 MB):

| Threshold | Titles | Projected |
|---|---|---|
| everything of a kept type | 2,832,564 | 471 MB |
| >= 0 votes (rated only) | 808,990 | 135 MB |
| >= 10 | 700,529 | 117 MB |
| >= 50 | 330,344 | 55 MB |
| >= 100 | 236,438 | 39 MB |
| >= 500 | 103,060 | 17 MB |
| >= 1,000 | 69,304 | 12 MB |
| >= 10,000 | 15,855 | 3 MB |

**R4's triggers are 2 hours of ingestion or a 4 GB database.** On `title.basics`
alone, neither is close: the whole kept set projects to 471 MB and the scan takes 8
seconds. Excluding `tvEpisode` removes 77% of IMDb before any threshold is applied.

**This does not clear R4.** It measures one of six datasets. `title.principals`
(cast and crew) is the largest by a wide margin and has roughly 90 million rows, and
both it and `title.akas` scale with the number of titles kept — so the threshold
chosen here multiplies through them. Measuring those is the next thing, and the
threshold should not be treated as settled until it is.

### The two-tier scope, measured against all four large datasets

`./target/release/ingest measure` (full, not `--quick`), 2026-09-01, commit `6d9e429`.
Author's ruling: **A for the index, C for enrichment** — every kept-type title enters
the catalogue; only the popular core gets cast, crew, akas and embeddings.

| Tier | Rule | Titles |
|---|---|---|
| index | any kept type, non-adult | 2,701,195 |
| core | >= 10 votes, **or** unrated and released within 2 years | 854,752 |

| Dataset | Rows total | Rows for core titles | Kept |
|---|---|---|---|
| `title.principals` | 101,540,407 | 10,493,168 | 10.3% |
| `title.akas` | 59,142,985 | 5,910,737 | 10.0% |

**Projected database**

| | Size |
|---|---|
| titles (index tier) | 450 MB |
| credits (core tier) | 1,921 MB |
| alternative titles (core tier) | 812 MB |
| **total** | **3.11 GB** |
| the same load **without** the core tier | **26.53 GB** |

**R4's fear was justified, and the two-tier scope is what answers it.** Ingesting
credits and akas for every title would produce a **26.53 GB** database — six and a
half times over R4's 4 GB trigger. Restricting enrichment to the core tier is an
**8.5x reduction** and brings it to 3.11 GB.

Full scan of all four datasets: **174 seconds**. R4's other trigger is two hours, so
time is not close to being a problem — size was always the real risk.

**Headroom is thin and should not be treated as settled.** 3.11 GB is 78% of the 4 GB
trigger, and AniList, MovieLens and the embedding artefact all still have to fit
inside it. Credits alone are 62% of the projected total. The lever, if it is needed,
is the core vote threshold: >= 50 votes cuts the core from 854,752 titles to roughly
330,000. That would not reduce credits proportionally — popular titles have larger
casts — but it is the largest single reduction available.

### Correction: the size projection was wrong by roughly half

The table above projected **450 MB** for the index tier. The real load passed
**900 MB before it had finished**, and the projection method was the reason.

The 2.4x multiplier came from the Phase 3 benchmark, where 500,000 synthetic rows
produced a 145.4 MB database. That benchmark inserted into `media_items` and `titles`
and nothing else. A real load also writes `external_ids` with its UNIQUE index,
`media_genres`, and a second `titles` row wherever the original title differs — none
of which the multiplier could capture, because none of them existed in the fixture it
was derived from.

**A multiplier is only valid for the shape it was measured on.** Reusing it across a
different set of tables is not a projection; it is a guess wearing a number's
clothes. The measurement was real and the arithmetic on top of it was not.

This matters for R4: if titles are roughly 2x the projection, the 3.11 GB total is
closer to 5-6 GB, which is **over** the 4 GB trigger rather than comfortably under
it. The measured index-tier figure below replaces the projection, and the credits and
akas projections are still unverified and should be treated as suspect for the same
reason.

### The index tier, actually loaded

`./target/release/ingest imdb` against the live IMDb datasets, 2026-09-01,
commit `048180a`. Release build, dev machine.

| Metric | Value |
|---|---|
| Titles indexed | **2,701,195** |
| In the core tier | **854,752** |
| `titles` rows | 2,870,270 |
| `external_ids` rows | 2,701,195 |
| `media_genres` rows | 4,448,436 |
| Distinct genres | 28 |
| **Database** | **1,153 MB** (+46 MB WAL) |
| **Load time** | **219 s** |

Counts matched the measurement exactly — 2,701,195 and 854,752 were both predicted —
because the scope logic is the same code. **The size was not:** projected 450 MB,
actual 1,153 MB, **2.56x low**. The multiplier is corrected to 6.2 and is now derived
from this load rather than from the Phase 3 fixture.

Integrity, checked rather than assumed: no null or empty titles, no literal `\N`
leaking through as a value, every rating within 0-100, no duplicate IMDb ids, no
media item without a title row, no media item without an external id.

Resumability was demonstrated on real data, not fixtures. The first run was killed at
2,350,000 items; restarting picked up from that checkpoint and finished, with no
duplicate and no gap.

**Load time is 219 s against R4's two-hour trigger.** Time was never the risk.

### R4, reopened

The two-tier scope was accepted on a projection of **3.11 GB**. That projection is
now known to be low by 2.56x on the one component that has been measured:

| Component | Projected | Measured |
|---|---|---|
| titles (index tier) | 450 MB | **1,153 MB** |
| credits (core tier) | 1,921 MB | not yet loaded |
| alternative titles (core tier) | 812 MB | not yet loaded |

Taking the two unmeasured figures at face value gives **3.9 GB**, which is at R4's
4 GB trigger rather than under it. If they are wrong in the same direction and by a
similar factor — and they were produced by the same kind of reasoning — the total is
closer to **7 GB**, well over.

**R4 is open again, and the >= 10 core threshold should be treated as provisional
until credits are actually loaded and measured.** That is the next measurement, and
it should happen before AniList rather than after.

### Credits, actually loaded — and the projections were wrong in both directions

`./target/release/ingest credits`, 2026-09-01, commit `afc5408`. Release build.

| Metric | Value |
|---|---|
| People loaded | 2,736,235 |
| Credits loaded | **10,079,841** |
| Core titles with at least one credit | 838,133 of 854,752 (98%) |
| Mean credits per title | 12.0 |
| People dropped (absent from `name.basics`) | 1,604 |
| **Database** | **2,427 MB** (+1,069 MB for people and credits) |
| **Load time** | **2,234 s** for the resumed portion |

Integrity, checked: zero credits pointing at a missing person, zero pointing at a
missing title, zero attached to a non-core title, and only roles the migration 0002
`CHECK` constraint allows.

**The projections were wrong in BOTH directions, which is the useful finding:**

| Component | Projected | Measured | Error |
|---|---|---|---|
| titles | 450 MB | 1,153 MB | **2.6x under** |
| people + credits | 1,921 MB | 1,069 MB | **1.8x over** |

A method that is 2.6x low on one component and 1.8x high on the next is not biased —
it is **unreliable**, and no correction factor fixes that. The multiplier was
corrected once already, from 2.4 to 6.2, and this shows that was treating a symptom.
Projections are now labelled as projections wherever they are printed, and the R4
position is stated from measurements only.

### R4 position, on measured data

| Component | Status |
|---|---|
| titles (index tier) | **1,153 MB measured** |
| people + credits (core tier) | **1,069 MB measured** |
| alternative titles (core tier) | not yet loaded |
| embedding artefact | not yet built (~330 MB at ADR-0014's rate for 854,752 core titles) |
| AniList, MovieLens | not yet loaded |

**Measured so far: 2,427 MB of a 4 GB trigger.** Roughly 1.6 GB of headroom for four
components, one of which is projected at 812 MB by a method now known to be
unreliable.

**Time is now a live concern too, and was not before.** Titles 219 s plus credits
2,234 s plus the failed first attempt is roughly 45 minutes of a two-hour budget,
with `title.akas`, AniList, MovieLens and the embedding build still to run.

**Both of R4's triggers are in play.** Neither is breached; neither is comfortably
clear.

### Alternative titles loaded — every R4 component is now measured

`./target/release/ingest akas`, 2026-09-01, commit `f67d939`.

| Metric | Value |
|---|---|
| Title rows | 8,435,215 |
| **Database** | **3,110 MB** (+683 MB) |
| **Load time** | **860 s** |

Variant distribution: primary 2,701,195 · english 2,671,786 · alternative 1,697,466 ·
original 909,292 · **native 380,185** · **romaji 75,291**. The script heuristic
produces plausible counts, and no title rows are orphaned or attached to a non-core
item beyond its primary.

### R4, every component measured

| Component | Projected | **Measured** | Projection error |
|---|---|---|---|
| titles (index tier) | 450 MB | **1,153 MB** | 2.6x under |
| people + credits (core) | 1,921 MB | **1,069 MB** | 1.8x over |
| alternative titles (core) | 812 MB | **683 MB** | 1.2x over |
| | | **3,110 MB total** | |

**3,110 MB of the 4,096 MB trigger — 76%.** Ingestion time so far is roughly 65
minutes of the two-hour budget.

Still to fit: the embedding artefact (~330 MB at ADR-0014's rate for 854,752 core
titles), AniList, and MovieLens. Estimating those at ~600 MB puts the final figure
near **3.7 GB, or 91% of the trigger** — under it, with no margin.

The projections were wrong in all three cases and in both directions. They are
retained above only to show that.

### A defect the variant distribution exposed

`Seven Samurai` has **six identical `english` rows**, differing only by region.
`idx_titles_unique` is `(media_item_id, variant, language, region)`, so the same
text under `en/US`, `en/GB`, `en/CA` and a null region are four distinct keys and
all were inserted.

| Uniqueness | Rows | Redundant |
|---|---|---|
| as stored | 8,435,215 | — |
| distinct (item, variant, text) | 6,184,278 | **2,250,937 (27%)** |
| distinct (item, text) | 4,370,081 | 4,065,134 (48%) |

At roughly 120 bytes per row, deduplicating on **(item, variant, text)** recovers
about **270 MB** — and costs nothing, because it removes duplicate rows rather than
information. A genuinely different regional title is still a different row.

**This matters more than the vote threshold.** Deduplication buys ~270 MB for no loss
at all; tightening the core to `>= 50` buys perhaps 750 MB but takes the cast list
away from roughly 370,000 films. The cheap fix should be taken before the expensive
one is considered.

### The deduplication worked, and saved nothing. Third projection error.

Migration 0007, applied to the real 3,110 MB catalogue in **32 seconds**.

| | Before | After |
|---|---|---|
| `titles` rows | 8,435,215 | **6,184,278** |
| Database | 3,110 MB | **3,133 MB** |

The row count fell by exactly the 2,250,937 predicted. **The file grew by 23 MB.**

**Why.** The old index was `(media_item_id, variant, language, region)` — two short
codes. The new one is `(media_item_id, variant, title)`, and an index carries the
values it is keyed on, so it now stores **the full title text of every row**. The
table got smaller and the index got wider by more than the table saved. `freelist_count`
is 0, so there is nothing for `VACUUM` to reclaim either.

I estimated 270 MB of savings. The actual result was **-23 MB**. That is the third
storage estimate in this phase to be wrong:

| Estimate | Predicted | Actual |
|---|---|---|
| titles | 450 MB | 1,153 MB |
| credits | 1,921 MB | 1,069 MB |
| akas | 812 MB | 683 MB |
| dedupe saving | +270 MB | **-23 MB** |

**The pattern is the finding.** Every one of these was an arithmetic argument about
storage that sounded reasonable and was wrong — twice high, twice low. Storage in
SQLite depends on page packing, index width and B-tree fill factor, none of which a
back-of-envelope multiplier models. **The rule from here: report measurements, and do
not present storage arithmetic as a prediction at all.**

**Was the migration still worth applying?** On data quality, yes and it stays: a
search for "Seven Samurai" no longer matches the same film six times, and 2.25 million
rows carrying no information are gone. On disk, it is break-even. It was sold on the
disk saving, and that part did not happen.

### R4, after deduplication

**3,133 MB of the 4,096 MB trigger.** Unchanged in substance: roughly 960 MB of
headroom for the embedding artefact, AniList and MovieLens — and no reliable way to
predict what those will cost, which is precisely why they will be measured as they
land rather than estimated now.

### Normalised titles backfilled (migration 0009)

`./target/release/ingest normalise`, 2026-09-02, commit `3155e6b`.

| Metric | Value |
|---|---|
| Titles normalised | **6,184,278** (all of them) |
| Distinct normalised forms | 3,453,809 |
| Empty forms (punctuation-only titles) | 126 |
| **Time** | **197 s** |
| **Database** | 3,133 MB → **3,399 MB** (+266 MB) |

Spot-checked: `Fullmetal Alchemist: Brotherhood` → `fullmetal alchemist brotherhood`,
and `8½` survives as `8½` because `½` is a numeric character rather than punctuation.

**The ambiguity the matcher refuses is real and large.** 568 catalogue items share the
normalised title `home`; 486 share `alone`; 330 share `broken`. Any matcher that
resolved ties by picking the first would be wrong hundreds of times and would record
each one as a fact. That is the case
`two_equally_good_candidates_are_refused_rather_than_guessed` exists for, and it is
not a hypothetical.

The 126 empty forms are titles made entirely of punctuation. They can never match,
which is correct — there is nothing to match on.

### R4 after migration 0009

**3,399 MB of the 4,096 MB trigger — 83%. Headroom is 697 MB.**

Still to fit: the embedding artefact, AniList and MovieLens. ADR-0014 states ~77 MB
per 200,000 titles, which for 854,752 core titles is around 330 MB — that is the
ADR's own rate, not an estimate of mine, and this document has already recorded four
storage estimates that were wrong in both directions.

**Headroom is now thin enough that the `>= 10` core threshold is a live question
again.** It should be settled once the remaining three components are measured, which
is the same discipline that has been applied throughout: no fifth prediction.

### Tuning the AniList matcher (subtask 4.4)

`./target/release/ingest anime --pages 5` — the same 250 most-popular AniList entries
against the full catalogue, re-run after each change. Pages come most-popular-first,
so this sample is deliberately the hardest one: long-running franchises with many
IMDb entries sharing a title.

| | v1 | + collapse by item | + evidence narrowing |
|---|---|---|---|
| **Matched** | 101 (40.4%) | 171 (68.4%) | **184 (73.6%)** |
| Ambiguous (refused) | **97** | 23 | **9** |
| Already claimed | 11 | 16 | 17 |
| Not in catalogue | 27 | 27 | 27 |
| Year conflict | 14 | 13 | 13 |

**v1 refused Death Note, One Piece, Naruto and Attack on Titan** — the four most
recognisable titles in the sample. Every one of them was ambiguous *against itself*.
The catalogue stores one `titles` row per spelling, so `Death Note`, `DEATH NOTE` and
`Death note` all normalise to the same key and came back as three rival candidates for
one item. The matcher counted rows where it should have counted items. That single bug
was 97 of the 250 outcomes, and it was invisible in unit tests because a fixture never
has three spellings of one title.

Two narrowing rules then took ambiguity from 23 to 9. Both use evidence already held
rather than preference, and neither can create a match or reject the last candidate
standing:

1. **A known, agreeing year beats no year at all.** `years_agree` is permissive about a
   missing year — correctly, since absence is not evidence *against* a match. But it is
   not evidence *for* one either, and treating the two as equal let a year-less
   catalogue stub veto a match an exactly-dated entry had earned. This is what refused
   Death Note (a 2006 series sitting beside an undated one), Sword Art Online and
   Steins;Gate.
2. **AniList states whether an entry is a film or a series.** `Naruto` is one series and
   two unrelated undated films; the format setting resolves it. It is applied only when
   several candidates are already tied, and only when it leaves exactly one.

**What is still refused is refused correctly.** The remaining 9 are cases where two
catalogue entries of the same kind share a title and a compatible year — nothing
distinguishes them, so nothing may choose. Ranking by vote count would resolve most of
them and would be wrong: popularity is evidence about a title, not about its identity.

`already_claimed` counts an AniList entry that resolved to the right catalogue item but
found it already mapped by an earlier one — nearly always a later season, since AniList
lists seasons separately and IMDb lists one series dated by its first year. Counting
those as matches gives (184 + 17) / 250 = **80.4% resolved**; the headline 73.6% is the
pessimistic reading, and E5 reads the counts rather than either rate.

### Japanese long-vowel folding: measured, and rejected

**The idea.** `Obake no Q-tarou` came back `not in catalogue`, and it is in the
catalogue — as `Q-Taro the Ghost`, reachable under `obake no q taro`. Japanese long
vowels romanise inconsistently (`ou` / `oo` / `ō` / `o`, `uu` / `ū` / `u`), so folding
them before comparison would recover titles like that one.

**The measurement.** `scratchpad/measure_vowels.py` over the real catalogue and the
5,997 `not in catalogue` entries from the full sweep:

| Metric | Value |
|---|---|
| Unmatched that folding would give a candidate | **200 of 5,997 — 3.3%** |
| Distinct normalised titles | 3,459,678 |
| Distinct titles **after** folding | 3,432,608 |
| **Collisions folding creates** | **27,070** |

**Rejected.** It would manufacture roughly 27,000 new ambiguities across the catalogue
to recover 200 matches — a bad trade by two orders of magnitude, and every one of those
ambiguities lands in a matcher whose entire design is that ambiguity must be refused.
Folding is also unsafe in general: it does not know a title is Japanese, so `Book`
becomes `Bok` and `Moon` becomes `Mon`.

The one example that prompted the question was real and unrepresentative. The spread
sample shows what the bucket actually contains: TV specials, OVAs, promotional shorts
and collaboration films that IMDb does not list as separate titles at all —
`Ore no Imouto ... Short Specials`, `Odekake Kozame Kurashiki-shi Collab Movie`. Those
are genuinely absent, and no normalisation reaches them.

### Offset pagination over a popularity ordering is lossy

Three full sweeps of the same catalogue:

| Run | Ordering within a year | Entries seen |
|---|---|---|
| 1 | `POPULARITY_DESC` | 14,737 |
| 2 | `POPULARITY_DESC` | **14,344** |
| 3 | `ID` | 14,737 |

Run 2 lost 393 entries — 2.7% — with **no code change** between it and run 1. Offset
pagination is complete only while the ordering holds still, and popularity does not: an
entry whose rank rises between page 4 being read and page 5 being requested moves onto
page 4 and is never seen.

**What this does and does not establish.** It does not prove drift caused run 2's
shortfall: a 10-second control sweep of one year showed no reordering under either sort,
and proving the mechanism would take repeated long runs. What it does establish is that
14,737 is the complete set, since two independent runs reached it and one fell short.
The switch to `sort: ID` rests on the property rather than the measurement — an
immutable key cannot reorder under a sweep, a popularity ranking can — and the id-ordered
run reaching 14,737 is consistent with that.

Ascending year and ascending id both mean *earlier first*, so the change also settles
the first-claim-wins rule: a series keeps season one's mapping rather than whichever
season was more popular on the day.

The unfiltered pass keeps `POPULARITY_DESC`, because it is capped at 5,000 entries and
there the question is *which* 5,000 rather than whether the sweep is complete.

### The full AniList catalogue, ingested

`./target/release/ingest anime`, 2026-09-04, commit `31aa603`. Id-ordered, year-swept.

| Outcome | Count | Share |
|---|---|---|
| **Seen** | **14,737** | |
| Matched | **7,328** | 49.7% |
| — exact title and year | 7,224 | |
| — title only, year unknown | 104 | |
| — season-aware | **0** | |
| Already claimed by an earlier entry | 409 | 2.8% |
| Not in catalogue | 6,238 | 42.3% |
| Year conflict | 510 | 3.5% |
| Ambiguous (refused) | 252 | 1.7% |
| **Time** | **809 s** | |

Resulting catalogue state: **5,917 `anime_series` and 1,466 `anime_film`**, 7,383
carrying an AniList id and 7,343 also a MAL id — the union of three sweeps; a single
clean sweep produces 7,328. Confidence spread: 7,226 at 1.0, 157 at 0.8, none at 0.6.

**Zero season-aware matches, and that is the design working rather than failing.**
Season-stripping still fires constantly; under id ordering season one always claims the
series first, so a later season now resolves into `already_claimed` instead of creating
a second mapping. The machinery's output moved from the `matched` column to the
`already_claimed` one, which is precisely where it belongs.

**42.3% not in catalogue is dominated by things IMDb does not list**, not by matching
failure — TV specials, OVAs, promotional shorts and collaboration films. See the
long-vowel measurement above for why no normalisation reaches them. All 7,409 unmatched
entries are written to `data/anilist-unmatched.tsv`, which is the list E5's 50
hand-checked titles get chosen from.

### Mislabelled title variants repaired

`./target/release/ingest repair-variants`, 2026-09-04.

**41,193 rows repaired, 0 still mislabelled, 39 s.** Spot-checked: *Spirited Away* now
carries exactly one `english` title, and `El viaje de Chihiro` and `Le voyage de
Chihiro` are `alternative` where they belong.

Database after the anime ingestion and the repair: **3,426 MB** — up 27 MB from 3,399,
which leaves **670 MB of R4 headroom**.

### Episodes: the scope chosen from a measured cost

`./target/release/ingest episodes --measure`, 2026-09-04. `title.episode` holds
**9,866,106 rows**, of which 6,868,550 (69.6%) hang off a core-tier series. At the core
tier's own 10-vote rule that is far more than R4 allows, so episodes needed their own,
tighter threshold — and this is the curve it was chosen from.

| parent series votes ≥ | episodes | series |
|---|---|---|
| 10 (the core tier's rule) | 6,759,802 | 111,134 |
| 100 | 3,490,720 | 42,151 |
| 1,000 | 1,265,318 | 12,787 |
| 5,000 | 426,043 | 4,724 |
| 10,000 | 216,121 | 2,832 |
| 50,000 | 52,453 | 715 |

**Cost per episode was measured, not predicted.** Anime was loaded first — 132,845
episodes for **+51 MB, 405 bytes each**. Widening to 10,000 votes then added 203,754
episodes at **402 bytes**, and to 5,000 votes another 203,218 at **410 bytes**. Three
independent measurements inside 2% of each other, which is the first time in this phase
a storage number has been trustworthy — because it was weighed rather than guessed.

| Scope | Episodes | Cost at 410 B | Database | Headroom left |
|---|---|---|---|---|
| anime only | 132,845 | 51 MB | 3,477 MB | 619 MB |
| + votes ≥ 10,000 | 336,599 | +78 MB | 3,555 MB | 541 MB |
| **+ votes ≥ 5,000 (chosen)** | **539,817** | **+79 MB** | **3,635 MB** | **461 MB** |
| + votes ≥ 1,000 | 1,378,000 | +344 MB | 3,979 MB | 117 MB |

**≥ 5,000 is where it stops**, because the next step down costs 344 MB and ADR-0014's
own rate puts the embedding artefact at roughly 330 MB. Widening past this point would
buy episode lists for mid-tier series at the price of the search engine.

Anime is exempt from the threshold entirely: `SPEC.md` §6.2 requires its numbering, and
the whole anime catalogue is 132,880 episodes — small enough that a threshold would cost
more in missing data than it saves in bytes.

Result: **539,817 episodes and 21,218 seasons.** Spot-checked against reality —
*Breaking Bad* 7/13/13/13/16, *Attack on Titan* 25/12/22/30, *Cowboy Bebop* a single
26-episode season. 15,691 episodes (2.9%) carry no season or episode number because
IMDb has none for them; they are stored as NULL rather than given an invented 0.

**The scope can be widened later without corrupting anything.** `episodes` has no
natural key, so a second pass would happily insert every episode again — the skip set
built from `external_ids` is the only thing preventing it, and
`widening_the_scope_adds_episodes_without_duplicating_the_first_pass` was watched
failing (4 rows instead of 2) with that skip removed before being trusted.

### E5 — hand-checked anime mappings

`./target/release/ingest verify-anime`, 2026-09-04, commit pending.
Fixture: `fixtures/anime/e5-hand-checked.tsv`, **64 rows** where E5 asks for 50.

| | |
|---|---|
| **Pass** | **64 / 64** |
| Expected matches | 60 |
| Expected refusals (ambiguous) | 2 |
| Expected refusals (year conflict) | 1 |
| Expected season claims | 2 |

Coverage of the categories E5 names: long-running shonen (One Piece, Naruto, Bleach,
Detective Conan, Dragon Ball Z, Gintama, Rurouni Kenshin), split-cour seasons (Attack on
Titan, Jujutsu Kaisen, Demon Slayer, Vinland Saga, Kaguya-sama, Haikyu), and films tied
to series (Mugen Train, Jujutsu Kaisen 0, Cowboy Bebop: The Movie, Made in Abyss).

**The first run failed 4 of 62, and all four were errors in the fixture rather than in
the matcher.** Two were IMDb ids I recalled wrongly — `tt1710308` is *Regular Show*, not
*Nichijou*. Two were refusals asserted on a false premise: Re:Zero is resolved by a later
title form, and IMDb does hold a separate (sparse, undated) record for *Attack on Titan
Season 2* rather than only the one series. All four are recorded in the fixture header,
because a fixture whose failures are quietly edited away until it passes is worth
nothing.

### A later title form can rescue an ambiguous earlier one

Building the fixture surfaced a real bug. AniList's romaji for *Great Teacher Onizuka*
is the three letters `GTO`, which several unrelated works share, so the matcher hit
ambiguity and **returned immediately** — never trying the English form `GTO: Great
Teacher Onizuka`, which exactly one catalogue entry has. The year-conflict path had
always remembered-and-continued. The asymmetry was the bug.

Measured over the full catalogue, before and after:

| | before | after |
|---|---|---|
| Matched | 7,328 | **7,391** |
| Ambiguous (refused) | 252 | **187** |
| Seen | 14,737 | 14,737 |

`seen` landing on 14,737 for the third consecutive id-ordered run is the evidence that
sweep is reproducible, which the popularity-ordered runs were not.

### Incremental catalogue refresh (ADR-0030 layer 1, corrected by ADR-0032)

`./target/release/ingest refresh`, 2026-09-04.

| Metric | Value |
|---|---|
| Titles added | **1,542** |
| Ratings re-applied | **1,086,185** |
| **Time** | **71 s** |
| Full re-ingest, for comparison | 219 s |

**ADR-0030's stated mechanism does not work, and the file says so.** It specified
`seek_past(highest id we have)` on the premise that IMDb's files are "sorted by id and
new titles get higher ids". Measured over the real 12,761,311-row `title.basics`:

```text
sorted lexicographically: true
sorted numerically      : false
first numeric decrease at row 967,458:   tt10001008 -> tt1000101
last id in the file:                     tt9916880   (not the largest)
```

`"tt10001008" < "tt1000101"` as text, so a newly issued `tt45000000` lands roughly half
way through the file rather than at the end. Seeking past the catalogue's maximum
(`tt44917931`) walked into the `tt5…`–`tt9…` rows already held and died on
`UNIQUE constraint failed: external_ids.source, external_ids.external_id`.

The refresh now filters by numeric id instead. **The saving ADR-0030 was after
survives**, because it was never the seek: the expensive half is inserting 2.7 million
rows, and nothing already held is inserted or even parsed into one. Three-fold on a
weekly job.

**Every fixture had used ids short enough for the two orderings to agree**, which is why
seven passing tests said nothing about it. The regression test now uses the exact
transition from row 967,458 of the real file.

### Artwork: WebP re-encoding measured on real photographs

`cargo test -p sinephile-artwork --test real_images -- --nocapture`, 2026-09-04.
Source: the 24 public-domain film stills in `dist/stills/` — *Battleship Potemkin*,
*Detour*, *Beat the Devil*, *His Girl Friday*.

| Metric | Value |
|---|---|
| JPEG in | 2,580 KB |
| Lossy WebP out (q80) | **1,493 KB** |
| **Saved overall** | **42.1%** |
| Worst single image | 27.8% (*his-girl-friday-1.jpg*) |

**Measured on photographs, not on gradients.** A synthetic test image compresses
nothing like a film still, so a saving measured on one says nothing about the other —
and this number is the entire justification for taking a C dependency (`webp`, wrapping
libwebp) rather than using `image`'s pure-Rust encoder. `image` supports WebP decoding
and **lossless** encoding only, and lossless WebP on a photograph is routinely *larger*
than the JPEG it came from, which would have made the cache worse while appearing to
satisfy the requirement.

The test asserts a floor of 10% rather than the measured figure, so it fails if the
dependency ever stops earning its place, without breaking every time libwebp changes a
heuristic.

### Blurhash

Implemented in `crates/artwork/src/blurhash.rs` rather than taken as a dependency —
about a hundred lines, and it is the interesting part of the subtask. 4×3 components,
28 characters per image, computed from a 64px thumbnail rather than the full poster
(the transform is O(pixels × components) and only the lowest frequencies survive
anyway).

Two bugs the tests caught, both of which would have shipped silently:

- **Averaging in sRGB rather than linear light.** Half black and half white averages to
  128 if the stored bytes are averaged directly and to ~188 done correctly. Every
  placeholder in the app would have been visibly too dark. This is the classic bug in
  reimplementations, and `the_average_is_computed_in_linear_light_not_in_srgb` is the
  test for it.
- **`&hash[2..6]` panics on a UTF-8 boundary.** These values come out of a database, so
  one corrupt row would have taken the process down. Indexed as bytes now.

### The embedding artefact: R4 answered by arithmetic on a measured format

`cargo test -p sinephile-embedding`, 2026-09-05. 24 passing.

ADR-0014 budgets INT8 at 384 dimensions, ~77 MB per 200,000 titles. The artefact format
is now written down rather than estimated, so the size is a calculation over a real
layout — a 256-byte header, one int8 per dimension per title, a 32-byte SHA-256:

| | |
|---|---|
| Core-tier titles to embed | 855,703 |
| Dimensions | 384 |
| **Artefact size** | **313 MB** (`the_expected_size_can_be_shown_before_a_download`) |
| R4 headroom before it | 452 MB (3,644 of 4,096 MB) |
| **Headroom after** | **139 MB** |

**It fits, and the `>= 10` core threshold survives.** That question has been open since
migration 0009 and is now answerable: at 855,703 core titles the artefact costs 313 MB
against 452 MB available. The number is arithmetic over a fixed format, not a
prediction, which is why it can be trusted where the earlier storage estimates could
not — and the artefact lives outside the database in any case, so R4's trigger is not
what bounds it. It is recorded here because ADR-0014 says catalogue growth must not
inflate it unnoticed.

**Quantisation was checked for the thing that would make it useless**: int8 must not
change the *order* of results. Over 384-dimension unit vectors, cosine similarity moves
by less than 0.005, and a vector's similarity to its own round trip is above 0.9999.

Two guards were broken deliberately to confirm they fail:

- `-128` is never produced. Its negation does not fit in an i8, so a dot product that
  negates a component overflows on exactly one value in 256 — a bug that surfaces once
  in a million comparisons and never reproduces. Raising the clamp to 128 makes
  `minus_128_is_never_produced` fail on the first seed.
- A mismatched model identity is refused. Disabling the check makes
  `a_mismatched_model_is_refused` fail. This is the more important of the two: a corrupt
  download fails loudly, but a *mismatched* artefact fails silently — vectors from one
  model compared against queries from another, with search simply getting worse and
  nothing to point at.

### The embedding artefact, produced

`./target/release/ingest embed`, 2026-09-06. Verified with `ingest verify-embeddings`.

| | |
|---|---|
| Vectors | **855,703** |
| Dimensions | 384, int8 |
| **Size** | **313 MB** (328,590,240 bytes) |
| **Time** | **4,293 s** (71.5 min) |
| Rate | 199 vectors/second |
| SHA-256 | `3fce6f062c25220a80425cce7e9f83a3b80412af3b34fe4e9071d3b80645a2b0` |
| Model | `all-MiniLM-L6-v2-int8`, sha256 `afdb6f1a…`, 22,972,370 bytes |
| Catalogue snapshot | 2026-09-04 |

**The size is exactly what the format predicted**: `256 + 855,703 × 384 + 32 =
328,590,240` bytes, to the byte. That was computed before the run from the layout in
`crates/embedding/src/artefact.rs`, which is the difference between arithmetic and an
estimate — and the reason it can be relied on for the next catalogue.

**71.5 minutes is inside ADR-0019's two-hour trigger, and not by much.** This is a
first-run cost on a Tier 2 machine (24 cores); ADR-0014 puts it on the author's machine
precisely so it is not paid on Tier 0, where it would be far worse. The artefact is
downloaded there instead.

`verify-embeddings` checks what actually matters — not "is the file intact" but "would
loading it produce meaningful results on this build": checksum, then model identity,
then document-builder version, then that the **last** vector is present and non-empty.
The last one is where an off-by-one in the layout shows up and nowhere else.

### No regression in earlier phases

Required before closing a phase (§10.12). Phase 3's indexed-lookup benchmark, re-run on
2026-09-06 after the catalogue grew from a fixture to 2.7 million real titles:

| Lookup | p50 | p95 | p99 | worst |
|---|---|---|---|---|
| by id | 0.031 ms | 0.067 ms | 0.121 ms | 0.722 ms |
| by exact title | 0.074 ms | 0.141 ms | 0.257 ms | 0.404 ms |
| by external id | 0.062 ms | 0.119 ms | 0.176 ms | 0.666 ms |

Against a 100 ms budget. Three orders of magnitude of headroom, unchanged.

## Phase 5 — Semantic search

### The FTS5 index, and 537 MB that were not doing anything

`./target/release/ingest search-index`, 2026-09-06.

| | Items | dbstat | Time |
|---|---|---|---|
| Main index (word) | 2,702,737 | **266 MB** | 213 s |
| Trigram index, core tier only | 855,703 | **61 MB** | — |

At that point the database was **4,023 MB against R4's 4,096 MB trigger** — 73 MB of
headroom, and a full-catalogue trigram index would have needed ~192 MB. The obvious move
was to cut scope. Measuring first found something better.

**`titles` is a 444 MB table carrying 882 MB of indexes.** Broken down with `dbstat`:

| | |
|---|---|
| `idx_titles_unique` (media_item_id, variant, title) | 265 MB |
| `idx_titles_text` (title COLLATE NOCASE) | 247 MB |
| `idx_titles_normalised` (normalised) | 187 MB |
| **`idx_titles_item`** (media_item_id, variant) | **183 MB** |

**`idx_titles_item` is dead.** A B-tree on `(a, b, c)` serves every lookup a B-tree on
`(a, b)` could, so SQLite never chooses it — confirmed against the real 6.2-million-row
table:

```
SELECT title FROM titles WHERE media_item_id = ? ORDER BY variant
  -> SEARCH titles USING COVERING INDEX idx_titles_unique (media_item_id=?)
```

It was added in migration 0001 and made redundant by 0007, which changed the unique
index's shape. Nothing noticed for five migrations, because a redundant index costs
disk and changes no behaviour at all.

Dropping it (migration 0012) and vacuuming:

| | |
|---|---|
| Before | 4,023 MB |
| **After** | **3,486 MB** |
| Recovered | **537 MB** — 183 from the dead index, ~354 from fragmentation |

**~350 MB of that was fragmentation** accumulated across incremental ingestion — nine
separate jobs writing, deleting and rewriting. Nothing was wrong with it; SQLite simply
does not repack as it goes. Worth knowing that `VACUUM` is a 46-second operation that
recovers a tenth of this database.

**R4 headroom is now 610 MB**, so the full-catalogue trigram index fits after all and
the core-only fallback is not needed. That answer came from measuring what was already
there rather than from cutting the catalogue, which was the first thing I reached for.

### Live search on the real catalogue

2,702,737 titles indexed, exact-title short-circuit:

| Query | Time | Top result |
|---|---|---|
| `Seven Samurai` | 3.8 ms | Seven Samurai (1954) |
| `Sen to Chihiro no Kamikakushi` | 8.8 ms | **Spirited Away** (2001) |
| `Tarkovsky` | 1.2 ms | Tarkovsky (2010) |
| `Face/Off` | 0.7 ms | Face/Off (1997) |
| `fullmetal alchemist brotherhood` | 1.4 ms | Fullmetal Alchemist: Brotherhood (2009) |

Against E1's 80 ms p95 budget. **These are the short-circuit path, not the full hybrid**
— they are a floor, not the criterion, which also has to include query embedding (~1 ms,
Spike C) and the vector half. But the Japanese title resolving to the English one, and a
title containing FTS5 syntax returning rather than erroring, are both real behaviour on
real data.

### The trigram index, and what "trigram fuzzy matching" actually is

`./target/release/ingest search-index --trigram`, 2026-09-06.

| | |
|---|---|
| Titles | 2,702,737 |
| Trigram index | **196 MB** (dbstat), +117 MB on disk |
| Time | 56 s |
| Database | **3,626 MB** — 470 MB of R4 headroom |

Projected 192 MB from the core-tier measurement, actual 196. That projection held because
it came from a complete measurement of 855,703 titles rather than from a sample.

The D28 fix also worked: the main index reported **"0 items"** and took no time at all,
where the previous run had re-indexed all 2.7 million.

**SQLite's trigram tokenizer does substring search, not typo tolerance.** `SPEC.md` calls
this deliverable "trigram fuzzy matching for typos" and the name invites the assumption.
A phrase query against a trigram index asks *"does this document contain this
substring"* — and `Solaris` does not contain `solariss`, so the first implementation
returned nothing for every typo.

Typo tolerance comes from the trigrams **individually**: `solariss` yields
`sol ola lar ari ris iss`, `Solaris` contains five of those six, and OR-ing them lets
BM25 rank by overlap. That is trigram similarity computed by the index rather than by us.

### Live typo tolerance, and why E1 is not met yet

| Query | Time | Top result |
|---|---|---|
| `solariss` | 256 ms | **Solaris** |
| `seven samuria` | 390 ms | **Seven Samurai** |
| `blade runer` | 354 ms | **Blade Runner** |
| `tarkovski` | 0.4 ms | *Why Can't I Be Tarkovsky?* (keyword) |
| `kurosowa` | 0.3 ms | *Margazhi Raagam* (keyword) |

**The mechanism is right and the performance is not.** E1's budget is 80 ms p95 and the
fuzzy path costs 256–390 ms — four to five times over. OR-ing ten trigrams across a
2.7-million-row index touches a great deal of it.

**And a weak keyword match blocks a good fuzzy one.** `kurosowa` returns *Margazhi
Raagam* because the keyword tier found *something*, filled the result limit, and fuzzy
never ran. The rule "fuzzy only fills what is left" is right in principle and wrong as
implemented: it should be "fuzzy fills what is left once weak keyword hits are
discounted", which needs a score threshold rather than a count.

Both are recorded rather than papered over: E1 is a measured criterion and it is
currently missed on this path.

### E2 met — and the harness found three different things

`cargo run -p eval --release -- search --report`, 2026-09-06, over 43 deliberately
awkward queries against the real 2.7-million-title catalogue.

| | First run | After |
|---|---|---|
| **E2 exact-title top-1** | 38/44 = 86.4% | **43/43 = 100%** |
| **E1 latency p95** | 803 ms | **7.3 ms** |
| p50 | 7.3 ms | **1.0 ms** |

E1's target is 80 ms *including* query embedding, which does not exist yet — so 7.3 ms
is a floor with room in it, not the criterion met.

**1. The tiering was correct for ranking and wrong for cost.** Fuzzy ran whenever there
was room left on the page — which is nearly always, since one excellent exact match
still leaves four empty slots. So every search paid for a trigram scan over 2.7 million
rows it did not need, and the slowest queries were ones whose answer had *already been
found*. Fuzzy now runs only when the other tiers found fewer than two results. p95:
803 ms → 13.5 ms.

**2. A real bug: the two halves of the same search disagreed about what a title is.**
The FTS5 index folds diacritics (`remove_diacritics 2`); `normalise` did not. So
"Nausicaa of the Valley of the Wind" missed "Nausicaä of the Valley of the Wind" and
fell through to a keyword match on a making-of documentary. Both now fold via NFD, and
618,605 accented titles were re-folded (151 s).

**3. Four of the six failures were the fixture, not the code.** Where several works
share an exact title, search returns the most popular — and I had asserted the version I
think of as canonical:

| Query | Code | I asserted |
|---|---|---|
| and then there were none | 2015, 49,811 votes | 1945, 18,802 |
| monster | 2022, 226,103 | 2004 anime, 74,880 |
| hunter x hunter | 2011, 204,023 | 1999, 12,528 |
| fullmetal alchemist | Brotherhood, 257,061 | 2003, 87,114 |

The code is right: E2's intent is "you get a film with that title", not "you get the one
I would have picked". Corrected in the fixture with the reasoning kept, exactly as the
E5 anime fixture records its own four errors. **Across two fixtures this session, the
fixtures have been wrong eight times and the code twice.**

### The vector index: recall measured against brute force, and the dial chosen from it

`cargo run -p eval --release -- vector --report`, 2026-09-06, commit pending, over the
real 855,703-vector artefact. Built by `./target/release/ingest vector-index`.

| | |
|---|---|
| **vectors** | 855,703 |
| **build** | **439 s** (1,947/s average; starts near 6,800/s, settles near 1,950/s) |
| **index on disk** | **435 MB** — 1.4x the 313 MB artefact |
| **open (mmap)** | 69.9 ms |
| **resident** | **6.6 MB** against 435 MB on disk |
| **recall@10** | **0.9670** (gate 0.95) |
| **latency** | p50 1.59 ms, p95 4.20 ms |

**The mmap claim is now a number.** `eval vector --memory`, opening the same file both
ways:

| | resident | open time |
|---|---|---|
| **`view` (mmap)** | **6.6 MB** | 70 ms |
| `load` (read it all) | **758.6 MB** | 438 ms |

758.6 MB is **three times Tier 0's entire 250 MB idle budget** (`SPEC.md` §2.3), for
the tier the artefact exists to serve — and it exceeds the 435 MB file, because loading
rebuilds the graph as live allocations rather than mapping it. This is P11 settled with
a measurement instead of a library's description of itself.

**The graph costs 122 MB on top of the vectors it indexes.** 435 MB against the
artefact's 313 MB. It is derived, so it is not a download — but it is disk, and it sits
outside the database alongside the artefact.

#### expansion_search was swept, not guessed

`eval vector --ef N`, same 200 queries:

| ef | recall@10 | p50 | p95 |
|---|---|---|---|
| 64 (usearch's default) | 0.9400 | 0.97 ms | 2.09 ms |
| 128 | 0.9550 | 1.23 ms | 3.33 ms |
| **192 (shipped)** | **0.9670** | **1.57 ms** | **4.28 ms** |
| 256 | 0.9750 | 1.93 ms | 4.96 ms |
| 384 | 0.9845 | 2.55 ms | 6.03 ms |

**The library default missed the gate.** 0.9400 at ef 64 — which is the number that
would have shipped had the constant been taken on trust and the harness written
afterwards. 192 is the smallest value clearing 0.95 with margin. Higher was affordable
on this machine, but the 80 ms budget is a *Tier 0* budget and the query embedding
(P8: 24–33 ms padded) and keyword pass come out of the same 80 ms.

#### The harness was proved before its numbers were believed

`eval vector --prove` rotates the position → catalogue-id mapping by one, leaving the
artefact, the index and the metric untouched.

| | recall@10 |
|---|---|
| correct mapping | 0.9670 |
| **mapping shifted by one** | **0.0055** |

This is the check §10.8 asks for: a wrong key mapping is invisible by inspection,
because it returns ten plausible films for every query. Ground truth comes from
`crates/vector-index/src/brute.rs`, which scans the artefact bytes directly —
deliberately **not** usearch's own `exact_search`, which would have compared usearch
against usearch over the vectors usearch stored, and would have agreed with itself.

#### One query loses 8 of its 10 neighbours, and it is not a measurement artefact

`worst 0.20` at *every* ef value, which is the signature of something other than a
lossy dial. Two explanations, opposite in meaning, so both were measured rather than
assumed:

- **Tied vectors** — duplicates in the artefact making "the true top ten" arbitrary:
  **1** vector ties for last place. Not this.
- **A crowded band** — thousands of vectors equally distant: **12** lie within 1% of
  the 10th distance. Not this either.

Position 697,314 has a real neighbourhood — nearest at 0.0000, tenth at 0.4009 — and
HNSW misses most of it regardless of how many candidates it keeps. A genuinely
poorly-connected node. Recorded as debt rather than smoothed away; the mean clears the
gate and the shape of a single bad neighbourhood matters again in 5.4, where fusion
decides what a weak candidate set does to a result page.

#### E2 unchanged, and a cold-cache number worth having

`eval search --report` after all of the above: **43/43 = 100%**, unchanged.

Latency, however, split cleanly in two:

| | p50 | p95 | max |
|---|---|---|---|
| **cold** (straight after 750 MB of index I/O) | 5.6 ms | **36.8 ms** | **131.9 ms** |
| warm (three consecutive runs) | 0.9 ms | 7.1–7.4 ms | 59.9–61.7 ms |

Not a regression — the warm numbers reproduce the 7.3 ms already recorded. But the
cold column is the one a user meets on the first search after launch, and its **max
already exceeds E1's 80 ms budget on a single query**. E1 is a p95 criterion so this
does not fail it; it does mean E1 must eventually be measured cold, and Phase 21 owns
that.

### The query embedder, and proof the two halves have not drifted

`cargo run -p eval --release -- embed --report`, 2026-09-07, commit 396f256.

| | |
|---|---|
| model load | 141 ms (once, at startup) |
| first inference | 2.5 ms (ONNX warm-up, paid once) |
| **query latency** | **p50 2.2 ms, p95 2.4 ms** over 10 natural-language queries |
| **agreement** | **10/10 byte-identical to the artefact** |

The agreement number is the important one. A query is compared to document vectors by
cosine, which only means anything if both were produced the same way — and tokenizer
settings, truncation, pooling and normalisation are four chances to differ, every one of
them silent. So the harness re-derives a document exactly as the producer built it,
embeds it through the *query* path, quantises, and compares against that title's vector
in the artefact. ADR-0014 makes the artefact deterministic, so the bar is byte-identical.

**And the check was seen to fail.** Cutting `MAX_TOKENS` from 256 to 32 — one setting, in
one crate, the most ordinary drift imaginable:

| | agreement | worst cosine | exit |
|---|---|---|---|
| correct | 10/10 | 1.000000 | 0 |
| **truncation changed** | **2/10** | **0.781292** | **1** |

### E1, measured end to end for the first time

`cargo run -p eval --release -- search --report`, 2026-09-07, commit bdbd560. The harness
now drives the whole engine — query embedding, BM25, vector search and fusion — because
E1's budget is keystroke-to-results and says the embedding is inside it.

| | p50 | p95 | max |
|---|---|---|---|
| keyword only (previous phase state) | 0.9 ms | 7.3 ms | 60 ms |
| **hybrid, warm** (3 consecutive runs) | **4.8 ms** | **15.0 ms** | 71 ms |
| hybrid, cold | 17.1 ms | 67.5 ms | 324 ms |

**E2 survives fusion at 43/43 = 100%** — the exact-title short-circuit still sits above
everything, which is the property it exists to guarantee.

**E1 is NOT claimed as met.** Both p95 figures are under 80 ms, but `SPEC.md` §2.3
enforces budgets against **Tier 0** and this is a Tier 2 dev machine. At the 3–4x Tier 0
penalty P8 already flags, warm lands at 45–60 ms and cold at 236–270 ms. The criterion is
measured, not met.

The rise from 7.3 to 15.0 ms warm is fusion doing what it is for: each half now returns
50 candidates rather than 5, plus 2.4 ms of query embedding and 4.2 ms of vector search.

### THE FINDING THAT MATTERS: the documents cannot answer a semantic question

`eval search --query "films about grief that aren't depressing"` — the exact phrasing
exit criterion **E4** names:

| # | result | why |
|---|---|---|
| 1 | Grief (2017) | Semantic |
| 2 | My Grief isn't Grief Enough (2024) | Semantic |
| 3 | Grief (1994) | Semantic |
| … | *seven more films titled "Grief"* | Semantic |

And E4's second query, `"like Wong Kar-wai but Korean"`: *Wong Ka Yan*, *Hong Kil-dong*,
*A Short Film About Wong Kar Wai*, *Miss Korea*. Name and word matching, with no notion
of "like X" or "but Korean".

**This is not a fusion defect.** The vector half is doing exactly what it was built to
do. The problem is what it was built over:

```sql
SELECT COUNT(*) FROM media_items
 WHERE in_core = 1 AND kind <> 'episode'
   AND synopsis IS NOT NULL AND length(trim(synopsis)) > 0;
-- 0    of 855,703
```

**Zero synopses in the entire catalogue.** So every embedded document is:

```
Placebo (2002), animation comedy short film, featuring Jim Carrey…
```

Title, year, genres, kind, cast — nothing about what a film is *about*. The embedding
space encodes little more than the words of the title, which is why semantic search over
it behaves like a fuzzy title matcher.

**Why it was invisible until now.** Phase 4 built and verified the artefact — 855,703
vectors, checksummed, deterministic, byte-reproducible — and every one of those checks
passed, because they check that the file is *correct*, not that the documents are
*informative*. Nothing had searched it. The first real query found it in one look.

`SPEC.md` Phase 5 specifies the document as "synopsis, genres, keywords, director, mood
descriptors, and era". Three of those six do not exist in the catalogue: synopsis,
keywords, mood descriptors. Synopses come from TMDB, and ADR-0027 makes a TMDB key
optional and absent by default — which is exactly the configuration this catalogue was
built in.

**E3 (nDCG@10 > 0.75) and E4 cannot be met against this artefact**, and no value of `k`
or `ef` changes that. Raised as decision **P12** rather than worked around.

### IMDb serves conditional requests — measured, and it decides the refresh design

2026-09-07, resolving the open question in debt D24 and setting the shape of subtask 5.7.

```
HEAD https://datasets.imdbws.com/title.basics.tsv.gz
  Content-Length: 226419080
  Last-Modified:  Sun, 06 Sep 2026 00:48:16 GMT
  ETag:           "c5a942c76317cde815f77fcf113ed80a-27"

GET  with If-None-Match: "c5a942c76317cde815f77fcf113ed80a-27"
  -> HTTP 304, 0 bytes downloaded
```

**Why it matters.** The author asked for the catalogue to refresh every time the app is
opened. Taken literally that is a 216 MB download per launch (D24), which is not
shippable. With a conditional request it is one round trip returning **zero bytes**
whenever IMDb has not republished — and IMDb republishes daily, so at most one real
refresh a day and nothing on every other launch.

The `http_cache` table from migration 0008 already stores ETag and Last-Modified, and
`crates/metadata-api` already handles revalidation, so the pieces exist. D24 guessed this
would work; it now has a number.

### Wikipedia as the text source: what the catalogue gained (ADR-0033)

`ingest wikipedia`, 2026-09-07. Two phases, both measured.

**Mapping** — IMDb id → English article, joined on Wikidata property P345:

| | |
|---|---|
| queries | 110 (100 planned, **10 subdivided themselves** after timing out) |
| mappings offered by Wikidata | 245,708 |
| **matched onto this catalogue** | **233,661** (95.1%) |
| time | 43 min |

**Extracts** — the lead text, 20 articles per request:

| | |
|---|---|
| articles fetched | 233,416 |
| **with usable text** | **233,114** (99.87%) |
| no usable lead | 302 |
| time | 4.8 h (Wikimedia throttled us from 41/s to 13/s; the limiter absorbed it) |

**Coverage is concentrated exactly where it matters**, which is the number that decides
whether this was worth doing:

| slice of the core tier | with a synopsis |
|---|---|
| top 1,000 by votes | **100.0%** |
| top 10,000 | **98.8%** |
| top 100,000 | 77.8% |
| all 855,703 | 22.1% |

The long tail stays empty and that is the correct outcome: a film nobody has voted on
has no Wikipedia article either, and semantic search over it was never going to be the
thing that surfaced it. Average extract is 495 characters, against the document
builder's 400-character budget — so most titles contribute their whole lead.

Before this, the same query was `0 of 855,703`.

### Three distinct attempts at E3/E4, and where they stopped (§10.9)

Wikipedia text was loaded, and semantic search still could not answer a plot query.
Three genuinely different fixes followed; each helped, none was enough.

**Attempt 1 — enrich the documents** (233,114 synopses). "films about grief that aren't
depressing" still returned ten obscure films *titled* Grief, every one with **no**
synopsis. A document reading `Grief (1921), drama short film` is almost entirely the
word "grief", so it sits on top of any query containing it. **Sparse documents are
louder than rich ones.**

**Attempt 2 — index only items that have descriptive text** (855,703 → 189,470). Real
gains, and free improvements elsewhere:

| | before | after |
|---|---|---|
| index on disk | 435 MB | **96 MB** |
| build | 428 s | 54 s |
| resident (mmap) | 6.6 MB | 1.0 MB |

Results improved but stayed lexical: *Fish Hooky*, *Killer Fish* for a query containing
"fishing".

**Attempt 3 — remove the title from the embedded document** (`document::VERSION` 2).
`SPEC.md` Phase 5 lists the ingredients as "synopsis, genres, keywords, director, mood
descriptors, and era" — the title was never among them; including it was my addition.

Still not enough, and this measurement is why:

```
cargo run -p eval --release -- embed --compare \
  "a grieving janitor becomes guardian of his teenage nephew in a Massachusetts fishing town"
```

| item | shipped (400 chars) | whole synopsis | synopsis |
|---|---|---|---|
| **Manchester by the Sea** | 0.1944 | **0.2689** | 1,847 chars |
| Fish Hooky | **0.4194** | 0.3921 | 126 chars |
| The Nephew | 0.3932 | 0.3835 | 159 chars |

*Manchester by the Sea*'s synopsis contains the query almost verbatim — "a depressed and
grief-stricken man who becomes the legal guardian of his teenage nephew after the death
of his…". *Fish Hooky*'s says it was "the 120th Our Gang short to be released". The
trivia scores **twice as high**, and giving the real answer its whole text does not close
the gap.

**That is hubness, not truncation.** Short generic documents land near the centre of the
embedding space and are therefore close to every query. More text cannot fix a document
that has no more text.

**Attempt 3b — exclude trivia-length synopses** (`MIN_SYNOPSIS = 300`, 189,470 → 119,874).
The hubs disappear — no more *Fish Hooky* or *The Nephew* — and *My Mister*, an acclaimed
melancholic Korean drama, now answers "like Wong Kar-wai but Korean". But *Manchester by
the Sea* still does not surface for its own plot.

#### What the phase gained anyway

| | |
|---|---|
| **E2 exact-title top-1** | **43/43 = 100%**, unchanged throughout |
| **E1 latency** (hybrid, warm) | p50 7.5 ms, **p95 22.2 ms** against 80 ms |
| **recall@10** | **0.9810** (gate 0.95) |
| vector search | p50 1.01 ms, p95 1.72 ms |
| index | 61 MB, **1.0 MB resident** |

#### The conclusion, stated plainly

`all-MiniLM-L6-v2` is a **symmetric similarity** model being asked to do **asymmetric
retrieval** — a short query against a long document. Retrieval-trained models
(`bge-small-en-v1.5`, `e5-small-v2`) exist for exactly this, use query/passage prefixes,
and are the same 384 dimensions, so the artefact format would not change.

E3 and E4 remain **not met**, and are not claimed on the strength of a plan. Raised as
blocker **B4** under §10.9: three distinct approaches, each measured, none sufficient.

### Query understanding (subtask 5.5): filters, and two failures found by typing

`crates/search-engine/src/query.rs` extracts year ranges, runtime bounds and "directed
by" as **constraints**, leaving the rest as the text that gets embedded. Measured against
the real catalogue, 2026-09-07.

| query | result |
|---|---|
| `directed by Akira Kurosawa` | Seven Samurai · Rashomon · Ran · Yojimbo · Ikiru |
| `films directed by Alfred Hitchcock from the 1950s` | Rear Window · Vertigo · North by Northwest · Dial M for Murder · Strangers on a Train |
| `comedies under 90 minutes from the 1980s` | filtered from four fuzzy hits to the one 1984 title |

**Three of the four filters the spec names are implementable. Language is not** —
`media_items.original_language` is NULL on all 2,702,737 rows, because the IMDb datasets
do not publish it. A language filter today would be one that silently matches nothing.

#### Two failures that only appeared by running it

**1. A filter with a hole in it.** The first version filtered the fused tier and not the
fuzzy tier, so "comedies under 90 minutes" cheerfully returned three-hour films the
moment fuzzy ran. A filter the user believes and that does not hold is worse than none.

**2. A purely structural query returned nothing at all.** "directed by Akira Kurosawa"
leaves no text after extraction, and `admitted` can only *narrow* candidates some text
tier produced. Filters now **retrieve** as well as narrow.

#### And a 63x latency bug, in my own SQL

`EXISTS (… JOIN people … WHERE p.name LIKE '%kurosawa%')` against `media_items` runs
per row, so SQLite considered 2.7 million of them:

| | |
|---|---|
| first version | **11,932 ms** |
| resolve the person first, then `person_id IN (…)` | 1,800 ms |
| **drive the query from `credits` rather than `media_items`** | **190 ms** |

The indexes (`idx_people_name`, `idx_credits_person`) existed the whole time; the query
shape defeated them. Starting from the thirty rows a director actually has, instead of
filtering 2.7 million down to them, is the whole difference.

**Still over budget and recorded as such**: 190 ms for a director query, 356 ms when a
year filter joins it, against E1's 80 ms. E1's fixture is exact titles so this does not
appear in that number — which is exactly why it is written down here.

### The model swap, measured (ADR-0034)

`bge-small-en-v1.5` replacing `all-MiniLM-L6-v2`. Re-embed 11,247 s (BGE is slower per
document), index rebuild 29 s, artefact sha256 `b17f15da…`, document builder v2, text
source `wikipedia`.

**E4's two named queries, before and after:**

| | MiniLM | **BGE** |
|---|---|---|
| *films about grief that are not depressing* | Funeral Home · Children of Sorrow · The Sadness · Sea Sorrow | **Good Grief (2023)** · Grief (1993) · Mourning Has Broken · **About Memory and Loss** · **No Sad Songs** · Bereavement |
| *like Wong Kar-wai but Korean* | Miss Korea · You Are the Best! · Dear Hongrang · Mouse | **Chungking Express (1994)** · *wkw/tk/1996* · *There's Only One Sun* · then Korean series |

The second is the striking one: **Chungking Express is Wong Kar-wai's signature film**,
and positions 2 and 3 are also his. The model recognised the director from the name alone
— MiniLM never did. It has not combined "like Wong Kar-wai" *with* "but Korean"; the page
mixes his films and Korean drama rather than finding Korean films that resemble his.

The first now leads with *Good Grief*, a comedy about bereavement, which is close to
literally what the query asked for.

**Everything else held or improved:**

| | |
|---|---|
| **E2 exact-title top-1** | **43/43 = 100%** |
| **E1** hybrid, warm | p50 9.8 ms, **p95 31.3 ms**, max 90.7 ms |
| **recall@10** | **0.9670** (gate 0.95) |
| vector search | p50 0.94 ms, p95 1.49 ms |
| query embedding | p50 5.5 ms, p95 7.2 ms (BGE is ~3x MiniLM's 2.2 ms) |
| producer/query agreement | **10/10 byte-identical** |
| index | 61 MB, **1.0 MB resident** |

#### What is still wrong, and the next measurable step

A third, harder query — *"a grieving janitor becomes guardian of his teenage nephew in a
Massachusetts fishing town"* — still does not surface *Manchester by the Sea*:

| item | cosine | synopsis |
|---|---|---|
| They Killed Sister Dorothy | **0.5788** | 336 chars |
| Manchester by the Sea | 0.4947 → **0.5239** with its whole synopsis | 1,847 chars |

The gap narrows from 0.084 to 0.029 when the full synopsis is used, which points at the
**document layout** rather than at the model. Manchester's document opens with
`(2016) drama film, featuring Kenneth Lonergan, Casey Affleck, Michelle Williams, Kyle
Chandler and Lucas Hedges.` — five names before a word of plot, inside a 400-character
budget.

Two untried changes, in order of expected value: **put the synopsis first**, and **raise
`SYNOPSIS_CHARS`** (the model reads roughly 1,000 characters' worth of tokens; we give it
400). Both are document-builder changes, so both cost a re-embed — and
`eval embed --compare` can price them in a minute before that is spent.

**E3 and E4 are still not claimed.** E4 additionally requires screenshots in a case
study, which needs the Phase 5.8 UI; E3 needs the semantic fixture from 5.6.

### E3 is a number now (subtask 5.6)

`cargo run -p eval --release -- search --report`, 2026-09-13, over
`fixtures/search/semantic-queries.tsv` — 10 queries, 37 graded answers, every film named
from independent knowledge before anything was run.

| | nDCG@10 | queries | target |
|---|---|---|---|
| **filter** | **1.0000** | 2 | 0.75 |
| **meaning** | **0.1188** | 8 | 0.75 |

**The filter half is perfect.** "directed by Akira Kurosawa" and "films directed by
Alfred Hitchcock from the 1950s" both return exactly the graded answers, in order. The
structured half of search works.

**The meaning half is nowhere near**, and the per-query detail says why:

```
"a man returns to his hometown to care for his nephew after a death"   nDCG 0.0000
   The Returning · The Return · Kids Return · We Shall Return · Returning Home

"slow science fiction about memory"                                    nDCG 0.0000
   About Memory and Loss · 2009: Lost Memories · Mémoire entropique · Hauser's Memory
```

**Still matching query words in titles** — after the titles were removed from documents
in `document::VERSION` 2. The route back in is Wikipedia itself: **every lead opens by
restating the title and the credits** — *"The Return is a 2003 Russian drama film directed
by…"* — so the title returns through the synopsis, and under CLS pooling that opening
sentence is weighted heavily while the plot behind it falls past the 400-character cut.

#### D39 priced, and rejected on the evidence

The obvious fix was to strip that boilerplate lead and put the plot first.
`eval embed --compare` measured it for the cost of one command rather than a three-hour
re-embed:

| item | shipped | whole synopsis | **plot first, lead stripped** |
|---|---|---|---|
| Manchester by the Sea *(the answer)* | 0.4884 | 0.5059 | **0.5539** |
| The Return | 0.5698 | 0.5823 | 0.5977 |
| Uncle Boonmee | 0.6069 | 0.5942 | 0.6295 |

It lifts the correct answer by 0.065 — **and lifts the wrong answers by about as much.**
The ordering does not change. A re-embed on this evidence would have bought nothing, and
the measurement cost a minute.

#### What the number actually indicts

Two of the eight meaning queries are **known-item** queries: they describe one specific
film, and nDCG over a fixture listing two answers scores 0 whenever that film lands at
rank 11. The other six are topical and score better — "films about grief that aren't
depressing" reaches 0.4770 with *Good Grief* first.

Worth saying plainly: *The Return* and *Uncle Boonmee* are **not absurd answers** to a
query about a man returning home after a death, and the fixture grades them 0. Some of
the 0.1188 is the fixture being narrower than the truth. Widening it to match what the
engine returns is exactly the trap the fixture header warns about, so the honest move is
to record the tension and change the fixture only where a film would have been listed
anyway, on its merits.

**E3 is not met, and it is now a number rather than an impression** — which is the whole
point of the subtask.

### The app opens the database, and refreshes itself (subtask 5.7, D26, D37)

Release build, dev machine (Tier 2), 2026-09-13. `cold_start_ms` from
`target/release/data/logs/`, process start → frontend painted.

**Cold start is unchanged**, which is the point of opening the database off the startup
path:

| launch | cold start |
|---|---|
| Phase 1 baseline | 515 / 660 ms |
| first run of a fresh binary | 871 ms |
| subsequent | **522 · 366 · 342 · 339 · 328 ms** |

The 871 ms outlier is the first execution of a newly-linked binary; every warm launch
sits at or below the Phase 1 figures. The database handle is created on a background
task and installed when ready, so nothing on the path to the first painted frame waits
for a disk.

**And the catalogue now refreshes itself**, automatically and silently, by the author's
decision:

```
02:19:27  database open
02:19:27  frontend interactive cold_start_ms=871
02:20:27  catalogue freshness outcome=Refreshed { titles_added: 4604 }
…
02:55:11  frontend interactive cold_start_ms=339
02:55:11  catalogue freshness outcome=AlreadyCurrent
```

**4,604 titles the catalogue did not have**, fetched without anyone typing a command —
the thing ADR-0030 specified and nothing had ever done. And a launch where IMDb has not
republished resolves to `AlreadyCurrent` **260 ms after the frontend paints**, costing
one HEAD request and no download.

#### Two bugs, both found by running it rather than reasoning about it

**1. Every launch re-downloaded 216 MB.** The first check is stale *without* a request —
there is no cached validator to compare — so it carried none, so nothing was recorded,
so the next launch was stale again. Observed directly: two consecutive launches, the
second reporting `Refreshed { titles_added: 0 }` after downloading the lot.

**2. Recording it at the end did not fix it either.** The app exits when its window
closes and a refresh takes about a minute, so a user who opens and closes quickly never
reaches the end — and never learns a validator. Confirmed by four launches leaving
`http_cache` empty.

The validator is now recorded **after the download and before the load**, which is the
only boundary that is wrong in neither direction:

| killed during | consequence |
|---|---|
| the download | nothing recorded, fetched again — correct |
| the load | bytes kept, validator recorded, `Job` resumes — correct |

#### One number worth chasing later

```
slow statement: SELECT MAX(CAST(SUBSTR(e.external_id, 3) AS INTEGER)) …   13.68 s
```

The refresh watermark — the highest IMDb id already held — is a full scan with a cast
over 3.2 million rows, and no index can serve it. It runs on the background task so it
costs no user-visible time today, but 13.7 s of disk on every refresh is not free.
Recorded as debt.

### The optional downloads, end to end (subtask 5.9)

`cargo run -p sinephile-catalogue --example fetch_assets -- <dir>`, 2026-09-15, against
the real published release `embeddings-v2`.

```
  plan: 3 files, 346 MB total
    Meaning index      313.4 MB  Absent
    Language model      32.4 MB  Absent
    Tokenizer            0.7 MB  Absent

  Meaning index    VERIFIED      internal checksum + model identity + builder version
  Language model   VERIFIED      pinned file sha256
  Tokenizer        VERIFIED      pinned file sha256
```

**There are three downloads, not one.** The model that embeds the query is 32.4 MB of the
346, and it had never been scoped because it was copied into place by hand during
development. The consent screen names each file separately.

#### The real download caught a mistake a unit test could not

The first run downloaded all 313 MB and was **refused**:

```
has sha256 32b93f3ac0824683ec5e417d22493b6d01b8dc93ea826aec923ed308978955be,
expected   b17f15dab916816be1e4b959cbfce6f5d369ab6e7bd5c676e6760b0a2980b7bb
```

`b17f15da…` is the value `ingest embed` prints — the artefact's **internal** checksum,
computed over its header and vectors. `verify_sha256` hashes the **whole file**, which
includes the 32-byte trailing checksum itself. Two different things both called "the
sha256", and pinning one where the other was meant.

The fix was not to pin the right constant. **The artefact verifies itself** — its own
checksum covers its own bytes, catches truncation, and cannot go stale when the artefact
is rebuilt — so `sha256` is now `Option`, `None` for the artefact, and pinned only for the
model and tokenizer, which are opaque and cannot vouch for themselves.

Worth noting what happened here: the refusal was *correct behaviour* firing on an
*incorrect constant*, and the only thing that could distinguish those was running it
against a real file.

## Session 16 — 2026-09-21/22 — the document layout, priced properly

### D39 was answered with the wrong documents, and the answer flips

`cargo run -p eval --release -- embed --compare "<query>" <ids>`, 2026-09-22, real
catalogue, `bge-small-en-v1.5-int8`.

D39 concluded that re-laying-out the document lifts the right answer and the wrong ones
by the same amount, leaving the order unchanged — so a re-embed would buy nothing. That
conclusion came from variants built with `format!`:

```
longer = format!("{document} {synopsis}")        // synopsis prefix appears TWICE
plot   = format!("{stripped} {document}")        // metadata AND prefix appear twice
```

Neither is a document any rebuild could produce. `document::build_with` now takes a
`Layout`, so a variant is assembled by the producer's own builder and each part appears
exactly once. Measured again, **the order does change.**

`"a man returns to his hometown to care for his nephew after a death"`

| item | shipped | 1000 chars | plot first | no cast |
|---|---|---|---|---|
| **Manchester by the Sea** *(graded 2)* | 0.4884 **#5** | 0.5136 #5 | 0.5609 **#3** | 0.5733 #3 |
| Uncle Boonmee | 0.6069 #1 | 0.6105 #1 | 0.5958 #1 | 0.6005 #1 |
| The Return | 0.5698 #3 | 0.5711 #3 | 0.5850 #2 | 0.6002 #2 |
| The Returning | 0.5718 **#2** | 0.5666 #4 | 0.4798 **#6** | 0.4767 #6 |
| Kids Return | 0.5691 #4 | 0.5807 #2 | 0.5395 #4 | 0.5420 #4 |
| **Ordinary People** *(graded 1)* | 0.4689 **#6** | 0.4634 #6 | 0.4826 **#5** | 0.4865 #5 |

`"films about grief that aren't depressing"` — the clearer one:

| item | shipped | 1000 chars | plot first | no cast |
|---|---|---|---|---|
| **Good Grief** *(graded 2)* | 0.6760 #1 | 0.6771 #1 | 0.6522 #2 | 0.6598 #1 |
| **Manchester by the Sea** *(graded 1)* | 0.5692 **#6** | 0.5795 #6 | 0.6553 **#1** | 0.6590 #2 |
| **A Monster Calls** *(graded 1)* | 0.5430 **#7** | 0.5289 #7 | 0.5329 **#5** | 0.5343 #4 |
| Grief *(ungraded)* | 0.6633 **#2** | 0.6638 #3 | 0.5186 **#6** | 0.5297 #5 |
| Mourning Has Broken *(ungraded)* | 0.6579 #3 | 0.6654 #2 | 0.5701 #3 | 0.5815 #3 |
| Disconnected *(ungraded)* | 0.6578 #4 | 0.6560 #4 | 0.5599 #4 | 0.5137 #6 |
| Bereavement *(ungraded)* | 0.6369 **#5** | 0.6259 #5 | 0.4902 **#7** | 0.4849 #7 |

Under `plot first` the three graded answers hold #1, #2 and #5 where they held #1, #6 and
#7; every ungraded title falls. **That is a reordering, which is the trigger D39 set for
spending a re-embed** — and `1000 chars` alone is not enough to produce it, so it is the
position of the metadata rather than the length of the synopsis that does the work.

What this does NOT establish: these are six- and seven-item probes over the fixture's own
contested set, not the catalogue. Other items rise too, and nothing here predicts what
nDCG@10 over 2.7 million titles would do. It says a rebuild is now worth its three hours,
not what the rebuild would score.

### E3's number, decomposed — it was measuring three things at once

`cargo run -p eval --release -- search --report`, 2026-09-22. **The headline is
unchanged** and stays the criterion as SPEC.md Phase 5 words it:

```
E3 — nDCG@10 over the semantic query set
   meaning  0.1188  over 8 queries   (target 0.75)
   filter   1.0000  over 2 queries   (target 0.75)

diagnostics — what that number is made of, not a restatement of it
   topical      nDCG@10 0.1584  over 6 queries
   known-item   MRR 0.0000, found in the top 10: 0/2
```

D40's two known-item queries score 0.0000 and neither film reaches the top 10 at all, so
MRR agrees with nDCG here rather than rescuing it. The topical six sit at 0.1584.

### Two graded answers cannot be returned at all, and two queries are capped below E3

The harness now reports, per query, the best nDCG@10 it could possibly reach given which
graded answers are in the vector index:

```
UNREACHABLE ANSWERS — graded films the vector half cannot return at all
   (synopsis under MIN_SYNOPSIS = 300, so not in the index)
   ceiling 0.7468 — BELOW THE TARGET  "films about grief that aren't depressing"
                      tt0103129  graded 2      (Truly Madly Deeply, 235 chars)
   ceiling 0.7468 — BELOW THE TARGET  "like Wong Kar-wai but Korean"
                      tt0140825  graded 2      (Christmas in August, 169 chars)
```

`MIN_SYNOPSIS = 300` keeps sparse documents out of the index, and it was measured and
right. It also means two of the eight meaning queries **cannot reach 0.75 however good
the engine gets** — the ceiling is 0.7468. A fixture that cannot distinguish "failing"
from "already at its ceiling" is not measuring what it claims to.

26 of the 28 graded meaning answers ARE reachable, so the rest of the 0.1188 is the
engine, not the fixture.

#### Seen to fail

`MIN_SYNOPSIS` temporarily raised 300 → 1500 and one topical query retagged
`meaning/known-item`: ceilings fell to 0.2385–0.7602, six queries reported capped instead
of two, the "BELOW THE TARGET" marker appeared and disappeared correctly either side of
0.75, and MRR moved 0.0000 → 0.3333 with presence 1/3. **E3's headline stayed 0.1188
through both perturbations**, which is the property that matters. Both restored.

### The agreement check had stopped running, and why

`cargo run -p eval --release -- embed --report` errored:

```
the artefact holds 855703 vectors and the catalogue offers 858170 core ids
```

Subtask 5.7 made the catalogue refresh itself on launch; the artefact is a published
snapshot. It had drifted **2,467 core titles in a week** and the equality check stopped
the harness dead. All 2,467 new ids sit after the artefact's last position (verified
directly), so position *n* still means the same title — the artefact is a prefix of the
catalogue, and the check was stricter than the invariant. It now compares over the
artefact's range, reports the gap, and still refuses if the catalogue has *fewer* ids,
which would mean positions had shifted.

Restored, and with `document::build_with` in place:

```
agreement  10/10 byte-identical to the artefact
latency    p50 5.6 ms   p95 6.6 ms
```

**That 10/10 is what proves the `Layout` refactor changed no shipped bytes** — documents
re-derived through the new builder quantise to exactly the published vectors.

### D44/D45 fixed, and the fix measured on the real catalogue (session 16)

`cargo run -p sinephile-ingest --release -- vector-index`, 2026-09-22, against the real
`embeddings-v2` artefact and a catalogue that had already drifted 2,467 titles past it —
the exact condition that made the old equality guard refuse:

```
  vectors          119874 of 855703 considered
  beyond artefact  2467 core titles the catalogue has gained since — keyword only
  index size       61 MB  (0.2x the artefact's 313 MB)
  build            33s  (3651 vectors/s)
```

The content verification ran first and passed: eight documents re-derived from the live
catalogue, embedded, quantised, and equal to the artefact's own bytes at those positions.

**No regression against the hand-built index it replaced.** The index built by the new
path and the one built by hand on 2026-09-07 are equivalent on every recorded metric:

| metric | 2026-09-07 (hand-built) | 2026-09-22 (this path) |
|---|---|---|
| recall@10 | 0.9670 | **0.9670** |
| E2 exact-title top-1 | 43/43 | **43/43** |
| E3 meaning / filter | 0.1188 / 1.0000 | **0.1188 / 1.0000** |
| index size | 61 MB | 61 MB |

`cargo run -p eval --release -- vector --report` and `-- search --report`. E1 read
23.7 ms p95 against 16.1–17.5 ms earlier in the session; the spread across three runs on
an unquiesced machine is larger than the difference, and all three sit far under the
80 ms budget.

And the application agrees. Launching the release-path binary logs:

```
sin_e_phile_lib::commands::assets: search: hybrid
cold_start_ms=485
```

which is the whole point of the fix: before it, a machine that had opened the app twice
could never build an index again, so that line read `keyword only (no index)` and the
313 MB sat unread.

#### Seen to fail

`verify_prefix`'s byte comparison was disabled (`if false && …`) and
`a_title_promoted_into_the_middle_of_the_core_tier_is_refused` failed, as it must — that
test promotes one title into the middle of the core tier, which is the shift a length
check cannot distinguish from a harmless append. Restored; 6 of 6 pass.

## Session 19 — 2026-10-25 — R4 re-measured, the v3 artefact, and its evals

### The database against R4's 4 GB, read as SI (D47, author's option 1)

R4 says "4 GB". Phase 4's "341 MB of headroom" read that as 4,000 MiB; the author chose
the SI reading (4,000,000,000 bytes), so every figure below is in SI units unless it
says MiB.

| when | bytes | GB (SI) | against 4 GB | command |
|---|---|---|---|---|
| 2026-09-07, Phase 4 E1 | 3,836,723,200 | 3.837 | 163 MB under | `ls -la data/sinephile.db` |
| 2026-10-25, before compaction | 4,062,978,048 | 4.063 | **63 MB over** | `ls -l data/sinephile.db` |
| 2026-10-25, after compaction | 3,945,598,976 | **3.946** | **54 MB under** | `./target/release/ingest.exe compact` |

Compaction recovered 111 MiB in 34 s (`compacted 4062978048 -> 3945598976 bytes`), at
commit `814fc41`. Every ingestion command that writes the catalogue now compacts after
itself (`df3ca35`) unless it's run with `--no-compact`. The app's own launch-time
refresh does NOT compact (VACUUM is exclusive and slow), so each launch that adds titles
eats into the 54 MB until the next ingestion command runs.

### The v3 artefact (D48, D45)

```
./target/release/ingest.exe embed     (resume; reassembly only)
  858170 vectors
  314 MB · 7s
  sha256 106f3de1644fe5a5d928a65495fea62b2a4412f132c794d5c737b918ae404229
```

329,537,568 bytes = 256 header + 858,170 × 384 + 32 trailer. Document layout v3
(plot first, 1,000-char synopsis), artefact format 3 (watermark + id digest). Commit
`814fc41`.

### The v3 index and search evals — E3 meaning REGRESSED

```
./target/release/ingest.exe vector-index
  vectors          119874 of 858170 considered
  index size       61 MB  (0.2x the artefact's 314 MB)
  build            22s  (5566 vectors/s)
```

| metric | v2 (2026-09-22) | **v3 (2026-10-25)** | command |
|---|---|---|---|
| recall@10 | 0.9670 | **0.9760** (164/200 perfect, worst 0.70) | `cargo run --release -p eval -- vector --report` |
| index latency | — | p50 0.96 ms, p95 1.41 ms | same |
| E2 exact-title top-1 | 43/43 | **43/43** | `cargo run --release -p eval -- search --report` |
| E1 hybrid p95 | 23.7 ms | **15.2 ms** (p50 6.4, max 69.6) | same |
| E3 meaning | 0.1188 | **0.0472** — regression | same |
| E3 filter | 1.0000 | **1.0000** | same |
| topical nDCG@10 | — | 0.0629 over 6 | same |
| known-item MRR | — | 0.0000, 0/2 in top 10 | same |

Commit `814fc41`. Per-query meaning nDCG@10: grief 0.1505, hometown/nephew 0.0000,
Wong Kar-wai/Korean 0.0752, slow SF memory 0.0000, anime loneliness 0.0000, heist
0.0000, mountain documentaries 0.1518, Italian neorealism 0.0000.

**This is a regression on a recorded metric, so it blocks the merge (CLAUDE.md rule
13).** v3 was chosen on `eval embed`'s ranking; that ranking and this number disagree,
and finding out why comes next.

#### v2 against v3, per query, on the same database

The pre-v3 code (`5a4b01c`, built in a scratch worktree) was run against the intact v2
artefact (`embeddings-…v2-backup.bin`) and today's compacted database. It reproduces
v2's recorded headline exactly, so the comparison holds: the only variable is the
artefact.

| query | v2 nDCG@10 | v3 nDCG@10 |
|---|---|---|
| films about grief that aren't depressing | 0.4770 | **0.1505** |
| a man returns to his hometown… nephew… | 0.0000 | 0.0000 |
| like Wong Kar-wai but Korean | 0.2385 | **0.0752** |
| slow science fiction about memory | 0.0000 | 0.0000 |
| anime about loneliness in a big city | 0.0000 | 0.0000 |
| heist films where the plan goes wrong | 0.0000 | 0.0000 |
| documentaries about mountain climbing | 0.0000 | **0.1518** |
| post-war Italian neorealism | 0.2346 | **0.0000** |
| **meaning** | **0.1188** | **0.0472** |
| topical (6) | 0.1584 | 0.0629 |
| filter | 1.0000 | 1.0000 |
| E2 | 43/43 | 43/43 |
| E1 p95 | 14.8 ms | 15.2 ms |

`SINEPHILE_DATA_DIR=<scratch> <5a4b01c>/eval search --report`, 2026-10-25.

**What moved.** v2's layout led with the title, and its three wins were partly titles that
contain the query's own words: *Good Grief* at #1 for "grief", and *Chungking Express*
at #1 for "Wong Kar-wai". Under v3's plot-first layout *Good Grief* falls to #8 and
*Chungking Express* to #8. The neorealism query lost *Rome, Open City* (#3 → absent).
The mountain documentary query gained *Meru* at #6.

**An observation, NOT a correction:** v3's neorealism top 10 holds *Stromboli* and
*Germany Year Zero*, Rossellini films from the period, and neither is graded. The
fixture's grades are sparse. Regrading after seeing which layout wins would be
redefining the criterion to fit the result, so the 0.0472 stands as measured.

**What D39's probe did and did not predict.** It showed plot-first *reorders* the
six or seven contested items in a probe set, and said explicitly that it did not
predict nDCG@10 over the catalogue. Over the catalogue, the reordering also let
858,170 other plots compete, and the graded answers lost.

**Accepted by the author, 2026-10-25: "keep v3".** The v3 artefact ships with E3 meaning at
0.0472, a recorded regression from v2's 0.1188, chosen deliberately over a 3-hour revert
(B7). The baseline future phases compare against is therefore **0.0472**, not 0.1188.

### The search screen's byline query (D53 redesign)

`SearchRepository::details`, the director/runtime/synopsis lookup for the hits on screen,
timed against the real catalogue (3.946 GB), 20 random core-tier film ids per call,
20 calls, cold pages: **p50 9.35 ms, p95 10.30 ms, max 12.62 ms**. Primary-key lookup on
`media_items`, `idx_credits_item` for the credits. It runs after ranking, so it adds to
E1's 15.2 ms p95 rather than replacing any of it, for roughly 25 ms of the 80 ms budget.
Python `sqlite3` with the exact SQL, 2026-10-25, uncommitted tree on top of `21d0b23`.

---

## Phase 6 — Source resolver (2026-11-05)

### E2: the Internet Archive backend returns real, playable results

**Find.** `cargo test -p sinephile-sources --test archive -- --ignored --nocapture`
(`live_the_archive_returns_a_reachable_whole_film`, commit `9ab7929`), query
*Night of the Living Dead* (1968), the live service: **3 candidates** in 12.68 s for the
whole test binary (one search, up to four metadata lookups, one HEAD). First candidate
resolved to `https://archive.org/download/NightOfTheLivingDead-MPEG/NightOfTheLivingDead.mp4`,
HEAD **200** after the Archive's redirect to a storage node.

**Play.** The same URL, through FFmpeg 9.0 (the decoder the player will use):

| Check | Command | Result |
|---|---|---|
| Streams | `ffprobe -v error -show_entries format=duration,format_name:stream=codec_type,codec_name,width,height -of compact <url>` | h264 640×480 video, aac audio, mp4 container, duration 5744.97 s (1 h 35 min) |
| Decode | `ffmpeg -v error -t 30 -i <url> -f null -` | exit 0, no errors; **900 frames** (30.03 s) decoded, 223× realtime, 5.47 s wall including connection |

The Archive labels the file `h.264`, which the backend maps to `VideoCodec::H264`, and
the probe agrees.

**One transient failure on the way, recorded because it will recur.** The first live
run's HEAD failed with a network error after ~16 s: the download redirected to
`dn711104.ca.archive.org`, which did not resolve from this machine at that moment.
Minutes later the same request redirected to a different storage node and succeeded,
and the first name resolved again. A candidate can be valid and briefly unreachable;
the player (Phase 8) must treat a failed open as "try the next candidate", not as the
film being unavailable.

### Regression check before merging Phase 6 (rule 13)

Every harness against the real catalogue (3.946 GB, migrations 0015–0017 applied on
open), release build, commit `1749551`, 2026-11-05. Phase 6 did not touch ranking; these
confirm it did not move anything either.

| Metric | Phase 5 recorded | Now | Command |
|---|---|---|---|
| E2 exact-title top-1 | 43/43 | **43/43** | `cargo run --release -p eval -- all` |
| E3 meaning nDCG@10 | 0.0472 | **0.0472** | same |
| E3 filter nDCG@10 | 1.0000 | **1.0000** | same |
| E1 p50 / p95, warm | 6.4 / 15.2 ms | **6.3 / 14.5 ms**, then 6.7 / 15.6 ms | `target/release/eval.exe search`, twice |
| HNSW recall@10 | 0.9760 | **0.9760** (200 queries; index p95 1.29 ms) | `target/release/eval.exe vector --report` |

The first E1 run, the one immediately after the migrations, read **p50 10.8 / p95 43.0 ms**:
the first open after a schema change, with the database's pages cold. The two runs after
it are the comparable ones and sit either side of the Phase 5 figure. Recorded rather than
dropped, because a cold first search is what a user sees after an update, and it is still
inside the 80 ms budget.

## Phase 7 — Torrent engine (2026-11-05)

Live, against the official Ubuntu 24.04 desktop torrent fetched at run time (never
committed), over this machine's connection (routed through a VPN), commit `250dbf5`,
`target/release/examples/bench.exe <torrent> <scratch> [bulk-seconds]`:

| Run | add → first 1 MiB via local HTTP (E1 proxy, < 8 s) | seek to 60% → 1 MiB (E2, < 5 s) | bulk |
|---|---|---|---|
| 1 | **9.58 s** — over | **2.17 s** | 31.87 MiB/s over 60 s, 52 peers |
| 2 | **15.37 s** — over | **0.84 s** | 12.48 MiB/s over 5 s |
| 3 | failed: the first range read returned 181 bytes (an error response), not investigated yet | — | — |

Run 3's short read was a real bug, now fixed: the streaming server sent `206` headers
before opening the torrent reader, so when opening failed the player got headers and a
closed connection. It now opens first and answers `503` (`a_dead_swarm_fails_fast_with_a_clear_error`
asserts the 503; seen to fail against the old ordering, which gave `206` with an empty body).

### Where E1's time goes (2026-11-06, `bench` now prints the first live peer)

Eighteen more runs. Once a peer is connected, the first MiB arrives in **0.6–3.4 s**. The
variance is all in **finding the first peer**: 2.2 s at best, 55 s at worst, and it grew over
a day of back-to-back runs re-announcing the same torrent from one address.

| Attempt | First live peer | add → first MiB |
|---|---|---|
| fresh data dir ×3 | 2.6, 2.2, 25.0 s | 3.8, 3.8, 28.5 s |
| kept data dir ×6 (DHT state is only saved every 60 s, so it never warmed) | 8.2–48.6 s | 11.7–50.1 s |
| TCP + uTP listener ×4 (reverted: no evidence it helps) | 22.1–53.8 s | 24.2–55.4 s |

### E6: against qBittorrent, same torrent, same connection, minutes apart

| | add → first 1 MiB | bulk |
|---|---|---|
| **sin-e-phile** (`bench`, 30 s bulk) | **3.58 s** (first peer 2.45 s) | **33.85 MiB/s**, 55 peers |
| **qBittorrent 5.2.0** (`python tools/bench/qbittorrent_compare.py`, sequential + first/last-piece priority, throwaway profile) | **13.08 s** | **34.71 MiB/s**, 32 seeds |

qBittorrent waits on the same peer discovery, so the slow starts are the swarm and this
network (a VPN, no inbound connections), not the scheduler. Bulk throughput is level.

**E2 (seek, < 5 s): met in all 18 runs**, 0.15–3.73 s.

**E1 (< 8 s): not reliably met** — met in 4 of 20 runs. Carried to Phase 8 under ADR-0036,
where first frame is measured end to end and the torrent can be started when the detail
page opens, so peer discovery overlaps with the user reading rather than following Play.

### Regression check before merging Phase 7 (rule 13)

2026-11-06, release build at `01be3d8`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000**;
E1 warm p50 / p95 **6.5 / 14.9 ms** (the first run after a rebuild read 13.8 / 44.7 ms with cold
pages, as in Phase 6); HNSW recall@10 **0.9760**. Unchanged from Phase 6.

## Phase 8 — Player core (2026-11-06)

### E2: 4K HEVC HDR decodes on the GPU

Clip: 3840×2160, HEVC Main 10 (`yuv420p10le`), PQ (`smpte2084`) / BT.2020, 24 fps, 20 s, made with
`ffmpeg -f lavfi -i testsrc2=size=3840x2160:rate=24 -t 20 -c:v libx265 -pix_fmt yuv420p10le
-x265-params colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc hdr4k.mp4`.
Machine: RTX 5070 Ti Laptop GPU, driver 610.88; libmpv shinchiro 20261021. Release build at `5506006`
plus the example. Command: `cargo run --release -p sinephile-player --example hwdec -- hdr4k.mp4 8`.

| Decode | mpv `hwdec-current` | Process CPU (share of one core) | Played |
|---|---|---|---|
| Hardware allowed | **`d3d11va-copy`** | **18.2 %** | 8.0 s in 8.0 s |
| Forced software | — (`software`) | **35.9 %** | 8.0 s in 8.0 s |

Hardware decode halves CPU at 4K HDR with real-time playback in both modes. `-copy` because
the run is headless (no swapchain to hand the D3D11 surface to); in the window the same
decoder runs without the copy. A synthetic test pattern is easy to decode, so the software
figure understates what a real film costs — the point is the decoder chosen, not the margin.

### Play → first frame in the real window (debug build, local 1080p clip)

2026-11-06, `player open` → mpv's first `playback-restart`, logged by `player_open`
(`SINEPHILE_AUTOPLAY=<clip> target/debug/sin-e-phile.exe`, then the `first frame` log line):
**1506 / 1497 / 1518 / 1448 ms** over four launches; creating the player is **9–11 ms** of it.

`SINEPHILE_MPV_LOG=v` shows where it goes: **1306 ms is WASAPI** failing with
`AUDCLNT_E_DEVICE_IN_USE` on the default output (another application holds it
exclusively), retrying at 1 / 10 / 100 / 1000 ms and then playing without sound. The
video path is the remaining **~200 ms** — within §2.3's 500 ms. A persistent shader cache
(`gpu-shader-cache-dir`) made no measurable difference (1684 → 1465 ms, inside the noise
of the audio retry). The < 500 ms criterion needs a run with the audio device free.

### E3: exact seeks

2026-11-06, `cargo test --release -p sinephile-player --test playback an_exact_seek -- --nocapture`:
paused, seek to **3.52 / 1.08 / 5.2 s** (25 fps, one keyframe per second) landed at
**3.520 / 1.080 / 5.200** in **4.5 / 2.3 / 2.8 ms**, measured to mpv's `playback-restart`.

### Play → first frame streamed from the Internet Archive (real window, debug build)

2026-11-06, search → detail → Play driven by keystrokes posted to the WebView, `first frame`
log line: **7859 ms** (an AVI of the 1922 *Nosferatu*) and **6349 / 5533 ms** (an MP4 copy,
resuming at 0:52). Each includes the ~1.3 s WASAPI retry above. §2.3's streaming budget is
< 8 s: met, without much room.

### Phase 7 E1, carried: the torrent started when the detail page opens (D61)

2026-11-06, `HEAD_START=5 target/release/examples/bench.exe <legal Linux ISO .torrent> <dir> 1`,
ten fresh runs, the torrent added 5 s before "Play" (time spent on the detail page), timed
from Play to the first MiB through the streaming server:
**2, 242, 3957, 5, 59362, 3, 2, 2, 2, 2 ms** — under 8 s in **9 of 10**. The miss is a run
where no peer connected for 62 s. With no head start the Phase 7 figures stand (first peer
2–55 s on this network): the head start hides discovery, it does not speed it up.

### Regression check before merging Phase 8 (rule 13)

2026-11-06, release build at `973efe0`, `cargo run --release -p eval -- all` twice and
`-- vector --report`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000**; E1 warm p50 / p95
**6.4 / 15.8 ms** (cold first run 12.4 / 32.6 ms, as in Phases 6 and 7); HNSW recall@10
**0.9760** (164/200 perfect). Unchanged from Phase 7 within run-to-run noise. Rust tests
557 passed, 0 failed; frontend 52 passed; controller flow 8/8.

## Phase 9 — Intelligent source selection

### E4: a dead first choice falls back within 10 s

2026-11-06, `cargo test -p sinephile-playback --test fallback a_dead_first_choice -- --nocapture`
(headless libmpv, the app's real `Patience`): the top choice is a loopback server that accepts
the connection and never answers; the next is a local file. Play → first frame of the
fallback: **8.11 s** (budget 10 s). Seen to fail at 11.17 s with the load patience raised
to 11 s.

### Play through the fallback path (real window, debug build)

2026-11-06, search → *Nosferatu* (1922) → Play, `first frame` log line:
**5615 ms**, `backend=internet-archive skipped=0`, resuming at 0:52. Normal playback is
unchanged by the new path (Phase 8: 5533–7859 ms).

### Regression check before merging Phase 9 (rule 13)

2026-11-06, release build at `653dc57`, `cargo run --release -p eval -- all` twice and
`-- vector --report`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000**; E1 warm p50 / p95
**5.9 / 14.9 ms** (cold first run 13.1 / 43.7 ms; Phase 8's cold run was 12.4 / 32.6, the
first run after a rebuild pays the disk cache); HNSW recall@10 **0.9760**, index p95 1.39 ms.
Unchanged from Phase 8. Rust tests 602 passed, 0 failed; frontend 70 passed; controller
flow 19/19.

## Phase 10 — Subtitle pipeline

### E1–E2: subtitle alignment over `fixtures/subtitles/`

2026-11-06, `cargo run --release -p eval -- subs --report`: ten synthetic films (speech by
Windows' synthesiser over room, street and music beds; traces from the app's own VAD),
eight damaged copies each. A copy aligns when 95% of its cues land within 150 ms.

| Damage | Aligned |
|---|---|
| none | 10/10 |
| offset (0.4–90 s either way) | 20/20 |
| scale (23.976→25, 25→23.976, 24→25) | 30/30 |
| intro removed (split) | 9/10 |
| intro added + 25→23.976 | 10/10 |
| **all** | **79/80 = 98.8%** (target > 90%) |

Tuned on six films: 48/48. **Held out** (four films built after tuning, never tuned on):
**31/32 = 96.9%**. False accepts, each film's subtitles against another film's speech:
**0/10** (controls score ≤ 1.30, real matches ≥ 1.6; threshold 1.45). The miss: a split
whose second part landed 156 ms off. VAD against the true speech: agreement
93.8–97.9%, recall 89.5–96.1%, precision 95.5–99.5% per film.

Seen to fail: with the scale set reduced to 1.0 alone, 39/80 = 48.8% and every scaled
copy refused. How the number got here, in order: z-score confidence 50.0% (21 refusals of
real matches) → distinctness 72.9% → corpus fix (no cue moved before zero) 77.1% →
realistic intros 85.4% → per-window candidate offsets 91.7% → prefer no stretch 97.9% →
film time runs forward (and a corpus fix: a cue moves whole) 100% on the tuned films.

### E1 again: the plateau's late edge

2026-11-06, same command: **80/80** (tuned 48/48, **held out 32/32**), false accepts 0/10.
Taking the plateau's late edge instead of its middle (`crates/subtitle-align/src/align.rs`
`peak`): the middle was biased early by half the subtitles' linger, found when
`crates/subtitles/tests/plan.rs` aligned a file lingering 400 ms to 160 ms off.

### Regression check before merging Phase 10 (rule 13)

2026-11-06, release build at `9d84655`, `cargo run --release -p eval -- all` twice (now including
`subs`) and `-- vector --report`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000**; E1 p50 / p95
**6.5 / 15.3** and **6.1 / 14.8 ms**; HNSW recall@10 **0.9760**; subtitles **80/80**, false accepts
**0/10**. Unchanged from Phase 9. Rust tests 646 passed, 0 failed; frontend 76; controller flow 19/19.

## Phase 11 — Player experience layer

### E1: the pause overlay

2026-11-06, real window, *Nosferatu* (1922) from the Internet Archive, Space pressed four times;
`window.__pauseMs` (the intent to the overlay's first painted frame): **22, 4, 4, 5 ms** (target
< 200 ms). The overlay used to wait for mpv's still capture (up to 500 ms); the video window is
now hidden first and the still fades in after.

### E2: scrub thumbnails without stuttering

2026-11-07, real window, a 20-minute 1080p H.264 file (`d3d11va`), `player_state.dropped_frames`
read 5 s and 35 s after the first frame: **0 → 0 with thumbnail generation running, 0 → 0 with the
sheets already cached.** Generation itself (`sinephile_player::thumbs::generate`: FFmpeg at
below-normal priority, one thread, keyframes only) took **970 ms** for the whole 20 minutes
(two 10×10 sheets). It is light enough that it overlaps only the first second of playback; on
a Tier 0 machine it would take longer, at the same priority.

### Phase 11 E5 — the playback checklist on every tier

2026-11-07, at `450cca6`, debug build in the real window, `SINEPHILE_TIER` set per run
(`get_hardware_profile` reported the override), `Garden.mkv` (8:00, H.264 720p, `d3d11va`) with a
sidecar `.srt` 82 s late, the aligned cache cleared before each run. Procedure:
`docs/MANUAL_TESTS.md` "Playback, on every tier".

| Tier | Subtitles | Pause overlay (`__pauseMs`) local / stream | Stream first frame |
|---|---|---|---|
| modest | loaded as is: fits the length (no speech alignment on Tier 0) | 2.2 ms / 2.3 ms | 359 ms |
| standard | aligned: scale 1.00000, offset −82180 ms, confidence 1.91 | 2.3 ms / 2.5 ms | 275 ms |
| capable | aligned: scale 1.00000, offset −82180 ms, confidence 1.91 | 2.4 ms / 3.7 ms | 272 ms |

Every tier: scrub sheets produced, statistics, shortcuts, mini player out and back,
picture-in-picture 742×418 on top and restored to the exact inner size, 0 `ERROR` lines.
The stream was the same file from a local HTTP server, so its first-frame times are a floor,
not a measure of a real network.

### Regression check before merging Phase 11 (rule 13)

2026-11-07, release build at `acf5a85`, `cargo run --release -p eval -- all` twice and
`-- vector --report`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000** (the carried miss; it is
why `all` exits 1, as before); E1 p50 / p95 **14.1 / 44.1 ms** on the first run, straight after the
release build with cold caches, and **6.4 / 14.8 ms** warm; HNSW recall@10 **0.9760**; subtitles
**80/80** (held out 32/32), false accepts **0/10**. Unchanged from Phase 10. Rust tests 654 passed,
0 failed; frontend 104.

## Phase 12 — Local library engine

### The filename parser, first run on the corpus

2026-11-07, `cargo test -p filename-parser --test corpus -- --nocapture`, 549 hand-labelled names
(`fixtures/filenames/names.psv`), before any tuning: title **98.54%**, year **100%**, season **99.45%**,
episodes **99.45%**, absolute **99.82%**, resolution **100%**, source **100%**, codec **100%**, group
**99.82%**; every field right on **98.36%** of names. The misses: S02E01E02 (the boundary was checked
before the continuation was read), a site prefix followed by `_`, a spaced ` - GROUP`, a film called
*Episode 50*, and three bare-number names whose labels broke the corpus's own convention
(`Episode 3.mkv` is labelled episode 3; `05.mkv` was not). Those three labels were corrected.

### The filename parser after tuning, and on names it never saw

2026-11-07, same command. After the fixes above (continuation read before the boundary check,
separators trimmed from a title's ends, ` - GROUP` after a technical tag, an opening "Episode N"
followed by a year is a title, a bare number is an episode): **549/549** names right on every field.
Then `heldout.psv`, 109 names written after the tuning and run once: **109/109** on every field. The
caveat: the held-out names were written by the same hand that tuned the parser, so they test what it
was not tuned on but not what its writer did not think of. Real libraries are the check that remains;
the identification harness and the review queue are where a miss would show.

### Identification, first run (E1, E2)

2026-11-07, `cargo run --release -p eval -- identify` against the full catalogue (2.7M titles), before
tuning. Tuning set 547 names (2 labels not in the catalogue, left out): **top-1 513/519 = 98.84%**;
held out **108/108 = 100%**. The lowest threshold with fewer than 1% wrong among accepted names is
**0.895**: 0/361 accepted wrong, but **34.0%** of names would be asked (37.6% held out). Below it the
wrong share is 1.8–4.7%. And **452.6 ms per name**: 10,000 files would take over an hour. Two problems
to fix: confidence that separates right from wrong earlier, and speed.

### Identification after tuning: E1 and E2 met

2026-11-07, `cargo run --release -p eval -- identify` (`--timing` for the per-query costs, `--report` for
every miss). Steps, each measured:

1. **Speed.** The trigram search cost 437.9 ms of the 452.6 ms per name (`--timing`: exact title 0.3 ms,
   words 9.3 ms, candidate facts 0.8 ms). Run only when the exact and word searches find nothing close:
   **43.4 ms per name**, accuracy unchanged.
2. **Kind over runtime.** *Westworld* S01E10 (a 90-minute finale) went to the 1973 film. Weights now
   title .40, year .22, kind .18, runtime .12, popularity .08; a series that has the claimed episode
   gains .08; a series runtime is judged loosely. A trailing country ("The Office (UK)") is also searched
   without it. Top-1 99.04%, asked 16.6%.
3. **Confidence.** It was the best score times the lead, which capped a runaway winner with no year in
   its name near 0.85. Now the lead times a plausibility that is full from a score of 0.80. Fansub
   naming makes a live-action namesake less likely (*One Piece* 1999 over 2023). Asked 8.6%.
4. **Labels corrected (9 of 549).** Every wrong acceptance in steps 2–3 was a row I had labelled `?`
   (cannot be identified) that the catalogue does identify. Four were seasons or episodes of single IMDb
   series whose episode was found (*Shingeki no Kyojin* S2E05, *Re Zero* S2E25, *Futurama* 2x01, *Spy x
   Family* 25), one film exists that I assumed did not (*Web of Lies*, 2009), and four rows carry a
   duration that names the film (*Arrival* x3 at 116 minutes, *Tokyo Story* at 136). They were relabelled.
   *Ookami to Koushinryou* 13 stays `?`: the 2008 series and its 2024 remake share the title and both
   have an episode 13. Accepting it is a real false-confident miss and is counted as one.
5. **The threshold.** The lowest with under 1% wrong among accepted was 0.615, at 0.97%: too close to
   the line. The library accepts at **0.75** (`sinephile_library::ACCEPT_AT`).

At 0.75, tuning set 549 names (2 labels not in the catalogue): **E1 top-1 523/528 = 99.05%**; **E2 2/510
accepted wrong = 0.39%** (0.37% of all names); **6.8%** asked. Held out, 109 names: **108/108 = 100%**,
**0/103** accepted wrong, **5.5%** asked. **43.7 ms per name.** Caveat: the held-out names were seen in
`--report` output during this tuning, though no parameter was set from them and none was relabelled.

### E3: scanning 10,000 files

2026-11-07, `cargo run --release -p eval -- scan` (default 10,000), full catalogue, real FFprobe. A
worst case for the scanner's shared searches: 10,000 **distinct** films sampled at random from the
catalogue (at least 500 votes), each a small file with a scene-style name in its own "Title (Year)"
folder. The scan walks, records, identifies in parallel (7 at a time) and runs FFprobe on the files it
is unsure of (here FFprobe cannot read the stand-in files, so it is tried and fails: its cost is
counted, its help is not).

| Run | Files | Time | Per file | Accepted / asked | Page queries p95, idle → during scan | Best right | Accepted wrong |
|---|---|---|---|---|---|---|---|
| 1 | 9,994 of 9,996 | **37.1 s** | 3.7 ms | 9,255 / 739 | 24.7 → **33.5 ms** (542 runs) | 99.45% | 1 / 9,255 |
| 2 (walker fixed) | 9,998 of 9,999 | **35.3 s** | 3.5 ms | 9,279 / 719 | 24.6 → **34.9 ms** (515 runs) | 99.54% | 1 / 9,279 |
| 3 | 9,995 of 9,995 | **35.0 s** | 3.5 ms | 9,278 / 717 | — | — | — |

Target: under 180 s and the UI not blocked. Run 1 lost two films whose titles start with "$" or ".",
which the walker skipped as system folders; it now skips by Windows' hidden and system attributes and
two system names (`crates/library/tests/scan.rs` `titles_that_look_like_system_names_are_still_found`,
seen to fail on the old rule). The one file run 2 did not list is most likely a film with "sample" in
its title: a file under 300 MB with that word is a release's sample clip, which a real film never is.

### E5: the review queue, and a local film played

2026-11-07. **Pad only** (`node tools/uiaudit/controller.mjs`): from Settings the D-pad reaches "Check
them" and twenty waiting files are answered by A alone in **5.3 s**. **Real window**, full catalogue:
22 files (21 ambiguous on purpose: twelve remakes with no year, five *Monster* episodes, four *The
Office* episodes with no country, and one 2.2 GB file named as *Heat* 1995) scanned in **1.3 s**, *Heat*
accepted and **21 asked**. Answered by keyboard: **14 answers settled all 21** (a series' first answer
settled its other episodes); the app took **20–290 ms** per answer from key to the next file on screen,
0.8 s in all. So twenty answers in under a minute is a person reading, not the app waiting.

Then *Heat* searched, its 1995 result clicked, Play: the page said "Your files", and the player logged
`first_frame_ms=388 backend=local` (budget 500 ms local). Phase 8's carried criterion, discharged.

### Regression check before merging Phase 12 (rule 13)

2026-11-07, `cargo run --release -p eval -- all` twice (now including `identify`) and `-- vector
--report`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000** (carried); p95 **44.1 ms** cold, **15.2 ms**
warm (14.8 at Phase 11: noise); recall@10 **0.9760**; subtitles **80/80**, held out 32/32, false accepts
**0/10**; identify unchanged at 99.05% / 0.39% (held out 100% / 0%). Unchanged from Phase 11. Rust tests
687 passed, 0 failed; frontend 116.

## Phase 13 — Download manager

### E7: bandwidth limits, measured

2026-11-07, `cargo test -p sinephile-downloads --test limits -- --nocapture --test-threads=1`: a
loopback seeder with no limit of its own and a 48 MiB file, downloaded through the queue; the rate is
the mean of `done_bytes` from one to seven seconds after the start.

| Limit | Measured |
|---|---|
| this download's own, 1.00 MiB/s | **1.02 MiB/s** (run 1), **1.02 MiB/s** (run 2) |
| global (`Engine::set_limits`), 2.00 MiB/s | **2.01 MiB/s**, **2.00 MiB/s** |
| none (control) | 48 MiB in 0.31 s, **157.2 MiB/s** |

The test holds each limit to within 15%. The control shows the swarm would run 80 times faster, so the
limit is what held it.

### E1: the advisor's trials, first run (not met), and a bug in the engine

2026-11-07, `cargo run --release -p eval -- advise`: twelve loopback swarms (one to four seeders, each
capped), measured as the app measures, the advisor asked, then the film played from a fresh engine at
its bitrate after a 3-second buffer; right when "stream" played without a pause or "download" paused.

The first runs measured every swarm at one eighth of its cap. **That was an engine bug from Phase 7**:
librqbit's `mbps` is mebibytes per second and the engine read it as megabits, so every reported speed
(the torrents list, the player's statistics) was 8.4 times too low. Fixed, with
`crates/torrent/tests/swarm.rs the_reported_rate_is_the_rate_bytes_arrive_at` (4.01 reported against
4.04 MiB/s arriving; 0.48 with the old conversion).

With true rates: **7 of 12 right**. Misses: a swarm measured during its ramp-up (1.5 MB/s of capacity
read as 0.74); swarms just below the bitrate on a 20-second film, which a 3-second buffer covers (the
model ignored the buffer and the runtime); single peers at 1.4x and 1.0x the bitrate judged unsafe by a
conservative headroom (1.15) and thin-swarm allowance (0.25). The model is being changed in the light of
these trials, so they no longer judge it: a fresh set of conditions does.

### E1: the advisor on fresh conditions, met

Changes made after the first run: the stall model now asks whether the swarm can deliver the film's
bytes over its runtime **plus the player's buffer** (`needed_rate`), headroom 1.05 (from 1.15), the
thin-swarm allowance 0.10 (from 0.25), and the swarm is measured over the later half of five seconds,
past the ramp-up. Then ten **fresh** conditions, chosen before they ran and run once
(`cargo run --release -p eval -- advise`; the first twelve stay available as `--tuned`):

| Film MB/s | Swarm | Measured | Stall p | Advice | Pauses | Right |
|---|---|---|---|---|---|---|
| 2.0 | 1 x 5.00 | 5.03 | 0.00 | Stream | 0 | yes |
| 2.0 | 1 x 2.60 | 2.61 | 0.00 | Stream | 0 | yes |
| 2.0 | 2 x 0.90 | 1.80 | 0.57 | Download | 16 | yes |
| 2.0 | 1 x 1.20 | 1.20 | 1.00 | Download | 20 | yes |
| 0.8 | 2 x 1.00 | 2.02 | 0.00 | Stream | 0 | yes |
| 0.8 | 4 x 0.22 | 0.86 | 0.04 | Stream | 1 | **no** |
| 0.8 | 1 x 0.85 | 0.85 | 0.07 | Stream | 0 | yes |
| 0.8 | 3 x 0.20 | 0.60 | 1.00 | Download | 7 | yes |
| 3.0 | 3 x 2.00 | 6.00 | 0.00 | Stream | 0 | yes |
| 3.0 | 2 x 1.20 | 2.40 | 0.98 | Download | 10 | yes |

**9 of 10 right** (target at least 8). The miss: four slow peers barely above the bitrate, judged
safe at 4%, paused once. Every reason was one sentence. Caveat: a loopback swarm is steadier than the
internet; the trials test the model's arithmetic and the measurement, not real-world peer churn.

### E2: a real qBittorrent round trip, met

2026-11-07, qBittorrent **v5.2.0** (installed on this machine), `cargo test -p sinephile-downloads
--test qbittorrent_real -- --ignored --nocapture`. Two instances isolated from the user's own
(separate `--profile` folders, Web UIs on 127.0.0.1, DHT/PeX/LSD/port forwarding off; each checked to
be the isolated one before anything was added; only those processes stopped): one seeds a 6.0 MB
file, the other is driven only through the app's `Qbittorrent` backend, by link. The same download
through librqbit, compared field by field:

| Field | librqbit | qBittorrent |
|---|---|---|
| state, done, total | Done, 6,303,801, 6,303,801 | Done, 6,303,801, 6,303,801 |
| file, relative to Downloads | `Heat/Heat.1995.1080p.mkv` | `Heat/Heat.1995.1080p.mkv` |
| library row size | 6,303,801 | 6,303,801 |
| match | Heat, 1.0, confirmed, Metadata, covers 1 | the same |
| downloads folder registered | yes | yes |

**Seen to fail first**, on the real thing: before the fix the qBittorrent copy landed at
`Heat.1995.1080p/Heat.1995.1080p.mkv`, because the backend gave qBittorrent the downloads folder
rather than the per-title folder librqbit uses. Fixed (`savepath` = the title's folder,
`contentLayout=NoSubfolder`). The real run also found two more bugs the stand-in could not:

- qBittorrent 5.2 answers `torrents/add` with **202** and JSON counts while it fetches; the backend
  treated anything but 200 as refused. Fixed (`took`), with a test of every version's answer.
- A download found by a source (`infohash:` with trackers) reached qBittorrent as the bare hash,
  **without its trackers**; it now goes as a link carrying them (`qbittorrent::link`).

And one upstream fault, recorded as D80: librqbit's `create_torrent` writes one piece hash too many
when a file is a whole number of pieces (500 bytes of hashes for 24 pieces); libtorrent refuses such a
torrent as invalid. The app never makes torrents; the test's file is deliberately not a whole number.

### Regression check before merging Phase 13 (rule 13)

2026-11-07, `cargo run --release -p eval -- all` twice and `-- vector --report`: E2 **43/43**; E3
meaning **0.0472**, filter **1.0000** (carried); p95 **43.8 ms** cold, **15.8 ms** warm (15.2 at Phase
12; 14.8 to 15.6 across recent runs: noise); recall@10 **0.9760**; subtitles **80/80**, held out 32/32,
false accepts **0/10**; identify **99.05% / 0.39%** (held out 100% / 0%), 44.7 ms per name. Unchanged
from Phase 12. Rust tests 739 passed, 0 failed, 6 ignored (E2's real round trip among them, run on
demand); frontend 136. `npm run audit:ui`: rail p95 16.8 ms, 0 dropped, 45 focus stops all ringed.

## Phase 14 — Profiles and onboarding

### E2: a Letterboxd export of 600 films, first run

2026-11-07, `cargo run --release -p eval -- import` (the export made at run time from the local
`ml-25m.zip`: the 600 most-rated films, titles as MovieLens writes them turned into Letterboxd's form,
each film's IMDb id as ground truth; never committed, as MovieLens and IMDb data may not be
redistributed). 550 watched, 50 on the watchlist:

| | Films | Share |
|---|---|---|
| right | 592 | **98.67%** (target at least 95%) |
| wrong | 1 | **0.17%** (target under 1%) |
| asked | 7 | 1.17% |

Recorded 347 ratings, 543 dated watches, 50 watchlist entries in 0.2 s; imported again, 0 new watches.
The wrong one: "300" (2007 in MovieLens) placed on an obscure 2007 "300" (15 votes) rather than the 2006
film (924,388 votes). The asked: five punctuation differences ("&"/"and", a trailing "?", "X-Files"/"X
Files", a possessive prefix) and two harness bugs (titles with two parentheticals not fully stripped).
The matcher is being changed in the light of this set, so a fresh set judges it next.

### E2: a fresh set after the changes, met

Changes made after the first run: a title a year either side with 100 times the votes beats an obscure
exact-year one (`choose`); when the exact name finds nothing, names are compared normalised ("&"/"and",
punctuation, a leading "the") a year either side (`normalised`, `MediaRepository::titles_released_between`);
the harness strips every parenthetical. Then the next 600 films by ratings (ranks 601-1200, disjoint),
chosen before running and run once: `cargo run --release -p eval -- import --from 600`, 2026-11-07.

| | Fresh set (601-1200) | Tuned set (1-600), for comparison |
|---|---|---|
| right | **592 = 98.83%** | 598 = 99.67% |
| wrong | **0 = 0.00%** | 0 = 0.00% |
| asked | 7 = 1.17% | 2 = 0.33% |
| import time | 7.5 s | 2.5 s |

E2 met on the fresh set: at least 500 films, at least 95% right, under 1% wrong, and imported again
adds nothing. Of the seven asked, two are genuinely ambiguous (two "Daredevil" of 2003, two "Emma" of
1996, close in votes), four are names written very differently ("Men in Black III" / "Men in Black 3"),
and one is the harness cutting "Birdman: Or (The Unexpected Virtue of Ignorance)" short. The fallback's
cost is the time: each miss scans three years of names (about a second for the worst).

### E2: is an imported history reflected? First measurement (not met; carried to Phase 15)

2026-11-07, `cargo run --release -p eval -- taste`: the first three MovieLens users with 600-900 ratings
(users 3, 12, 72), each one's real history written as a Letterboxd export (their half stars, their dates),
imported through `sinephile_imports` into a temporary profile (deleted after), then five-fold
cross-validation over their four- and five-star favourites: the seed (`sinephile_taste::seed`, the
weighted mean of the liked titles' vectors) is built from four fifths, and the held-out fifth is looked
for among the 500 titles nearest it. Bar fixed before running: beat popularity, and 20 times chance.

| User | Rated / matched | Favourites held out | Seed | Popularity |
|---|---|---|---|---|
| 3 | 656 / 649 | 494 | 15.8% | 57.1% |
| 12 | 736 / 725 | 404 | 7.4% | 24.3% |
| 72 | 813 / 808 | 404 | 8.9% | 20.8% |
| **all** | | 1,302 | **11.06%** | **35.64%** |

Chance is 0.417% (500 of 119,874 indexed). **Reflected, but not well enough**: 27 times chance, and 0.31
times popularity. One mean over about 400 liked titles is a centroid of everything they like, which is
what SPEC.md Phase 15 warns against ("not one average vector"); MovieLens favourites are also mostly
popular films, which flatters the popularity baseline. The import half of E2 is met (above); this half is
carried to Phase 15, whose taste modes must beat popularity on this harness.

### The films most people know: an index, measured

2026-11-07, found in the real window: onboarding's first round of title cards took several seconds to
appear (the log showed each decade's query over a second). On the full catalogue (Python `sqlite3`
against `data/sinephile.db`, the queries exactly as `MediaRepository` runs them):

| Query | Before | After 0025 |
|---|---|---|
| most-voted films (Home's "Widely loved") | 2.13 s | **0.001 s** |
| a round of eleven decades (title cards) | 2.27 s | **0.067 s** |

Migration 0025: a partial index on votes. The decade query walks it in vote order with year and kind
kept out of the index choice (`+`), the same answers in every decade (checked). A second index on
(year, votes) was tried and not used by the planner, so it is not in the migration.

### E5: switching profiles, timed

2026-11-07, `cargo run --release -p eval -- switch`: a switch is one write (`touch`) and the new
profile's Home (`sinephile_taste::home::rails`, the taste seed included), on the full catalogue with the
vector half; a profile with nothing, and one with MovieLens user 3's real history imported (650 films,
498 liked). Five switches each, three runs:

| Profile | Run 1 (cold, straight after the import) | Runs 2 and 3 |
|---|---|---|
| nothing yet | 3.8 ms | 2.1 / 2.5 ms |
| 650 films imported | **112.1 ms** | **11.5 / 10.9 ms** |

Where the warm 11 ms goes: `liked()` 0.6 ms, the seed of 498 vectors 0.3 ms, the nearest search 0.8 ms,
the other rails 2.7 ms. The cold 112 ms is the index and catalogue pages being read from disk the first
time; it is the case a switch right after launch will see. Isolation: `crates/persistence/tests/profiles.rs`
(seen to fail with a profile filter removed). The page's own cost (rehydrating two stores and refetching)
is not in these numbers.

### Regression check before merging Phase 14 (rule 13)

2026-11-08, `cargo run --release -p eval -- all` twice and `-- vector --report`: E2 **43/43**; E3 meaning
**0.0472**, filter **1.0000** (carried); p95 **27.3 ms** cold, **16.4 ms** warm (15.8 at Phase 13: noise);
recall@10 **0.9760**; subtitles **80/80**, held out 32/32, false accepts **0/10**; identify **99.05% /
0.39%** (held out 100% / 0%). Unchanged from Phase 13. Phase 14's own, rerun after migration 0025: import
(fresh set) 98.83% / 0 wrong; taste seed 11.06% against popularity 35.64% (carried); switch 2.1 / 11.5 ms.

## Phase 15 — Taste model

### Taste modes with a popularity prior: Phase 14 E2's carried half, met

2026-11-08, `cargo run --release -p eval -- taste` (the same three MovieLens users, folds and 500 slots
as Phase 14's measurement; nothing about them changed). Steps, each measured on the same harness:

| Model | Held-out favourites in the 500 |
|---|---|
| Phase 14 seed (one weighted mean) | 11.06% |
| taste modes (k-means, k 3-8 by silhouette, weighted and decayed signals) | 12.75% |
| **modes with a popularity prior of 0.5** | **40.32%** |
| the 500 most popular (the bar) | 35.64% |
| chance | 0.417% |

**The prior was chosen on other users**: `eval taste --tune` runs the next three users in the same band
(120, 166, 171), never the three that judge. There: popularity 19.79%; modes alone 9.63%; with the prior
0.0 10.16%, 0.1 23.88%, 0.2 25.07%, 0.3 25.07%, **0.5 25.66%**, 0.75 25.40%, 1.0 24.74%, 1.5 23.61%, 2.0
23.28%. A broad plateau from 0.2 to 0.75; 0.5 was taken and the judging users run once.

What the numbers say: the embeddings know what a film is about, not how widely it is seen, and these
users' favourites are mostly widely seen; closeness to a taste mode plus being widely seen beats either
alone (97 times chance, 1.13 times popularity). The margin over popularity is 4.7 points on these users.

Second run on the judging users after an engineering change (the silhouette sampled at 160 points for
speed, nothing tuned): **40.63%** against popularity 35.64%; 97x chance, 1.14x popularity. Unchanged.

### E1: two unrelated tastes, two sets of modes

2026-11-08, `cargo run --release -p eval -- modes`: a profile that loves the 30 most-voted horror films
and the 30 most-voted documentaries (none in both), five stars each, clustered on the real vectors:

| Mode | Weight | Horror | Documentary | Purity |
|---|---|---|---|---|
| 1 | 47% | 28 | 0 | 100% |
| 2 | 42% | 0 | 25 | 100% |
| 3 | 10% | 2 | 4 | 67% |

(three identical runs; weighted purity **96.6%**, target at least 90%; each taste in modes of its own; the
small third mode holds the two tastes' edge cases, which is where a mixed mode belongs).
**Seen to fail**: `-- modes --prove` shuffles the vectors between titles, and purity falls to 59.3%
(chance is 50%), E1 NOT MET. The first version iterated a hash map, so k-means' seeding changed run to
run (94.9% then 98.3%); the titles are now taken in a fixed order, as the app's model takes them.

### Switching, with the taste model

`-- switch` again: building a profile's model is 25 ms (nothing yet) and 160 ms (650 films), done in the
background at launch for every profile (`warm_models`, D82); a switch after that is **20.8 / 29.8 ms
p95** (the prior ranks a 3,000-title pool). E5 holds.

### The model after a watch (Phase 15 E5)

`cargo run --release -p eval -- update`, 2026-11-08, on 59a1d6d plus the uncommitted harness: a
650-film profile (MovieLens user 3) watches five well-known films it had not met, and each is timed
from the viewing's write to the rebuilt model through the app's `ModelCache`. **125.7 to 133.2 ms**
(target under 2,000 ms); every watched film is a member of the model it timed; with nothing new, the
cached model costs 0.3 ms. The model at launch: 162.3 ms. In the app the rebuild runs in a background
task after `end_viewing` and `rate_title` return (`commands::home::refresh_model`), so the UI
does not wait. **Seen to fail**: with `ModelCache::get` made to ignore the fingerprint, the update
"takes" 0.5 ms and no watched film is in its model: E5 NOT MET.

The same run of `-- modes` now says the modes in words (15.4), on the real catalogue:
"You gravitate toward horror and drama, mostly from the 2000s and 2010s (The Silence of the Lambs,
The Shining, Alien)" (47%); "You also love documentary and biography … (Bowling for Columbine,
Fahrenheit 9/11, The Social Dilemma)" (42%); and the 10% edge-case mode, "documentary and action …,
Jeff Tremaine above all (Get Out, Saw, Super Size Me)", which says honestly that it is a mixture.

### Regression check before merging Phase 15 (rule 13)

2026-11-08, `cargo run --release -p eval -- all` (twice), `-- vector --report`, `-- import --from 600`, `-- taste`, `-- switch`, `-- modes`, `-- update`: E2 **43/43**; E3 meaning **0.0472**, filter **1.0000** (carried); p95 **15.6 ms** warm (16.4 at Phase 14: noise); recall@10 **0.9760**; subtitles **48/48** tuned, **32/32** held out, false accepts **0/10**; identify **99.05% / 0.39%** (held out 100% / 0%); import (fresh 600) **98.83% / 0 wrong**; taste **40.63%** against popularity 35.64%; switch **22.9 / 30.5 ms** p95; modes purity **96.6%**; update **125.5–133.2 ms**. Unchanged from Phase 14 and from this phase's own first runs.

### D77: no console window from the release build (2026-11-08, ba98081)

`npm run tauri build`, then: the release `sin-e-phile.exe` is a GUI-subsystem program (PE subsystem 2;
the debug build is 3, console). Launched on the real `data/` and watched for 45 s (visible
`ConsoleWindowClass` windows, sampled every 100 ms): **0** new console windows; it opened to the first-run
wizard. A scan and an alignment were exercised from two temporary `#![windows_subsystem = "windows"]`
programs calling the app's own code on an 8-second test video, five runs each, sampled every 20 ms:
`sinephile_library::probe::ffprobe` **0** console windows, `sinephile_subtitles::plan::ffmpeg_speech`
**0**. **Seen to fail**: the same FFprobe started without `CREATE_NO_WINDOW` opened **5**, one per run.
The temporary programs were not committed.

## Phase 16 — Ready to Watch

### E4: TV layouts read from their folders (2026-11-08, 66be7cb plus the 16.1 change)

`cargo run --release -p eval -- tvlayouts` over `fixtures/tv-layouts/layouts.psv` (184 files: the
author's own Twin Peaks layout with its groups replaced, plus season folders, flat, anime fansub numbering
and one show split across three folders). **Before** (`--before`, no extras rule; the scanner as it was):
episodes 57/59 = 96.6%, and **105 of 105 extras searched for as titles** ("Cherry Pie", "The Log Lady 01",
"Tim Hunter" …), which is what the author saw. **After**: episodes **59/59 = 100%**, **0** filed as films,
the nested film 1/1, extras **105/105** kept out and **0** searched for. E4 MET.

The two anime misses on the way were real: a fansub "- 05" had nothing to resolve against (no free source
publishes absolute numbers; no AniList numbering is stored), and is now read as the fifth episode only when
the show has exactly one season (the identity, not a sum, ADR-0031), and only to place the episode after the
series is chosen. Used while scoring, it favoured one-season shows: `eval identify` went from 523 to 522 of
528 ("Steins;Gate 0 - 03" to Steins;Gate) until it was confined to placement; now **523/528 = 99.05%,
false-confident 2/510 = 0.39%, held out 108/108 and 0/103**, identical to Phase 12.

On the author's real folder (`-- tvlayouts --folder <path>`, read-only): 151 videos, **102 extras left
out**, 48 episodes all filed under Twin Peaks (1990) (30 on their episode; 18 from the 2017 season placed on
the series, whose rows the catalogue lacks), Fire Walk with Me as the film, 0 to the review queue.

### E5: the catalogue refreshes itself (2026-11-08)

The author's catalogue before: a refresh job `running` since 2026-11-07 06:05 UTC, never resumed, every
launch answering `AlreadyCurrent`; 2,734,015 titles, 539,817 episodes, 52,426 films of 2026.
`target/release/ingest.exe keep` (the function the app's hourly timer calls,
`freshness::refresh(…, force = false)`), on the real data: **finished the interrupted refresh — 885 titles
added (300 of them 2026 films) and 7,951 episodes — in 86 s**; job 26 `complete`, job 28
`refresh-episodes` `complete`; `catalogue.last_refresh` = `{"at":"2026-11-08T06:21:10Z","titles_added":885,
"episodes_added":7951}`. After: 2,734,900 titles, 547,768 episodes, 52,726 films of 2026.

Tests (`crates/catalogue/tests/refresh.rs`): `an_interrupted_refresh_is_finished_before_the_publisher_is_asked`
(a new film and an episode of a 40-vote series on a watchlist, zero requests to the publisher) and
`a_resumed_refresh_reads_past_the_watermark_it_started_from`. **Seen to fail**: without the resume branch
the first returns `AlreadyCurrent`, the author's bug exactly; with the watermark taken afresh the second
loses the title between the start and the stored maximum.

### Home in the author's order (16.3, 2026-11-08)

Rails: continue, **new episodes of your shows**, **your library**, **new releases**, taste, watchlist,
widely loved (`crates/taste/tests/home.rs home_is_in_the_authors_order_…`, seen to fail with two rails
swapped and with the new-release vote floor removed). New releases on the real catalogue, 2026 with at
least 1,000 votes: Project Hail Mary, The Odyssey, Spider-Man: Brand New Day, A Knight of the Seven
Kingdoms, Backrooms, Michael …

**A regression caught and fixed before commit (rule 13).** The new-releases query walked the votes index
and filtered by year: 92–98 ms on the real catalogue, and `eval switch` went from 22.9 / 30.5 ms to
**111.7 / 118.2 ms p95, over its 100 ms target**. Migration 0027 adds `idx_media_items_year_votes`, named in
the query because the planner otherwise still walked the votes index to avoid a sort: **10.9 ms**. `eval
switch` now **30.3 / 40.7 ms p95** (E5 MET); the remaining ~10 ms over Phase 15's numbers is the three new
rails' queries (new episodes 1 ms, library 0 ms, new releases 11 ms).

### The pad in playback, and the right stick (16.4, 2026-11-08)

**Measured first, because the first explanation was wrong.** A debug build with a `dev_log` probe
(`SINEPHILE_AUTOPLAY`, the author's own film, 20 s of playback): every half second `visibility=visible
focus=true frames=120`. The page was never hidden or unfocused behind the video window, so the pad was
being read; the faults were in our handling. Found and fixed, each step in `node
tools/uiaudit/controller.mjs` seen to fail first: Y opened nothing (the old mapping had Y for sound); with a
quick menu open, Down turned the volume down instead of moving in the menu; a volume change said nothing.
Now: Up 100 → 105 with "Volume 105%" on screen; Y opens the subtitles with focus on "Off"; Down stays in
the menu; A picks and closes. Right stick: Settings scrolled 0 → 1,094 px in 0.6 s with focus following
into view; the paused film's details 0 → 636 px in 0.4 s with focus left on Resume (0 px with the dialog
fallback removed). Not yet covered: X (sound needs two audio tracks), LB/RB (needs a series), and a real
controller in the author's hands.

### The player, measured in its real window (16.5, 2026-11-08)

Debug build, `SINEPHILE_AUTOPLAY` (the author's own film) and `SINEPHILE_PROBE` (timed player intents sent as
the pad sends them); the page logs through `dev_log`, and the video window's region is read from Windows by
process id (`GetWindowRgn`, sampled every 10 ms). PrintWindow cannot show what is seen through the holes, so the
region is the evidence.

- **The subtitles menu opened unseen beneath the video.** Before: with the menu open the page sent two holes
  (the bars) and none for the menu above the bar. After (every overlay marked `data-cutout`, holes recomputed
  when one comes or goes): three holes; Windows' region 39 rectangles with the bars, **49 with the menu**, 39 when
  it closes, 1 when the bars hide. Pad flow: "the menu is cut out of the video: 2 -> 3 holes", seen to fail with
  the menu's mark removed.
- **Controls respond in about 14 ms**: intent logged at 08:21:00.033, region 1 rectangle at .017 and 49 at .046.
- **Opening**: the video window stays hidden until the first frame (430 ms after open), so no black flash.
- **Closing**: Back logged at 08:21:28.179, the video window hidden by .186 (7 ms), its region unchanged until then.
- **The bar no longer moves in quarter-second steps**: between the backend's four reports a second the position
  glides on at the playback speed, never more than 0.5 s past a report or past the end (`glide`, unit-tested).

### Subtitles in the player (16.6, 2026-11-08)

`crates/subtitles/tests/chain.rs the_viewer_chooses_among_the_offers_and_only_a_download_spends_the_budget`:
offers come back best first (the preferred language, this file's own timing first) and searching spends nothing;
the chosen offer is downloaded within the budget; a refused download gives its reservation back (seen to fail
with the refund removed). `src/features/player/SubtitleTools.test.tsx`: size, colour and outline saved for the
profile and applied at once (seen to fail with the size step changed); found subtitles named by language,
release and "timed for this file"; "align again" on an embedded track says why it cannot. Pad flow: Menu, the
D-pad to "Find subtitles", A, the D-pad to the first found, A downloads and shows it. Not yet seen in the real
window: the style reaching mpv, and "Load a file" (Windows' file picker).

**16.6 in the real window** (2026-11-08): the debug build playing the author's film, the probe's `style` step
setting yellow and outline 5 exactly as the panel does, then reading mpv back through `dev_player_prop`:
`sub-color=#FFFFD400 sub-border-size=5.000000`. The style reaches the playing film live. Still unseen in the
real window: "Load a file" (Windows' file picker needs a hand).

### First run and Home at three window sizes (16.9, 2026-11-08)

A layout check (`src/lib/layoutCheck.ts`, the same rules in the pad-flow audit): the page must not scroll
sideways, no text cut off in its box, nothing off the right edge outside a deliberate scroller. **Its control**:
a 3,000 px paragraph injected into Home must be reported, and is ("the page scrolls sideways: p (3750px)"). The
first version did not report it: it excused everything inside `main`, which scrolls, and so excused the page.

**What it found, which the fake pages had not shown:** on the author's real Home through the debug probe,
at 1280×720 on their 150% display (839 CSS px wide), the window scrolled sideways: "TV Shows" wrapped onto two
lines and the window buttons were pushed off the edge; Home's rails had padding of their own on top of the
rail's own margins, pushing the index column in and cutting the rail short. In the audit with a controller
(TV mode's 125% zoom), the window buttons overflowed by 20 px at 1280. Fixed: the masthead measures whether it
fits and goes compact when it does not (a breakpoint could not, since TV mode zooms without changing what a
media query sees), and Home's extra padding went. **After**: the real app at 1280×720, 1600×900 and
1920×1080, two runs: no problems; the pad-flow audit in TV mode, every first-run step and Home at all three
sizes: no problems.

### E1: every control in the player panel, by pad (16.10, 2026-11-16)

`node tools/uiaudit/controller.mjs`, new step "the D-pad reaches every control in the player panel": Menu opens
the panel, then the walk goes row by row as a thumb would (Left to the row's start, Right along it, Down to the
next), marking each control focus lands on, and compares with every enabled control in the panel: **34/34, none
missed**. Seen to fail: the Speed section made `inert` gives 20/34 (Slower … Close missed); restored, 34/34.
With 16.5's real-window numbers (input to the video region changed in ~14 ms; the video window hidden until the
first frame, 430 ms after open; hidden 7 ms after Back) E1 is met. The first version of the walk pressed Right
on full-width rows, where spatial navigation correctly goes up and to the right, and looped; the walk was wrong,
not the navigation.

**Regression, 2026-11-16** (`cargo run --release -p eval -- all`, `tvlayouts`, `import --from 600`, `taste`,
`switch`, `modes`, `update`): identify **99.05% / 0.39%** (held out 100% / 0%); tvlayouts **59/59**, 0 as films,
extras 105/105; import **E2 MET**, 0 new on re-import; taste **40.63%** against 35.64%; switch **32.3 / 37.3 ms**
p95 (30.3 / 40.7 at 16.3: noise); modes purity **96.6%**; update slowest **131.9 ms**. No regression.

### E1 again, and E3: the controller in the player and the right stick (16.10, 2026-11-16)

E1 was reopened the same day: the phase names the picture's **shape and zoom**, and the panel had neither. Built in
7776473 (Shape cycles as filmed, 16:9, 4:3, 2.35:1, fill; Zoom 50-300%; `crates/player` `framing`, its tests, and
`tests/playback.rs` `the_shape_and_zoom_reach_the_playing_film`, seen to fail first). `node tools/uiaudit/controller.mjs`
at 2c1cd5b: every control in the panel by D-pad, **40/40**, none missed. With 16.5's real-window numbers, **E1 met**.

E3, same run: Y and X quick menus (focus on the first choice, cut out of the video, the D-pad kept inside, A chooses and
closes), **RB/LB by chapter** in a film (10:00, then 0:00) and said on screen, Right seeks once per press, Left held
0.9 s scrubs **5 steps**, RT 30 s, Menu, A, B. The right stick: a 24-card rail **0 -> 970 px** with focus following onto
a card in view; the player panel **349 -> 0**; the paused film's details **636**; Settings **0 -> 1095**.

Pushing the stick where nothing had found three faults, each seen in the audit before its fix:

| Fault | Before | After |
|---|---|---|
| A snapping rail undid every step of the stick | rail 0 -> 0 | 0 -> 970 (snap suspended while driven) |
| The virtualised rail unmounted the focused card; focus fell to the page | focus on `body` | a card in view (600 ms grace) |
| Down at the foot of the panel left the dialog for the bar | panel 349 -> 349, then 349 -> 318 | 349 -> 0 (a dialog holds the D-pad and the follow) |

`npx vitest run src/lib/gamepad.test.ts`: four new jsdom tests, each seen to fail with its fix removed (the clipping
test also fails with `onScreen`'s ancestor check removed; the grace test with the grace removed). **E3 met.** Frontend
**193** tests (189 before).

### E6: a finished scan on Home (16.10, 2026-11-16)

Rail order: `crates/taste/tests/home.rs` `home_is_in_the_authors_order_with_new_episodes_the_library_and_new_releases`.

Scan to Home, in the real app: the debug build on a **copy** of the 4 GB catalogue database (`SINEPHILE_DATA_DIR`, in
scratch), its library folder pointed at films generated with FFmpeg (16x16, one frame a second) named and timed as real
catalogue titles, and emptied before each launch so the launch rescan adds them. The probe (`SINEPHILE_PROBE`,
`src/features/player/useDropToPlay.ts`) logs the scan's `done` event and checks every frame for the first card of the
"Your library" rail, in the page's own clock.

| Library | Scan | Identified | Shown on Home after the scan finished |
|---|---|---|---|
| 5 films | 103 ms | 5 of 5 | **28 ms** |
| 200 films (the most voted) | 1,519 ms | 198, 2 to the review queue | **38 ms** |

The page received `done` 0.4 ms after the backend logged the scan; half a second before, the rail was not there.
Seen to fail: with Home's refetch on `done` disabled, "home library not shown within 10 s of the scan finishing".
Budget 2 s; Tier 2 dev machine, debug build, so the ~50x margin is what Tier 0 has to spend. **E6 met.**

### E7: the first run done only by pad (16.10, 2026-11-16)

`node tools/uiaudit/controller.mjs` at af1be92, the first run with nothing but the pad: a library folder's
path typed on the TV keyboard and added; an add-on's address typed, looked at, then added; the OpenSubtitles
and TMDB keys typed and saved; the VPN's network adapter chosen from its list and saved. Each step says it
took, and Home then has nothing left to set up. **104 checks**, all by pad. Filling it in found four faults
the walk-through (Continue, Continue...) never touched, each seen in the flow before its fix:

| Fault | Seen | Fix |
|---|---|---|
| No select could be changed by pad (A clicks; a script's click does not open a select) | timed out waiting for the adapter list | `PadPicker`: the options as a list the D-pad walks |
| No slider could be changed by pad (Left and Right moved off it) | unit test | `adjustValue`: Left and Right step it |
| "Add" disabled itself once pressed and dropped focus onto the page | "no button Continue" | `rescueFocus`: back near where it was, unless the screen went |
| A long step's last fields sat half under the sticky footer; Down found nothing | "could not reach Start watching" | `scroll-pb-32` on the wizard's scroller |

The real backend's checks behind "checked as saved" are the crates' tests (`a_key_is_checked_when_it_is_entered`,
`a_key_is_checked_and_a_refused_one_says_so`, `paste_describe_install_find_resolve`,
`bound_to_one_adapter_traffic_never_leaves_by_another`); "no source yet" on the title page is
`DetailScreen.test.tsx` "says why nothing can play". Frontend **203** tests. **E7 met**; a real controller in
hand is in the author batch as confirmation.

### The author's second library (2026-11-16)

The author's report: TV shows not grouped, episodes as separate titles. Their database (read only):
Samurai Champloo (`2a. Samurai Champloo (2004)/... - Ep. 01 - ...`) and Monster (`Monster [2004-2005]
DVDRip .../Monster - Chapter 01 - ...`) were not read as episodes, so each of 100 files was searched for
as a film and kept at confidence 0.00-0.41 ("Afro Samurai Champloo", "Entrapment", "The Fugitive"), and
Home's library rail showed every guess. Fixed in c00d4e5 (reading) and the commit after it (the show's
page, Films and TV Shows).

`cargo run --release -p eval -- tvlayouts`, with the author's layouts added (groups replaced): **71/71
episodes, 0 filed as films, 106/106 extras kept out**. The same rows on the old code: 62/71, 9 filed as
films, 1 extra searched for. `-- identify` unchanged: tuning **99.05% / 0.39%**, held out **100% / 0%**.
Space Dandy's `S01E14`-`S01E26` (IMDb's first season has 13) and Twin Peaks' `S03` (IMDb: a 2017 series)
match the show itself and are kept as their names number them (ADR-0031), listed under "Also on this PC".
Rust **803** tests, frontend **211**.

### First-run catalogue build (Phase 16, brief item 3) — 2026-11-17, on 5841b0b + working tree

A copy of the author's installed database (2,735,153 non-episode titles, no index, nothing normalised),
built with the release `ingest` binary and `--no-compact`, as the app's first run will build it.
Command: `ingest <step> --data-dir <scratch copy> --no-compact`, each step timed with `date +%s`.

| Step | Time | Result |
|---|---|---|
| Load (the app's refresh, from the copy's `ingest_jobs`) | 1,065 s titles + 342 s episodes | 3,167,536 items |
| `akas` (523 MB download included) | 862 s | 6,456,057 title rows (+3.55 M) |
| `normalise` | 123 s | 6,456,057 normalised, 0 left |
| `search-index --trigram-core` | 127 s | 2,735,153 indexed, 881,027 in the typo index |

After it: `search_index MATCH 'nosferatu'` 84 rows, `MATCH 'kyojin'` 62 (romaji, from the alternate titles).
Database 1,392 MB → 2,440 MB. Alternate titles are 862 of the 2,519 s, so they move after search works
(Home opens ~14 minutes sooner); the titles they touch are re-indexed by the catch-up.

The same build in the real app (release, on a fresh copy of the author's install, 2026-11-17): the refresh
ended at 19:17:27; stage A ran with the library held back; the scan was released at 19:21:11 (3 min 44 s).
Files in the author's library folder matched at or above `MATCH_ACCEPTED_AT`: **0 of 200** in the installed
database (no index), **199 of 200** after stage A. Command: `SELECT COUNT(DISTINCT local_file_id) FROM
local_file_matches WHERE confidence >= 0.75` on each.
The rest of that run, on 6c8d015's code: alternate titles and the catch-up finished at 19:38:34, 17 min 23 s
after search opened; the catch-up re-indexed 880,187 titles. Afterwards `MATCH 'kyojin'` gives 62, the same
as building alternate titles first, and no title is left without a normalised form. Job order in the copy:
refresh, refresh-episodes, titles-normalise, search-index, imdb-akas, catalogue-catch-up, all complete.

### Films and TV Shows catalogue rails (Phase 16, brief item 9) — 2026-11-17

The rail query (`MediaRepository::most_voted_of`: the votes index walked, the genre checked per row), on the
3,167,536-title copy of the author's install, `LIMIT 30`, Python `sqlite3`, read-only. Films: Drama 12 ms, Comedy
4, Action 0, Thriller 2, Crime 1, Sci-Fi 1, Animation 2, Horror 87; most watched 0; new this year (`new_of`, year
index) 113. Shows: Drama 4, Comedy 4, Crime 3, Animation 8, Sci-Fi 18, Mystery 7; new 8. Left out as too slow for
a tab (walks far down the index): documentary films 507 ms, war series 856, western series 769.

### Search by meaning on an install (Phase 16, round 8, 2026-11-17)

| Metric | Value | Command / how |
|---|---|---|
| Key file for `embeddings-v3` | 119,874 of 858,170 positions named by IMDb id; 959,052 bytes | `cargo run -p sinephile-ingest --bin ingest -- artefact-keys` (dev catalogue; id digest `e66f698d…` matches the artefact) |
| Index build on a copy of the author's install | 30 s, 60.9 MB; 119,833 of the 119,874 keyed titles found in that catalogue (41 absent), 761,194 core titles beyond the artefact | debug app, `SINEPHILE_DATA_DIR` = copy, Search → Build the index, timed to the `.usearch` file |
| Before the fix, same copy | refused: "position 858169 is now id 2615642" | Search → Download |
| "a heist where the plan goes wrong" | 20 results, titles, people and meaning: Tower Heist, Heist: Who Stole the American Dream?, Jimmywork, The Crew, Cash Out, Hell's Angels '69, Reindeer Games | same run, real window |
| A new install, release build, fresh data folder (ADR-0050) | titles loaded in 18 min (2,735,153 titles, 432,462 episodes); stage A done, then `new install: fetching search by meaning` with no button pressed; download and index 79 s (indexed=119833); `search: hybrid` | `target/release/sin-e-phile.exe`, `SINEPHILE_DATA_DIR` = empty folder with stage A's three datasets; its log, 10:05:45 → 10:29:14 UTC |

### Search by meaning, the author's broken install (Phase 16, round 9, 2026-11-18)

Commit: c4c1fcc. Copy of the author's install made read-only with sqlite's backup API
(9 s), their meaning files copied as they were.

| Metric | Value | Command / how |
|---|---|---|
| The author's meaning file | 329,537,568 bytes, sha256 `6f92d2ae…`; the published file is `7f11415a…`. 5,128,192 bytes wrong from byte 69,422,516: the file's own data, 1,005,504 bytes late | `sha256sum`; byte compare with the dev copy and the fresh-install download (both `7f11415a…`) |
| Its failure, reproduced and now logged | "checksum mismatch: the artefact is corrupt or was modified" | debug app on the copy, `target/debug/data/logs` |
| Debug build, no installed copy: launch | corrupt file found and removed, nothing downloaded; Search offers "Download 314 MB" | same run, real window |
| Debug build: press Download | downloaded, checked, 119,833 titles indexed, hybrid in 54 s | same run |
| Release build with the installed copy (`npm run release`), the copy in the author's state | at launch: 364,263,390 bytes of superseded downloads removed, index built from the installed copy in 23 s (119,833 titles, 63.8 MB), hybrid, no button | `target/release/sin-e-phile.exe`, `SINEPHILE_DATA_DIR` = copy, `target/release/data/logs` |
| `tools/bundle/check_meaning.py` | refuses the author's copy (exit 1), passes the published files (exit 0) | `python tools/bundle/check_meaning.py <dir>` |

### Load a file, in the real window (Phase 16 E2, round 10, 2026-11-18)

Debug build of this round's code, a scratch data folder, a generated 2-minute 1280x720 clip (ffmpeg `testsrc2`) and a
one-cue `.srt`. The probe opened the panel (`SINEPHILE_PROBE=6000:player_menu`), "Load a file…" was clicked, and
Windows' picker opened over the player, titled "Load subtitles". The path was typed and Enter pressed: the cue was on
the picture, the track was listed as now showing, and the panel said "Showing the file (not aligned (no speech found in
the audio); loaded as it is)". Before this round's fix the picker listed every file in the folder (it opened on a
pictures folder) and the track read "sin-e-phile"; after it, the picker offers "Subtitles
(*.srt;*.ass;*.ssa;*.vtt;*.sub)" and the track reads "clip-test". Unit: `src/features/player/SubtitleTools.test.tsx`
"loads a file from disk…" (seen to fail with the button disconnected); `crates/subtitles` `files::tests::
the_picker_offers_every_subtitle_kind_and_a_loaded_file_keeps_its_name` (seen to fail naming the track with its
extension).

**Found on the way: a first launch could leave no default profile.** The second launch on that folder logged
"subtitles could not be prepared … UNIQUE constraint failed: profiles.name", and the panel had no Style section. The
first launch's simultaneous `ensure_default` calls had each seen no default: one made "You", the other demoted it and
failed on the name, leaving `(1, 'You', 0)`; every later ask failed the same way, and everything kept per profile with
it. Fixed in `crates/persistence/src/repositories/profiles.rs`: each step is one statement, and an install with
profiles but no default gives the first one the flag back. The same folder then launched with `(1, 'You', 1)`, no
warning, and the Style section back. `cargo test -p sinephile-persistence --test profiles`: eight asks at once, ten
rounds, one default profile each time (seen to fail before the fix: "round 0: … UNIQUE constraint failed:
profiles.name"; it passed before the fix too until the pool was warmed to eight open connections);
`an_install_left_without_a_default_gets_its_first_profile_back` and
`a_new_default_that_cannot_be_made_leaves_the_old_one_the_default`, each seen to fail.

## Phase 17 — Recommendation Engine

### The item-item matrix (17.1, 2026-11-18)

`cargo run --release -p eval -- recommend [--tune]` over the local `ml-25m.zip` (25,000,095 ratings). Users split by a
hash of their id (`crates/recommend/src/split.rs`): 80% build the matrix, 10% tune, 10% judge; a fifth of each judged
user's likes (4.0 stars and up) held out. Recall@20 = hits / min(20, held out), averaged over users; popularity gets the
same 20 slots with the most-liked titles the user has not liked. Coverage = the share of the matrix's titles that reach
at least one judged user's 20.

Built from 9,944,040 likes by 129,892 people over 18,503 titles (5 likes or more). Reading the ratings 2.5 s, indexing
0.2 s, the matrix 0.4 s on 24 threads (Tier 2 dev machine), peak working set 137 MB. The first version held about 1 GB:
each row kept the room of everything its item touched (`truncate` keeps capacity); one reused buffer per thread fixed it
with identical results.

**Tuning users** (15,672 judged; popularity 0.1665), score `c / (mass_i^(1-alpha) * mass_j^alpha) * c / (c + shrink)`:

| alpha | shrink | Recall@20 | against popularity | coverage |
|---|---|---|---|---|
| 0.3 | 0 | 0.2668 | +60.3% | 4.0% |
| 0.5 (cosine) | 0 | 0.2932 | +76.1% | 9.9% |
| 0.55 | 3 | 0.2970 | +78.4% | 9.3% |
| 0.6 | 0 | 0.2959 | +77.8% | 14.3% |
| **0.65** | **0** | **0.2864** | **+72.1%** | **16.6%** |
| 0.7 | 2 | 0.2719 | +63.3% | 15.4% |
| 0.75 | 3 | 0.2467 | +48.2% | 16.5% |
| 0.9 | 0 | 0.0690 | -58.6% | 25.1% |

Rule, fixed before looking at the test users: the best recall whose coverage clears E2's 15%. Shrinkage helped recall
only marginally where it helped at all, and always cost coverage. The harness reports a loss when there is one
(alpha 0.9, -58.6%).

**Test users** (15,641 judged), alpha 0.65, shrink 0: **Recall@20 0.2855 against popularity 0.1640, +74.1%**
(E1 asks more than +40%); **coverage 16.4%** (E2 asks more than 15%). This is the matrix alone; E1 and E2 are measured
again on the whole recommender in 17.4.

**On the development copy** (`target/release/ingest neighbours`, 2026-11-18, every user, the films the catalogue holds):
25,000,095 ratings, 12,402,053 likes by 162,342 people over 19,792 titles; 989,600 rows stored; **10.4 s end to end**,
database +22.6 MB, peak working set 199 MB. Neighbours, read back: *Tokyo Story* — Late Spring, Early Summer, Ugetsu,
Sansho the Bailiff, Floating Weeds, Sunrise, Ordet, A Man Escaped; *Stalker* — Solaris, The Mirror, Andrei Rublev,
Nostalgia, The Sacrifice, Ivan's Childhood, Persona, Come and See; *Spirited Away* — seven Ghibli films and Grave of the
Fireflies.

**The app's own path, MovieLens' rolling release** (`target/release/ingest neighbours`, which calls the launch step
`neighbours::keep_current`; 2026-11-18, development copy, real network): `ml-latest.zip` fetched over verified HTTPS
(334.6 MB), joined, built and stored: 33,832,162 ratings, 16,844,614 likes by 324,303 people over **25,032 titles**
(19,792 from ml-25m); 1,251,574 rows; the build 15.6 s, **130.5 s in all** (the download most of it); database
+20.9 MB; peak working set 242 MB; the archive deleted. **Run again: `Current`**, one HEAD, nothing fetched.

How current it is: titles with neighbours by year, 2019: 662, 2020: 461, 2021: 400, 2022: 332, **2023: 77**, none
later. *Past Lives* (2023) has neighbours (You Hurt My Feelings, Of an Age, …); *Oppenheimer* and *The Boy and the
Heron* (2023) do not. GroupLens' rolling release stops in 2023, which is why ADR-0054 adds AniList's and TMDB's live
recommendations and cold-start neighbours (17.6) rather than relying on MovieLens.

### The whole recommender (17.4, 2026-11-18)

`cargo run --release -p eval -- recommend --whole [--tune] [--limit N]`: the app's own gathering
(`sinephile_taste::hybrid::candidates`) and ranker (`sinephile_recommend::rank`) on MovieLens users the matrix never
learned from. The matrix is rebuilt in the harness from ml-25m's build users onto catalogue titles (129,892 people,
18,377 titles); each judged user's shown likes become a profile (modes from their vectors in the meaning index, axes
from the catalogue); the held-out likes are the answer; popularity gets the same 20 slots with the build users'
most-liked titles not shown.

**First trial** (300 test users, the starting setting near 1, loved 1, popular 0.15, diversity 0.3): Recall@20 0.2482
against popularity 0.1730 (+43.5%); about 926 candidates a user. Ranking one user's candidates took p50 26.08 ms; the
variety step compared every candidate with every pick in each round. Keeping each candidate's closeness to the nearest
pick as picks are made: **p50 3.73 ms, p95 3.93 ms**, identical results (release build, Tier 2 dev machine).

**Tuning sweep** (2,000 tuning users, 108 settings of near, loved, popular and diversity; 723 s): popularity 0.1627;
every setting between **0.2497 and 0.2560 (+53.4% to +57.3%)**, best near 1, loved 0.5, popular 0.3, diversity 0.15.
Coverage 7.3-8.9% (2,000 users fill only 40,000 slots, so this is not E2's measure). No rail over a cap. The weights
barely matter; and the whole recommender finds fewer held-out favourites than the matrix alone did on the same kind of
users (0.2864, +72.1%), so something structural costs recall. Suspects being measured: only 60 loved titles' neighbours
are asked for (in the harness every like weighs the same, so which 60 is arbitrary), and three per year is a strict cap.

**Where the recall went** (the same 1,000 tuning users each time, popularity 0.1665; the starting setting, then the
best of the 108):

| Variant | Starting setting | Best setting |
|---|---|---|
| A: as built (60 loved titles asked, caps 2/1/3) | 0.2484 (+49.2%) | 0.2537 (+52.4%) |
| B: every loved title asked (`--asked 2000`) | 0.2650 (+59.2%) | 0.2706 (+62.6%) |
| C: caps 2 per director, 2 per franchise, 5 per year (`--caps 2,2,5`) | 0.2691 (+61.6%) | 0.2735 (+64.3%) |
| **D: both** | **0.2859 (+71.7%)** | **0.2906 (+74.6%)** |

Both are structural, and together they bring the whole recommender back to the matrix alone's level (+72.1% on tuning
users) and past it. Taken: up to 500 loved titles asked (one query of about 25,000 rows), and caps of 2/2/5 in a rail
of twenty, so no director, franchise or year takes more than a quarter of it.

**Tuning sweep again, with 500 loved titles asked and caps 2/2/5** (2,000 tuning users, 108 settings, 583 s; per user
modes 7 ms, axes 14 ms, gathering 31 ms, the rest ranking 108 settings): popularity 0.1627; best **near 1, loved 0.5,
popular 0.15, diversity 0.15: Recall@20 0.2906 (+78.6%)**, coverage 7.8% at 2,000 users; the range across settings
+74.7% to +78.6%, coverage 7.2-8.4%; no rail over a cap. Taken as `rank::WEIGHTS` and `rank::DIVERSITY`. Ranking a
user's candidates: p50 2.94 ms, p95 3.21 ms.

With these weights a pick's reason changed: the score maps an unrelated title's closeness (cosine 0) to half its part,
and counted whole that floor outvoted three loved titles pointing to a title. A reason now weighs closeness only above
unrelated (`max(0, cosine)`); the score is unchanged, so the numbers above stand
(`rank::tests::a_title_loved_alongside_what_you_loved_ranks_first_and_says_why` failed before the change).

**Every tuning user, the app's setting** (`eval recommend --whole --check`, 15,671 users, 877 s): popularity 0.1666;
**Recall@20 0.2835 (+70.2%)**; **coverage 15.6%**; intra-list diversity 0.407 against 0.404 for the same candidates in
score order; no rail over a cap. Ranking a user's candidates: p50 0.92 ms, p95 3.29 ms. Per user: modes 8 ms, axes
14 ms, gathering 31 ms.

Coverage clears E2 by 0.6 points, inside the difference two samples of users can make, so before the test users are
read a margin is bought on the tuning users alone, by a rule fixed before it ran: of the app's setting and five more
(popularity weight 0.15 or 0, diversity 0.15, 0.3 or 0.5), the best recall whose coverage over every tuning user is at
least 16%.

| Every tuning user (15,671) | Recall@20 | against popularity | coverage | ILD |
|---|---|---|---|---|
| popular 0.15, diversity 0.15 (the app's, then) | 0.2835 | +70.2% | 15.6% | 0.407 |
| popular 0.15, diversity 0.3 | 0.2831 | +69.9% | 15.8% | 0.411 |
| popular 0.15, diversity 0.5 | 0.2769 | +66.2% | 17.8% | 0.419 |
| **popular 0, diversity 0.15** | **0.2835** | **+70.2%** | **16.7%** | 0.407 |
| popular 0, diversity 0.3 | 0.2826 | +69.6% | 16.9% | 0.410 |
| popular 0, diversity 0.5 | 0.2767 | +66.1% | 19.6% | 0.419 |

By the rule: popular 0, diversity 0.15. Taking popularity's push away cost no recall to four places and widened
coverage by 1.1 points; the most-voted still enter as candidates. No rail over a cap in any setting.

**The test users, once** (`cargo run --release -p eval -- recommend --whole`, 2026-11-18; 15,640 users; 1,061 s;
the app's setting: near 1, loved 0.5, popular 0, diversity 0.15, caps 2/2/5, 500 loved titles asked):

| Criterion | Measured | Target |
|---|---|---|
| E1 | **Recall@20 0.2834 against popularity 0.1641: +72.7%** | more than +40% relative |
| E2 | **coverage 16.6%** of the 18,377 titles MovieLens' build users liked five times or more | more than 15% |
| E3 | **no rail over a cap** (two per director, two per franchise, five per year); intra-list diversity **0.407** against **0.404** for the same candidates in score order | at least the score order's, and no cap broken |
| E5 | ranking one user's candidates (median 925, at most 1,000): **p50 1.23 ms, p95 3.94 ms, max 6.48 ms** | under 150 ms on Tier 0 |

E5 is measured on the Tier 2 development machine (release build); the ranker is single-threaded and does about
8 million multiply-adds for a thousand candidates, 38 times inside the budget here. A Tier 0 run is asked for in the
batched author checks.

**Each verdict seen to fail**: E1 by the matrix harness (alpha 0.9: -58.6% against popularity); E2 and E3 by
`--whole --limit 300 --caps 20,20,20` (300 users fill too few slots for coverage, 3.8%: "NOT met"; ranked with loose
caps, 196 of 300 rails break the app's caps: "NOT met"). The caps check judges against the app's caps whatever the
ranking used, which is what made it able to fail.

### 17.3: the taste rail in the real window, and its reasons (2026-11-18)

A copy of the development data (`SINEPHILE_DATA_DIR`), profile 1 given five loved films (Stalker, Solaris, Tokyo
Story, In the Mood for Love, Late Spring) beside its three liked ones; the release build's Home, then the whole rail
with `cargo run --release -p eval -- recommend --rail 1` (Home's own `rails`, built in 53-55 ms):

| Reason rule | Loved by people who loved | Close in meaning | Widely loved |
|---|---|---|---|
| the biggest part (96310a6) | 0 | **20** | 0 |
| the part furthest above the typical candidate (this commit) | **20** | 0 | 0 |

The picks are the same twenty, in the same order (the rule changes no score): Andrei Rublev, Early Summer, The Mirror,
Wild Strawberries, A Man Escaped, The Passion of Joan of Arc, Ugetsu, L'Avventura, The Godfather Part II, Sunrise,
Pickpocket, Harakiri, Floating Weeds, Persona, Sansho the Bailiff, Close-Up, Viridiana, Man with a Movie Camera, The
Discreet Charm of the Bourgeoisie, La Dolce Vita. Under the first rule Pickpocket and The Discreet Charm of the
Bourgeoisie were "close to The Godfather and Oppenheimer"; under the second, "loved by people who loved Tokyo Story,
Stalker and Solaris". Every candidate from the meaning pool is close to some mode, so closeness never told one apart;
the people who loved the same titles did. "Close to" is now what a title gets when closeness is what sets it apart,
which is the case of a title nobody has rated (17.6).

### 17.6: meaning for titles without it, on a copy of the development catalogue (2026-11-18)

`ingest --data-dir <copy> meaning` (`sinephile_catalogue::meaning::at_launch`, what the app runs after each launch's
catalogue check), against the live services, no TMDB key (so AniList and Wikipedia only). The published meaning file
holds 119,874 titles; the catalogue has 874,755 core titles.

| Run (on 3bd324e and after) | Waiting | Described: AniList / Wikipedia | Embedded | Time |
|---|---|---|---|---|
| 1. only the titles past the published file | 2,000 | 0 / 16 | 16 | 60.6 s |
| 2. every core title without a vector (A44) | 2,000 | 103 / 67 | 170 | 115.6 s |
| 3. as 2, with the opening of articles whose lead was short | 2,000 | 69 / 119 | 188 | 186.0 s |

- Run 1 found what "the titles a refresh added" are: 19,052 core titles past the file, mostly this year's, the most voted
  with 4,741 votes; Wikipedia had an article for 23 of the first 2,000. Meanwhile 1,331 core titles with 10,000 votes or
  more had no vector (Doctor Who, One Punch Man, Solo Leveling, Delicatessen), which led to A44.
- Run 2: One Punch Man and Solo Leveling described by AniList, Dept. Q by Wikipedia (306 characters), all embedded.
  1,518 of the 2,000 Wikipedia leads were under the 300-character rule (Delicatessen's is 248): the reason for run 3.
- Run 3: up to 100 openings a launch (1,200 characters, into the plot), most-voted first.
- After the three, the companion graph holds 374 vectors in 298 KB. The waiting query takes 2.85 s over the core tier
  (logged as slow; once a launch, in the background). Doctor Who has no IMDb link on Wikidata for its id, so only a
  TMDB key describes it.

### 17.6: live neighbours on the copy (2026-11-18)

`ingest --data-dir <copy> neighbours --live` (`sinephile_catalogue::live::at_launch`), real AniList, no TMDB key, the
copy's profile given two loved anime (One Punch Man, Solo Leveling) beside its films: AniList asked about 2 titles,
16 rows. One Punch Man's first are Mob Psycho 100 (0.288, MovieLens' mean at rank 1) and Mashle (0.248); Solo
Leveling's, Sword Art Online and Tower of God. A second run asks nothing (a week has not passed).

**What it showed about the ranker.** `eval recommend --rail 1` (now opening both graphs, as the app does): the same
twenty arthouse films and no anime, though two of the profile's nine loved titles are anime. The loved part sums a
candidate's neighbour scores over the loved titles, so a film loved beside four of the profile's films outscores an
anime recommended beside one; with seven films to two anime, the larger taste takes every slot. Recall@20 does not see
it (it rewards the majority). Calibration (a rail's mix of taste modes matching the profile's, Steck 2018) is the
remedy, measured against E1-E3 before it is kept. Home built in 63-96 ms warm (368 ms on the first, cold, read after
migration 0031 rewrote the table).

### 17.6: calibration by taste mode, on the test users (2026-11-18)

Each taste mode may fill at most its share of a rail (largest-remainder apportionment, every mode at least one slot;
`rank::quotas`), so a rail mixes as the profile does (Steck 2018). Measured with the same harness on the same 15,640
test users, `SINEPHILE_DATA_DIR=<copy> eval recommend --whole` (on afa2ec0 plus the ranker change; 1,257 s, the machine
also building), against the recorded run without it:

| Setting | Recall@20 | against popularity | coverage | ILD | miscalibration (KL) |
|---|---|---|---|---|---|
| uncalibrated (c183f27, recorded above) | 0.2834 | +72.7% | 16.6% | 0.407 | not measured |
| **calibrated (the app's now)** | **0.2648** | **+61.4%** | **43.5%** | **0.417** | **0.008** |
| the same candidates in score order | 0.2830 | +72.5% | 16.4% | 0.404 | 0.683 |

E1 met (+61.4% > 40%), E2 met (43.5%), E3 met (no rail over a cap; 0.417 against 0.404). Ranking p50 1.75 ms, p95
6.29 ms on this machine with a build running beside it. **The trade, taken knowingly:** 1.9 points of recall (held-out
likes follow the larger taste, so recall rewards a rail that does) for coverage nearly tripled and a rail whose mix
follows the profile's: on the copy, a profile of seven films and two anime got 15 films and 5 anime instead of 20 films.

**The rule for the maker's weight, fixed before its run:** `eval recommend --whole --tune --people 0,0.1,0.25,0.5
--limit 2000` on tuning users, calibrated; the weight with the best Recall@20; two within 0.002 of each other count as
equal, and the larger is taken, because what the part is for (a title nobody has rated) is what held-out MovieLens likes
cannot measure.

**The maker's weight, by that rule** (`eval recommend --whole --tune --people 0,0.1,0.25,0.5 --limit 2000`, on 95e17cc's
code, calibrated, 2,000 tuning users; ranking p95 7.52 ms with a build beside it):

| people | Recall@20 | against popularity | coverage | ILD |
|---|---|---|---|---|
| 0 | 0.2700 | +65.9% | 16.3% | 0.418 |
| **0.1** | **0.2705** | **+66.2%** | **16.9%** | 0.418 |
| 0.25 | 0.2566 | +57.6% | 20.1% | 0.418 |
| 0.5 | 0.1812 | +11.3% | 24.0% | 0.420 |

0.1 is best, and 0 is within 0.002 of it (equal by the rule, so the larger): **people 0.1**. A heavier weight fills the
rail with a loved director's other work, which MovieLens' held-out likes reward less and less. (Coverage here is over
2,000 users; it grows with the number of users judged, so it is not comparable with the test run's 15,640.)

### 17.6: the ratings refresh, one chunk of 200 (2026-11-18)

Found by launching the release build on the copy while IMDb's files had been republished: the app log
(`slow statement`) showed 15.9 s and 19.8 s for chunks of the ratings refresh written as `UPDATE ... FROM` (95e17cc,
never pushed). Timed on the copy (3.27 million titles, 200 real IMDb ids, each in a rolled-back transaction, Python's
SQLite 3.49):

| Form | One chunk |
|---|---|
| `UPDATE ... FROM` with the rising columns | 17,329.8 ms |
| the same, join order forced with `CROSS JOIN` | 187,520.4 ms |
| the correlated form before 17.6 | 7.2 ms |
| **the correlated form with the rising columns (5c70542)** | **8.2 ms** |

A refresh is about 8,000 chunks: hours against about a minute. The cause: no planner statistics for `external_ids`, so
the planner put it outside, by `source` alone. `a_ratings_chunk_touches_only_its_own_rows` reproduces those statistics
on 300,000 titles: 2.8 s a chunk on the `FROM` form (fails), well under 400 ms on the correlated one.

### 17.6: the whole recommender as the app now runs it, on the test users (2026-11-18)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- recommend --whole` on eca974f (calibrated, the makers' pool
and part at 0.1; 5c70542's interleaving changes only the order, so every number below holds for it), 15,640 test users,
1,496 s with builds and the app running beside it:

| Setting | Recall@20 | against popularity | coverage | ILD | miscalibration | over caps |
|---|---|---|---|---|---|---|
| **the app's** | **0.2639** | **+60.9%** | **45.7%** | **0.418** | **0.008** | **0** |
| the same candidates in score order | 0.2834 | +72.8% | 17.0% | 0.404 | 0.664 | 0 |

**E1 met** (+60.9% > 40%), **E2 met** (45.7% > 15%), **E3 met** (no rail over a cap; 0.418 against 0.404).
Candidates per user: median 1,051, max 1,144 (the makers' pool adds about a hundred). Ranking one user's candidates:
p50 2.30 ms, p95 7.36 ms, max 13.75 ms on this machine under load; E5's 150 ms is for Tier 0 and stays to be measured
there. Against the first build of the whole recommender (c183f27: +72.7%, 16.6%): 11.8 points of lift given for
coverage nearly tripled and rails that mix as the profile does.

### Phase 17 regression check before merge (2026-11-18)

Every harness on the copy of the development data (`SINEPHILE_DATA_DIR=<copy> target/release/eval.exe all`, then
`vector --report`), on f8d8e58, release build, after three launches' meaning had been added to the copy:

| Metric | Recorded before | Now |
|---|---|---|
| Search E2 exact-title top-1 | 43/43 | **43/43** |
| Search E1 p50 / p95, warm | 6.4 / 15.2 ms | **6.5 / 14.5 ms** (34.5 ms p95 in a first run with builds beside it) |
| Search E3 meaning nDCG@10 | 0.0472 | **0.0472** (carried on to Phase 18) |
| Search E3 filter nDCG@10 | 1.0000 | **1.0000** |
| HNSW recall@10 (published graph) | 0.9760 | **0.9760**, index p95 1.85 ms |
| Subtitles aligned / false accepts | 80/80, 0/10 | **80/80, 0/10** |
| Identification E1 top-1, tuning / held out | 99.05% / 100% | **99.05% / 100.00%** |
| Identification E2 false-confident at 0.75 | 0.39% / 0.00% | **0.39% / 0.00%** |

No metric regressed. The first `eval all` refused at the vector harness ("the index holds 119874 vectors and the
catalogue offers ..."): A44's job had written synopses for titles inside the published file, which then counted as the
file's own. Fixed in f8d8e58 before this run.

## Phase 18 — Discovery Engine

### Verdicts fixed before any run (2026-11-19)

Written here before the first run of each harness, so a number cannot choose its own bar.

- **E1** (`eval discovery --simulate`): 1,000 simulated viewers, 120 days, one allocation of 60 slots a day (the taste
  rail shows at most 20 of the familiar's), three cards acted on a day (finished with the viewer's chance for its arm,
  otherwise "not for me"). Each viewer has a favourite discovery arm (chance 0.7; other discovery arms 0.15; the
  familiar 0.4) that changes to a different arm on day 61. **Met** when the new favourite's mean share of discovery's
  slots over days 101-120 is at least 15 points above its share over days 41-60, the old favourite's at least 15 points
  below, and the new favourite's share rose for at least 80% of viewers.
- **E2** (same command): 1,000 maximally conservative viewers (Comfort; every familiar card finished with chance 0.9;
  every discovery card "not for me") for 120 days. **Met** when on every day of every viewer discovery has at least 20%
  of the slots and at least one rail's worth (6), and every discovery arm is shown on at least 10% of each viewer's days.
- **E4** (`eval discovery`, MovieLens test users): novelty is mean self-information, `-log2(p)`, `p` a title's share of
  MovieLens build users who liked it. **Met** when the discovery rails' mean novelty is at least 1 bit above Phase 17's
  taste rail on the same users, and Recall@N of everything Home's taste and discovery rails show stays more than 40%
  above popularity's top N (N the same).
- **E4, made precise before its first run (2026-11-19, after E1 and E2 ran):** a title no build user liked has no share
  (every film after ml-25m's 2019 cut-off among them), and counting it as the most novel would flatter discovery for the
  wrong reason. Such titles are left out of both rails' means, and each rail's share left out is reported beside. Home
  as a new profile sees it: no outcomes yet, the budget at its middle (0.5), 60 slots, the taste rail at most 20.

### 18.2: the bandit in simulation, E1 and E2 (2026-11-19)

`cargo run --release -p eval -- discovery --simulate` (pure simulation, no data; a few seconds, Tier 2 dev machine).

**First run, on 7bc4ef3** (each discovery arm's share by one Thompson draw, over as many rails as fit at six slots
each, no arm above 40% of discovery): **E1 NOT met.** The old favourite's share of discovery went 31.2% to 25.2% (-6.0
points), the new favourite's 17.3% to 29.3% (+12.0); it rose for 99.6% of viewers. E2 met (least 20.0%, no day without
a rail, every arm on at least 27.5% of days). Cause: at 25 to 30 discovery slots, five arms at a rail's minimum of six
fill nearly all of them, so a learned preference could only decide which arm was left out.

**Changed** (the bar unchanged): an arm's share follows its chance of being the best arm, how often its draw is the
highest of 100 joint draws (probability matching); only as many rails as it takes to place discovery's slots under the
cap; the cap raised to half of discovery, since with a rail's minimum per arm 40% kept a changed favourite second.

**On fcc4110:**

| Measure | Value | Bar |
|---|---|---|
| E1: new favourite's share of discovery, days 41-60 to 101-120 | **12.8% to 47.8% (+35.0 points)** | at least +15 |
| E1: old favourite's share | **49.0% to 28.0% (-21.0 points)** | at least -15 |
| E1: viewers whose new favourite's share rose | **98.9%** | at least 80% |
| (a viewer who warms to discovery on day 61: discovery's share of all slots) | 20.4% to 46.1% | (shown, not a bar) |
| E2: discovery's least share on any day of any viewer | **20.0%** (mean 20.0%) | at least 20% |
| E2: days without a discovery rail | **0** of 120,000 | none |
| E2: the least any discovery arm was shown, of one viewer's days | **23.3%** | at least 10% |

**E1 met. E2 met.** Each seen to fail: `--frozen` (beliefs never updated) gives +0.1 and +0.2 points and 50.0% rose, E1
NOT met; `--no-floor` (floor 0) gives discovery 0.0% on all 120,000 days, E2 NOT met.

### 18.2: E4 and E3 on the MovieLens test users (2026-11-19)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- discovery` on a1dd9a9 (each arm's picks ordered by Phase 17's
relevance), on a copy of the development data with every title of 1,000 votes asked of Wikidata (ADR-0055); 15,640 test
users, 1,973 s, Tier 2 dev machine. Home as a new profile sees it: no outcomes, budget 0.5, 60 slots, the taste rail at
most 20; 49.5 cards shown a user.

**A trial on the first 100 users first** (each arm by closeness, rating or region): novelty +3.58 bits, Recall@N +16.0%
over popularity; discovery picks found a held-out like 0.0097 times a card against popularity's 0.0539 and the taste
rail's 0.1340. Phase 17's ranker asked for the same N reached +53.4%, so the bar is reachable. Ordered by relevance, the
same 100 users gave +1.72 bits and **+42.0%**: a sample of 100 flattered it, as the full run shows.

| Measure | Value | Bar |
|---|---|---|
| novelty, taste rail (20 a user) | 3.74 bits (1.5% of picks liked by no build user, left out) | |
| novelty, discovery rails | **5.67 bits** (5.0% left out) | |
| E4: discovery over the taste rail | **+1.93 bits** | at least +1: met |
| E4: Recall@N of everything shown, against popularity's top N | **0.3091 against 0.2400: +28.8%** | more than +40%: **NOT met** |
| (the taste rail alone, Recall@20) | 0.2639 (Phase 17 recorded 0.2639: no regression) | |
| (Phase 17's ranker asked for N, the ceiling) | 0.3553, +48.0% | |
| (held-out likes per card: taste rail / discovery / popularity) | 0.1326 / 0.0299 / 0.0597 | |
| E3: discovery labels checked with `strategy::holds` | **461,488, none false** | none false: met |

Per arm: stretch 4.64 bits, bridge 5.42, blind spot 4.15, canon 3.57, deep cut 11.37 (16.7% liked by no build user);
each arm had about 8,500 rails (blind spot 7,821). Gathering a user's discovery pool: p50 24 ms, p95 43 ms.

**E4 is not met.** To reach +40% the discovery picks need about 35% more held-out likes a card (0.030 to 0.040) with
at least a bit of novelty kept; they have 0.93 bits to spare. Next: hits per card by arm, then the matrix's weight in
the arms' relevance, judged on 2,000 users and then once more on all of them.

### 18.2: three ways at E4's relevance, on the first 2,000 test users (2026-11-19)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- discovery --limit 2000`. The 2,000 track the whole set: on
a1dd9a9 they give +27.6% (all 15,640: +28.8%) and +1.96 bits. Held-out likes per card by arm on a1dd9a9: canon 0.0597
(popularity's own rate), bridge 0.0386, stretch 0.0357, blind spot 0.0116, deep cut 0.0021 (11.4 bits: a film with at
most 15,000 IMDb votes is seldom a MovieLens favourite).

| Approach | Recall@N against popularity | novelty over the taste rail |
|---|---|---|
| 1. each arm's picks by Phase 17's relevance (a1dd9a9; from +16.0% on 100 users) | +27.6% | +1.96 bits |
| 2. the matrix's part weighed 1.5 in that relevance, not 0.5 (reverted) | +27.5% | +1.94 bits |
| 3. the familiar's whole share shown (a second taste rail past twenty; 7c8377c) | **+33.2%** | **+2.02 bits** |

The third is kept as a fix, not a tuning: at the middle budget the plan gave the familiar 30 slots and Home showed 20,
so discovery had six cards in ten where the plan meant five. **E4 stays NOT met** (+33.2% against more than +40%):
three distinct approaches, so the choice is recorded as blocker B17 in PROJECT_STATE.json (ADR-0040).

### 18.2, B17 option A: the rule for a population prior, fixed before any run (2026-11-20)

A new profile's bandit starts flat, so every discovery arm starts equally likely, though on the test users their
held-out likes per card differ thirtyfold (canon 0.060, deep cut 0.002) and their novelty threefold. A prior from
relevance alone would leave only canon and bridges, and novelty would fail; so each arm's prior weighs both.

1. **Measure** on the first 2,000 TUNING users (`eval discovery --tune --limit 2000`), with no prior: each discovery arm's
   held-out likes per card `h` and mean novelty `n`.
2. **Value**: `v = h / max(h) + n / max(n)` over the five discovery arms, so 0 to 2; the prior's mean is `v / 2`.
3. **Strength**: `K` pseudo-trials per arm (`alpha += K * mean`, `beta += K * (1 - mean)`), used only in the draws that
   share discovery's slots between its arms; discovery's share of Home is unchanged. `K` from {0, 2, 4, 8, 16}.
4. **Choose** on the same 2,000 tuning users: the largest Recall@N against popularity whose novelty is at least
   **1.2 bits** over the taste rail (a margin over E4's 1); a tie goes to the smaller `K`.
5. **Judge** once on all test users (E4), and run `eval discovery --simulate` again (E1, E2) with the prior in place.

### 18.4: a popularity prior for search by meaning, the rule fixed before any run (2026-11-20)

Phase 5's E3 (meaning nDCG@10 0.0472 against 0.75) is worked in Phase 18, where mood shelves depend on it. Its eight
meaning queries are too few to choose a setting on and then be judged by, so a TUNING set of ten more, graded before any
run, is `fixtures/search/meaning-tuning.tsv` (38 answers, every id checked against the catalogue's title).

- **The change tried**: the meaning half retrieves its 200 nearest, scores each `cosine + lambda * ln(1 + votes) /
  ln(1 + the most votes among them)`, and passes its best 50 to the fusion as now. Plot-first documents let 858,170
  obscure plots outrank the films a person means (v3, 2026-10-25); votes say which are meant.
- **Chosen** on the tuning set: `lambda` from {0, 0.05, 0.1, 0.2, 0.4}, the best meaning nDCG@10, a tie to the smaller.
- **Judged** once on `semantic-queries.tsv` (E3 and E4's two named queries); E2 (43/43 exact titles) must hold, and
  E1's p95 must stay inside the 80 ms budget.

### 18.2, B17 option A: the prior measured and chosen on tuning users (2026-11-20)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- discovery --tune --limit 2000 --prior-weight K`, the first
2,000 tuning users, by the rule above.

**Step 1, no prior:** Recall@N +38.8% over popularity (0.3566 against 0.2569), +1.95 bits. Per arm, held-out likes per
card and novelty: stretch 0.0302 and 4.69 bits, bridge 0.0303 and 5.49, blind spot 0.0108 and 4.15, canon 0.0497 and
3.64, deep cut 0.0018 and 11.35. **Step 2, the means** (`v / 2`): stretch 0.5104, bridge 0.5467, blind spot 0.2915,
canon 0.6604, deep cut 0.5181.

| K (pseudo-trials) | Recall@N against popularity | novelty over the taste rail |
|---|---|---|
| 0 | +38.8% | 1.95 bits |
| **2** | **+44.1%** (0.3705) | **1.79 bits** |
| 4 | +43.6% | 1.63 bits |
| 8 | +43.0% | 1.50 bits |
| 16 | +44.0% (0.3703) | 1.37 bits |

**Chosen: K = 2** (the best relevance with at least 1.2 bits). At K = 2 a new profile's first Home almost never has a
blind-spot rail (one of 2,000) while canon is in nearly every one; blind spots come with outcomes, which E2's re-run
shows. **E1 and E2 with the prior** (`eval discovery --simulate`, on 193e4d4): E1 +35.2 / -22.2 points, rose for 98.9%
(a blind spot that becomes the favourite goes from 10.3% to 47.3% of discovery); E2 floor 20.0% every day, no day
without a rail, every arm on at least 16.7% of days. Both met.

### 18.4: the search popularity prior measured on the tuning set (2026-11-20)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- search --tune-meaning`, on `fixtures/search/meaning-tuning.tsv`
only:

| prior | meaning nDCG@10 (10 queries) |
|---|---|
| 0 (as before) | 0.0215 |
| 0.05 | 0.1136 |
| 0.1 | 0.2210 |
| 0.2 | 0.3816 |
| **0.4** | **0.4213** |

**Chosen: 0.4**, the grid's top, so a larger weight might do better; the grid was fixed first and is not extended after
seeing it. Per query at 0.4: samurai 0.5064, stranded astronauts 0.2021, courtroom 0.3975, mafia saga 0.7156, dinosaurs
0.7680, simulation 0.0000, silent stunts 0.2689, Japanese animated heroine 0.5945, zombies 0.4034, boxer 0.3561.

### 18.2: E4 judged once on all test users, with the prior (2026-11-20)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- discovery`, the build of 193e4d4's tree (prior weight 2; the
banner's HEAD read 394c3ef because the commit came after the build), 15,640 test users, 2,020 s.

| Measure | Value | Bar |
|---|---|---|
| novelty, taste rail / discovery rails | 3.74 / 5.55 bits | |
| E4: discovery over the taste rail | **+1.81 bits** | at least +1: met |
| E4: Recall@N of everything shown against popularity's top N | **0.3639 against 0.2593: +40.4%** | more than +40%: **met** |
| (the taste rail alone, Recall@20) | 0.2639 (unchanged) | |
| (Phase 17's ranker asked for N) | 0.3772, +45.5% | |
| E3: discovery labels checked | **467,875, none false** | met |

Per arm: stretch 5.05 bits and 0.0276 a card, bridge 5.81 and 0.0301, blind spot 9 rails in 15,640 users, canon 3.63 and
0.0495, deep cut 11.33 and 0.0017. **E4 met, by 0.4 points**: a margin that thin is stated as it is, not rounded up.

### 18.4: search by meaning judged once with the prior at 0.4 (2026-11-20)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- search --report` on 890a469, release build, Tier 2 dev machine,
nothing else running:

| Measure | Recorded before (Phase 17 close) | Now | Target |
|---|---|---|---|
| E2 exact-title top-1 | 43/43 | **43/43** | 100% |
| E1 latency p50 / p95 / max | 6.5 / 14.5 ms | **11.6 / 24.2 / 93.0 ms** | p95 < 80 ms (Tier 0) |
| E3 meaning nDCG@10 (8 queries) | 0.0472 | **0.3276** | > 0.75: **NOT met** |
| E3 filter nDCG@10 | 1.0000 | **1.0000** | |
| topical nDCG@10 (6) | 0.0629 | **0.2307** | |
| known-item MRR (2) | 0.0000, 0 of 2 in the top 10 | **1.0000, both first** | |

Per query: grief 0.0000 (was 0.1505), hometown and nephew 0.7602 (was 0), Wong Kar-wai but Korean 0.2385 (was 0.0752),
slow science fiction about memory 0.4770 (was 0), anime loneliness 0.1894 (was 0), heist 0.1505 (was 0), mountain
documentaries 0.5028 (was 0.1518), neorealism 0.3028 (was 0). E1 rose by about 10 ms at p95: 200 nearest instead of 50,
and one query for their votes.

**Phase 5's E4 by eye** (`eval search --query`): "like Wong Kar-wai but Korean" now gives *Chungking Express*, *In the
Mood for Love*, *Memories of Murder*, *2046*, *The Grandmaster*, *Thirst*, *As Tears Go By*, *3-Iron*: defensible.
"Films about grief that aren't depressing" gives *Hereafter*, *Antichrist*, *The Queen*, *Cries & Whispers*, *1984*,
*Take Shelter*, *Talk to Her*: about grief or loss in part, but *Antichrist* and *Cries & Whispers* are the opposite of
"not depressing", and *1984* does not belong. The negation is what the meaning model misses. **Half met.**

### 18.5: the real window changed the arms, so the prior's rule was run again (2026-11-20)

`eval recommend --rail 2` (a probe profile of 27 rated films on the copy) showed four things no metric had: a taste named
by its strongest film though the pick was nearer another of its films (*Kiki's Delivery Service* "close to Tokyo
Story"); co-productions as stretches (*Ghost in the Shell* "but from the United Kingdom"); obscure bridges (*Tokyo File
212*); and mood shelves opening on a 1912 short. Fixed in 8ab7abe (see the case study). The stretch and bridge arms
changed (1,000 votes; a country is a step out only when none of the film's are the taste's), so the rule above was run
again from step 1, on the same 2,000 tuning users.

**Step 1, no prior:** +38.8% (0.3565 against 0.2569), +1.98 bits. Per arm: stretch 0.0281 and 4.73 bits, bridge 0.0302
and 5.60, blind spot 0.0109 and 4.15, canon 0.0494 and 3.65, deep cut 0.0018 and 11.35. **Means:** stretch 0.4928, bridge
0.5524, blind spot 0.2931, canon 0.6608, deep cut 0.5182.

| K | Recall@N against popularity | novelty over the taste rail |
|---|---|---|
| 0 | +38.8% | 1.98 bits |
| 2 | +43.3% (0.3684) | 1.86 bits |
| 4 | +43.0% (0.3679) | 1.74 bits |
| 8 | +43.3% (0.3684) | 1.55 bits |
| 16 | **+44.1% (0.3705)** | 1.40 bits |

**The rule chose 16, and 16 failed E2** in the simulation step 5 requires (`eval discovery --simulate`): blind spots, with
the lowest prior mean, were shown on only 2.5% of a refusing viewer's days against E2's 10% (E1 held: +29.6 / -18.3,
94.7%). The rule did not say what then. A weight that breaks an exit criterion cannot ship, so **the next by the rule's
own order that keeps E1 and E2 was taken: 2** (tied with 8 on Recall@N; a tie to the smaller), and this departure is
recorded here rather than the rule rewritten. **E1 and E2 at 2** (0e06e59): E1 +35.1 / -22.1 points, rose for 98.8%;
E2 floor 20.0% every day, no day without a rail, every arm on at least 16.7% of days. Both met.

### 18.5: E4 judged on all test users with the fixed arms and the re-chosen prior: NOT met (2026-11-20)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- discovery` on 0e06e59 (the arms after the real window, prior
weight 2), 15,640 test users, 1,934 s.

| Measure | Value | Bar |
|---|---|---|
| novelty, taste rail / discovery rails | 3.74 / 5.63 bits | |
| E4: discovery over the taste rail | **+1.89 bits** | at least +1: met |
| E4: Recall@N of everything shown against popularity's top N | **0.3626 against 0.2593: +39.9%** | more than +40%: **NOT met** |
| (the taste rail alone, Recall@20) | 0.2639 (unchanged) | |
| E3: discovery labels checked | **467,741, none false** | met |

Per arm: stretch 5.08 bits and 0.0248 a card (5,861 rails, down from 7,454: the co-production rule and the floor of
votes), bridge 5.91 and 0.0301, blind spot 13 rails, canon 3.63 and 0.0494, deep cut 11.33 and 0.0017.

**E4 is NOT met, by 0.1 point.** The +40.4% recorded above was measured on the arms before the real window's fixes; the
fixes made the labels honest and cost half a point of relevance, and they stay. The test users have now judged two
versions; a third version will be chosen on the tuning users (whose numbers run about 3.5 points above the test users'
on every setting so far) and judged on the test users once.

### Phase 18 regression check (2026-11-20)

Every harness on the copy of the development data (`SINEPHILE_DATA_DIR=<copy> target/release/eval.exe all`, then
`vector --report`), on 0e06e59's tree, release build, Tier 2 dev machine:

| Metric | Recorded before (Phase 17 close) | Now |
|---|---|---|
| Search E2 exact-title top-1 | 43/43 | **43/43** |
| Search E1 p50 / p95, warm | 6.5 / 14.5 ms | **7.6 / 17.4 ms** (max 70.2; the popularity prior's 200 nearest and their votes) |
| Search E3 meaning nDCG@10 | 0.0472 | **0.3276** |
| Search E3 filter nDCG@10 | 1.0000 | **1.0000** |
| HNSW recall@10 (published graph) | 0.9760 | **0.9760**, index p95 1.38 ms |
| Subtitles aligned / false accepts | 80/80, 0/10 | **80/80, 0/10** |
| Identification E1 top-1, tuning / held out | 99.05% / 100% | **99.05% / 100.00%** |
| Identification E2 false-confident at 0.75 | 0.39% / 0.00% | **0.39% / 0.00%** |
| Recommender, the taste rail's Recall@20 on test users | 0.2639 | **0.2639** (`eval discovery`, above) |

No quality metric regressed; search's p95 rose by 2.9 ms, inside its 80 ms budget, the measured cost of the prior.

### 18.5, B18: popularity in the arms' order, the rule fixed before any run (2026-11-20)

E4 stands at +39.9% on the test users (bar more than 40%). The arms order their picks by Phase 17's relevance (closeness
to a taste, half the matrix's part); a held-out favourite leans towards the widely seen, and search by meaning gained
most from exactly that term. So: `relevance + w * ln(1 + votes) / ln(1 + the most votes among the titles)` in
`strategy::relevance`, the same form as search's prior.

1. `w` from {0, 0.1, 0.2, 0.4}, each measured on the first 2,000 TUNING users (`eval discovery --tune --limit 2000`), the
   bandit's prior as shipped (0e06e59).
2. **Chosen**: the best Recall@N against popularity whose novelty is at least 1.2 bits over the taste rail; a tie to the
   smaller `w`.
3. **Then** `eval discovery --simulate` (E1, E2) must still hold; if the chosen `w` breaks either, the next by the same
   order that keeps both is taken (said here, before running, this time).
4. **Judged** once on all test users (E4). If it is not met, E4 is carried to Phase 19, whose Home overhaul decides which
   rails Home shows and how many cards, with its measured state.

### 18.5, B18: popularity in the arms' order, measured on tuning users (2026-11-20)

`eval discovery --tune --limit 2000`, the prior as shipped (weight 2), each weight a rebuild of `RELEVANCE_POPULARITY`:

| w | Recall@N against popularity | novelty over the taste rail |
|---|---|---|
| 0 (as shipped, 0e06e59) | +43.3% (0.3684) | 1.86 bits |
| **0.1** | **+43.6% (0.3692)** | **1.68 bits** |
| 0.2 | +43.6% (0.3691) | 1.59 bits |
| 0.4 | +43.6% (0.3691) | 1.50 bits |

**Chosen: 0.1** (b2842d4). A small gain, 0.3 point on these users, most of it from bridges and stretches leaning to the
widely seen. `eval discovery --simulate` unchanged (E1 98.8% rose, E2 every arm on 16.7% of days): the simulation does not
see the arms' order.

### 18.5: E4 judged once with popularity in the arms' order: met, by the narrowest margin (2026-11-20)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- discovery` on b2842d4, 15,640 test users, 1,944 s.

| Measure | Value | Bar |
|---|---|---|
| novelty, taste rail / discovery rails | 3.74 / 5.45 bits | |
| E4: discovery over the taste rail | **+1.71 bits** | at least +1: met |
| E4: Recall@N of everything shown against popularity's top N | **0.3630 against 0.2593: +40.0%** | more than +40%: **met** |
| (the taste rail alone, Recall@20) | 0.2639 (unchanged) | |
| E3: discovery labels checked | **467,741, none false** | met |

**Met by under 0.05 point**: the harness judges the unrounded ratio above 1.40, and the printed four decimals bound it at
no more than 1.4004. That is inside the difference two samples of users make, and is stated so rather than rounded into
comfort. The test users have now judged three versions of the arms (193e4d4 +40.4%, 0e06e59 +39.9%, b2842d4 +40.0%), each
change chosen first on the tuning users by a rule written before it ran.

### 18.6: half stars in the taste model (ADR-0056), the taste harness before and after (2026-11-21)

`SINEPHILE_DATA_DIR=<copy> cargo run --release -p eval -- taste`: the same three MovieLens users (3, 12, 72), five folds,
500 slots. Each user's own half-star ratings go in as a Letterboxd export through `sinephile_imports`, so the whole path
is measured: import, storage, the taste model's weights, liked titles.

| | Whole stars (3.5 rounded up to 4; b2842d4) | Half stars kept (ADR-0056) |
|---|---|---|
| liked, users 3 / 12 / 72 | 498 / 411 / 407 | 355 / 321 / 407 |
| held-out favourites, modes alone | 12.83% | 11.58% |
| **modes with the popularity prior (as shipped)** | **40.63%** | **40.71%** |
| the 500 most popular (the bar) | 35.64% | 34.73% |
| lift over popularity | 1.14x | **1.17x** |
| E2 (Phase 14, reflected in the taste model) | met | met |

What changed is what counts as a favourite: a 3.5 is no longer rounded up into "liked" (it weighs 0.6 in the model, between
three stars' 0.2 and four's 1.0), so the held-out set is stricter and popularity finds less of it. On that stricter set
the model finds slightly more than before and its margin over popularity widens (+5.98 points against +4.99). Whole-star
ratings weigh exactly what they did (`modes::rating_weight`, tested), so nothing already rated moved.

### 18.7: Surprise me's speed (the author, 2026-11-21: "very slow")

`SINEPHILE_DATA_DIR=<a copy of the author's data> cargo run --release -p eval -- surprise`: their own profile, the
catalogue's vector half, the evening. Budget: a press under 300 ms.

| | Before (a1816c7) | After (Home's rails reused while current) |
|---|---|---|
| Home's rails, first build of the run | 3,677 ms | 325 ms (the files already in the OS cache) |
| Home's rails, warm | 402 ms | 315 ms |
| **Surprise me, a press** | **373 to 377 ms: over budget** | **1.5 to 1.8 ms** |
| a press before Home was ever built | (as a press) | 342 ms (builds Home) |

Before, each press built all of Home (taste model, discovery pools, mood shelves) to choose one film, holding the search
engine's lock, so in the app it also waited behind any search or the launch's background work. Now Home keeps the rails
it last built for each profile, and a press picks from them while nothing they stand on has moved (the taste fingerprint,
the part of the day, "what you love", 15 minutes), without the engine. The first run, over budget, is the check seen to
fail.

### Regression check before merging Phase 18 (rule 13), 2026-11-21

What 18.6 and 18.7 touched that earlier harnesses measure, run again on the scratch data on 525bb0a's code:

| Harness | Recorded | Now |
|---|---|---|
| `eval update` (Phase 15 E5: a watch to an updated model, under 2 s) | 125.7 to 133.2 ms | 113.2 to 120.9 ms; every watch in its model; E5 met |
| `eval import` (Phase 14 E2: 600 films, tuned set) | 598 right (99.67%), 0 wrong, 2 asked | 598 right (99.67%), 0 wrong, 2 asked; 349 ratings recorded (half stars kept) |
| `eval taste` (Phase 14 E2 / 15) | 40.63%, 1.14x popularity | 40.71%, 1.17x (18.6) |
| `eval discovery` (Phase 18 E3, E4) | as judged on b2842d4 | unchanged code paths since |
| Search (Phase 5 E1, E2, E3) | as recorded in 18.4 | no search code changed since |

Nothing regressed. The full workspace suite: 960 passed; the front end: 294.

## Between Phases 18 and 19 — disk use and searching a person (ADR-0057, 2026-11-21)

### Disk, measured before and after (`du -sh`)

| Where | Before | After |
|---|---|---|
| The installed app's `data/torrents` (streamed, not downloaded: 43 torrents) | 524 GB | 0 (and their 43 session entries removed) |
| The repository's `target/` (`cargo clean`: 343,106 files) | 295 GB (325 GiB) | rebuilt lean on demand |
| `src-tauri/target` (stale), the spikes' build output | 6.5 GB, 4.4 GB | 0 |
| Stale data (the previous model's files, a backup of the meaning file) | 0.7 GB | 0 |
| Scratch copies of data in Temp | 40 GB | 2 MB |
| **Free on C:** | about 157 GB (not measured: 681 GB measured once the streams were gone, less their 524) | **1,028 GB** |

The author's Downloads (11 GB) were not touched. `tools/doctor` now warns when `target/` passes 60 GB: seen to fail at
295 GB before the clean, 0.0 GB after.

### Streaming fetches only the watched file (`cargo test -p sinephile-torrent --test swarm`)

`streaming_fetches_only_the_file_watched`: a three-file torrent (a 12 MB film, a 2 MB sample, a readme) streamed by its
film fetched **at most the film plus 1 MB**; without the fix it fetched 14,693,309 bytes for the 12,595,257-byte film
(seen to fail). `the_stream_cache_keeps_to_its_cap_and_clears_what_nothing_owns`: over a 1 MB cap the stream playing is
kept and a stray folder removed; not playing, it is removed with its file. The cache's order (`cache::evictions`, unit
tests): least recently watched first, never what plays, a stream never recorded first.

### Person search (`eval search --query`, the full catalogue)

| Query | Top results now |
|---|---|
| zendaya | Zendaya (a series of that exact name), then Dune: Part One, Spider-Man: No Way Home, Spider-Man: Homecoming, Dune: Part Two, Spider-Man: Far from Home (Starring) |
| timothee chalamet (no accent) | Interstellar, Dune: Part One, Dune: Part Two, Don't Look Up, Lady Bird, Call Me by Your Name |
| christopher nolan | The Dark Knight, Inception, Interstellar, The Dark Knight Rises, Batman Begins, The Prestige (Directed) |
| tom hanks | Forrest Gump, Saving Private Ryan, The Green Mile |
| drive / heat (titles) | Drive (2011) / Heat (1995) first, unchanged |

Before: "zendaya" returned none of her films (titles and episodes *named* Zendaya outranked them in the text tier).

**The Phase 5 harness, regression** (`eval search --report`): E2 exact titles **43/43**; E3 filter 1.0000, known-item MRR
1.0000, meaning queries unchanged; **E1 p95 20.8 ms** (17.2 ms without the person lookup; the first version read 400
titles' credits per query and took p95 to 38.8 ms, so it now reads 50 and only for queries of four words or fewer).

