# Phase 13 — Download Manager and the Stream-vs-Download Advisor

**Status:** in_progress · **Depends on:** 7, 9, 12 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Downloading is first-class, and the app tells you honestly which is the better choice.

## Deliverables

A download manager: queue with priorities, pause/resume/cancel, per-download and global bandwidth limits, scheduling (start after midnight), completion notifications, and automatic filing into the local library so a completed download is indistinguishable from a file you already had. **A Downloads page** — its own surface, reached from the masthead's Downloads button, which shows live progress and the count of active items (ADR-0037), showing the queue, per-item progress, speed, seeds/peers, ETA and the reason each item was queued, with the same design language as every other screen. The manager without a surface was an inconsistency in this document: §7's feature list omitted it entirely. **A pluggable download backend** (ADR-0029): librqbit by default, and a user's own **qBittorrent over its Web API** when one is configured — detected, never bundled, never required, and falling back to librqbit if it is not running. The queue is backend-agnostic, so an item files into the library identically whichever moved the bytes. **qBittorrent is a transport only**: its search plugins, RSS feeds and tracker lists are never read, which would be a §2.1 violation arriving by the back door. **The advisor**, combining all four signals the author asked for: (1) measured swarm health versus the bitrate the file requires, producing a stall-probability estimate; (2) free disk space and existing local availability; (3) learned network conditions by time of day, suggesting overnight downloads for large files; (4) taste-model input — something predicted to be a favourite and a likely rewatch is worth the disk, something being sampled is not. Presented as a single clear recommendation with a one-line reason, not a wall of statistics, with the other option always one click away. Storage management: a library size budget, a "cleanup" view showing watched downloads by size and age, and per-item keep/delete decisions.

## Exit criteria

- [ ] **E1** The advisor's recommendation is correct (matches what actually happens) in at least 8 of 10 manual trials across varied swarm conditions.
- [ ] **E2** A download completed through the qBittorrent backend files into the library identically to one completed through librqbit, verified by comparing the resulting rows.
- [ ] **E3** With qBittorrent configured but not running, a queued download falls back to librqbit rather than failing.
- [ ] **E4** `tools/guard` rejects any code path that reads qBittorrent's search plugins, RSS feeds or tracker configuration.
- [x] **E5** Downloads survive an app restart and resume.
- [x] **E6** A completed download is automatically matched and appears in the local library with no user action.
- [ ] **E7** Bandwidth limits are actually respected, verified by measurement.
- [ ] **E8** The recommendation reason is a single readable sentence.

## Subtasks

- [x] **13.1** The download queue (crates/downloads): items with priority, state (queued, active, paused, done, failed), per-item and global limits, a schedule window (start after a time), surviving restart (migration), a backend trait with librqbit as the default; completion files the item into the library through sinephile_library::scan (E5, E6).
- [ ] **13.2** The qBittorrent backend over its Web API (ADR-0029): configured by the user (address, credentials kept like other secrets), detected, never bundled or required; falls back to librqbit when not running (E3); a transport only, with a guard rule rejecting any read of search plugins, RSS or tracker lists (E4), seen to fail. Tested against a local stand-in Web API; one real-qBittorrent check batched for the author (E2).
- [ ] **13.3** Bandwidth limits measured: a local seeder and a timed transfer under each limit, numbers to docs/eval-results.md (E7).
- [ ] **13.4** The advisor (crates/downloads advise.rs): stall probability from swarm throughput against the file's bitrate, free disk and an existing local copy, time-of-day network history, and the taste model's input (a neutral stand-in until Phase 15), giving one recommendation and one readable sentence (E8); trials recorded for E1.
- [ ] **13.5** The Downloads page (masthead Downloads button with a live count, ADR-0037): queue, progress, speed, seeds/peers, ETA, the reason each was queued, pause/resume/cancel, priorities, limits; storage budget and a cleanup view. Pad-complete in tools/uiaudit/controller.mjs.
- [ ] **13.6** Author checks batched in one message: real controller (Phase 8 E5), VPN adapter pulled mid-download (Phase 7 E7), a real qBittorrent round trip (E2), the review screen's feel, D77's release-build console check. Then docs/specs/downloads.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

Estimating whether a stream will stall; bitrate versus throughput; why the taste model belongs in this decision.

---

<!-- subtask log: appended during the phase -->

## Work log
- **13.1** The download queue (crates/downloads): items with priority, state (queued, active, paused, done, · `163095b`
