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
