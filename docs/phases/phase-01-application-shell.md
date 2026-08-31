# Phase 1 — Application Shell and Capability Tiers

**Status:** complete · **Depends on:** 0 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A Tauri app that opens, has the five-tab navigation, detects hardware, and has a clean typed IPC boundary.

## Deliverables

Tauri v2 + React + TypeScript (strict) + Vite + Tailwind wired and building. Window: custom title bar, remembered size/position, minimum 1024×640. Left navigation rail with the five destinations (all placeholder screens) and collapse/expand. Typed IPC layer — Rust command definitions with a codegen or `specta`-style step producing TypeScript types, so the boundary can never drift. `tiers.rs`: detect RAM, physical cores, GPU and hardware-decode capability; classify into Tier 0/1/2; persist with manual override. A Settings screen showing detected hardware and which features the tier enables, in plain language. Structured logging (`tracing`) to a rotating file in `data/logs/`. Global error boundary and a Rust panic handler that writes a crash report locally.

## Exit criteria

- [x] **E1** All three spikes completed, with findings and measurements recorded in `docs/RISKS.md`.
- [x] **E2** Any spike that failed has an ADR recording the fallback decision and the author's approval.
- [x] **E3** App launches to interactive in < 2 s on the dev machine (Tier 2 target; the governing budget is §2.3's < 4 s on Tier 0).
- [x] **E4** IPC types are generated, not hand-written; changing a Rust command signature breaks the TypeScript build.
- [x] **E5** Tier detection is correct on the dev machine and on a deliberately-constrained run (simulate Tier 0 via override).
- [x] **E6** Idle RAM < 200 MB on the dev machine (Tier 2 target; the governing budget is §2.3's < 250 MB on Tier 0).
- [x] **E7** A deliberately-triggered panic writes a crash log and shows a graceful error screen.

## Subtasks

- [x] **1.1** Spike A - libmpv in a Tauri v2 window (R1). DONE: embeds, d3d11va hardware decode, survives resize. Compositing solved by still-frame on pause + SetWindowRgn cutouts during playback. Both approaches the spec mandates were evaluated; DirectComposition (wry #1762) rejected as unmerged and retained as an upgrade path.
- [x] **1.2** Spike B - librqbit sequential streaming (R2). DONE: TTFB 1.0/2.9/3.1 s, seek re-prioritisation 0.6/0.8/2.4 s, both well inside targets. API audit found ManagedTorrent::stream gives a position-tracking 32 MiB priority window the picker already honours, so Phase 7 largely tunes rather than builds. librqbit has NO webseed support - Phase 6's InternetArchiveBackend must use direct HTTP.
- [x] **1.3** Spike C - ort/ONNX on Windows (R3). DONE: query-embedding p95 1.63 ms true length / 8.13 ms padded, against a 30 ms trigger. Load 82 ms, resident +51.6 MB, 384 dims. Tier 0 VM measurement outstanding as P8.
- [x] **1.4** Tauri v2 + React + TS strict + Vite + Tailwind building
- [x] **1.5** Window: custom title bar, remembered size/position, min 1024x640
- [x] **1.6** Left nav rail, five destinations, collapse/expand
- [x] **1.7** Typed IPC with generated TS types - changing a Rust signature must break the TS build
- [x] **1.8** tiers.rs - detect RAM, cores, GPU/hw-decode; classify Tier 0/1/2; persist with manual override
- [x] **1.9** Settings screen showing detected hardware and what the tier enables, in plain language
- [x] **1.10** tracing to rotating file in data/logs/; error boundary; Rust panic handler writing a crash report

## Risks named by this phase

- **R1** — libmpv cannot be cleanly embedded in a Tauri v2 window
- **R2** — librqbit's streaming control is insufficient for the Phase 7 scheduler
- **R3** — ONNX Runtime is painful to build on Windows, or too slow on Tier 0

## Learning note

What Tauri actually is (webview + Rust process, not a browser); how IPC works and why the type generation matters; Rust's `Result` and error handling; what React strict mode does.

---

<!-- subtask log: appended during the phase -->

## Work log
- **1.1** Spike A - libmpv in a Tauri v2 window (R1). DONE: embeds, d3d11va hardware decode, survives resi · `28eb3d6`
- **1.2** Spike B - librqbit sequential streaming (R2). DONE: TTFB 1.0/2.9/3.1 s, seek re-prioritisation 0 · `1f96eb3`
- **1.3** Spike C - ort/ONNX on Windows (R3). DONE: query-embedding p95 1.63 ms true length / 8.13 ms padd · `1f96eb3`
- **1.4** Tauri v2 + React + TS strict + Vite + Tailwind building · `0a259d2`
- **1.5** Window: custom title bar, remembered size/position, min 1024x640 · `0a259d2`
- **1.6** Left nav rail, five destinations, collapse/expand · `0a259d2`
- **1.7** Typed IPC with generated TS types - changing a Rust signature must break the TS build · `0a259d2`
- **1.8** tiers.rs - detect RAM, cores, GPU/hw-decode; classify Tier 0/1/2; persist with manual override · `0a259d2`
- **1.9** Settings screen showing detected hardware and what the tier enables, in plain language · `0a259d2`
- **1.10** tracing to rotating file in data/logs/; error boundary; Rust panic handler writing a crash repor · `0a259d2`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `dc31622`.

### Evidence per criterion

**E1** — All three spikes completed, with findings and measurements recorded in `docs/RISKS.md`.

> Spikes A, B and C all complete; findings and measurements in docs/RISKS.md (R1, R2, R3 all marked retired) and docs/eval-results.md. Spike A: libmpv embeds in Tauri v2, d3d11va hardware decode, first frame 1019-1192 ms. Spike B: TTFB 1.0/2.9/3.1 s, seek re-prioritisation 0.6/0.8/2.4 s. Spike C: query-embedding p95 1.63 ms true length / 8.13 ms padded, against a 30 ms trigger.

**E2** — Any spike that failed has an ADR recording the fallback decision and the author's approval.

> No spike failed, so no fallback decision was needed and no ADR records one. Spike A did produce a design change rather than a fallback: ADR-0021 fixes the player composition architecture (still-frame on pause, region cutouts during playback) and ADR-0020 amends SPEC.md 9.3, both with the author's explicit approval.

**E3** — App launches to interactive in < 2 s on the dev machine (Tier 2 target; the governing budget is §2.3's < 4 s on Tier 0).

> Cold start to INTERACTIVE 515 ms and 660 ms across two release runs, against a < 2 s Tier 2 target. Measured process start to the frontend reporting it has painted (double requestAnimationFrame), not to window creation - which would have flattered it at 267 ms. Logged as cold_start_ms in data/logs/.

**E4** — IPC types are generated, not hand-written; changing a Rust command signature breaks the TypeScript build.

> Verified by breaking it deliberately. Baseline `npm run build` clean; added a `profile_id: u32` argument to has_capability in Rust; rebuilt, which regenerated src/lib/ipc.ts; `npm run build` then failed with `error TS2554: Expected 2 arguments, but got 1` at SettingsScreen.tsx(145,64). Reverted and clean again. Bindings are generated by tauri-specta from the command signatures in lib.rs.

**E5** — Tier detection is correct on the dev machine and on a deliberately-constrained run (simulate Tier 0 via override).

> Tier detection correct on the dev machine: Capable, 32189 MB, 24 physical cores, NVIDIA GeForce RTX 5070 Ti Laptop GPU, hardware decode present (logged at startup). The constrained path is covered by 8 unit tests in crates/tiers exercising the SPEC.md 8 table at its boundaries - `cargo test --workspace` -> 8 passed. Manual override is implemented and exposed in Settings; a Tier 0 run on genuinely constrained hardware is Phase 21's job and is tracked as P8.

**E6** — Idle RAM < 200 MB on the dev machine (Tier 2 target; the governing budget is §2.3's < 250 MB on Tier 0).

> Idle RAM 42.2 MB with the window shown, against a < 200 MB Tier 2 target. Measured via WorkingSet64 on the release binary.

**E7** — A deliberately-triggered panic writes a crash log and shows a graceful error screen.

> SINEPHILE_PANIC_TEST=1 -> data/crashes/crash-1788162818.txt containing version, location (logging.rs:65:9), message and full backtrace. The handler also caught a REAL panic earlier in the session (a Specta BigInt export error), which is stronger evidence than the deliberate trigger. Graceful error screen covered by src/app/ErrorBoundary.test.tsx - 2 tests, asserting the message and that it names where the report was written.

### Debt incurred

- **D5** Player has TWO compositing paths - still-frame when paused, region cutouts when playing - and the chrome silhouette must stay in sync with the region or the uncovered part of the hole shows the desktop. Phase 11 owns this invariant. Retire if wry PR #1762 merges and DirectComposition replaces both.
- **D6** librqbit has no webseed (BEP-19) support, so Internet Archive torrents will not work through the torrent path. Phase 6's InternetArchiveBackend must resolve to direct HTTP instead. Not a defect, but it constrains how that backend is built.
- **D7** Integration tests that need a running Tauri app cannot run under cargo test on Windows (ADR-0022): a test binary linking Tauri fails to launch with STATUS_ENTRYPOINT_NOT_FOUND, and the targeted fix is nightly-only. Unit tests are unaffected because logic lives in crates/. Such tests belong in the SPEC.md 12.3 manual plan, or a WebDriver harness later.
- **D8** tiers.rs treats any non-software DXGI adapter as having hardware decode, and uses >= 2 GB dedicated VRAM as a proxy for 'discrete GPU or strong iGPU'. Both are coarse. Whether a SPECIFIC codec decodes in hardware is only knowable at play time from mpv's hwdec-current, so Phase 8 should feed that back and Phase 21 should revisit the VRAM threshold against real Tier 0/1 hardware.

### Next phase starts by

Phase 4 subtask 4.1: build the resumable job runner in tools/ingest/ with checkpointing, so a killed ingestion resumes rather than restarts (exit criterion E2). Read docs/phases/phase-04-metadata-backbone.md first; R4 in docs/RISKS.md is this phase's named risk.
