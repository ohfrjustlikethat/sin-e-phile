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
