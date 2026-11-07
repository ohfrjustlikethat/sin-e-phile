# Phase 13 — Download Manager and the Stream-vs-Download Advisor

**Status:** complete · **Depends on:** 7, 9, 12 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Downloading is first-class, and the app tells you honestly which is the better choice.

## Deliverables

A download manager: queue with priorities, pause/resume/cancel, per-download and global bandwidth limits, scheduling (start after midnight), completion notifications, and automatic filing into the local library so a completed download is indistinguishable from a file you already had. **A Downloads page** — its own surface, reached from the masthead's Downloads button, which shows live progress and the count of active items (ADR-0037), showing the queue, per-item progress, speed, seeds/peers, ETA and the reason each item was queued, with the same design language as every other screen. The manager without a surface was an inconsistency in this document: §7's feature list omitted it entirely. **A pluggable download backend** (ADR-0029): librqbit by default, and a user's own **qBittorrent over its Web API** when one is configured — detected, never bundled, never required, and falling back to librqbit if it is not running. The queue is backend-agnostic, so an item files into the library identically whichever moved the bytes. **qBittorrent is a transport only**: its search plugins, RSS feeds and tracker lists are never read, which would be a §2.1 violation arriving by the back door. **The advisor**, combining all four signals the author asked for: (1) measured swarm health versus the bitrate the file requires, producing a stall-probability estimate; (2) free disk space and existing local availability; (3) learned network conditions by time of day, suggesting overnight downloads for large files; (4) taste-model input — something predicted to be a favourite and a likely rewatch is worth the disk, something being sampled is not. Presented as a single clear recommendation with a one-line reason, not a wall of statistics, with the other option always one click away. Storage management: a library size budget, a "cleanup" view showing watched downloads by size and age, and per-item keep/delete decisions.

## Exit criteria

- [x] **E1** The advisor's recommendation is correct (matches what actually happens) in at least 8 of 10 manual trials across varied swarm conditions.
- [x] **E2** A download completed through the qBittorrent backend files into the library identically to one completed through librqbit, verified by comparing the resulting rows.
- [x] **E3** With qBittorrent configured but not running, a queued download falls back to librqbit rather than failing.
- [x] **E4** `tools/guard` rejects any code path that reads qBittorrent's search plugins, RSS feeds or tracker configuration.
- [x] **E5** Downloads survive an app restart and resume.
- [x] **E6** A completed download is automatically matched and appears in the local library with no user action.
- [x] **E7** Bandwidth limits are actually respected, verified by measurement.
- [x] **E8** The recommendation reason is a single readable sentence.

## Subtasks

- [x] **13.1** The download queue (crates/downloads): items with priority, state (queued, active, paused, done, failed), per-item and global limits, a schedule window (start after a time), surviving restart (migration), a backend trait with librqbit as the default; completion files the item into the library through sinephile_library::scan (E5, E6).
- [x] **13.2** The qBittorrent backend over its Web API (ADR-0029): configured by the user (address, credentials kept like other secrets), detected, never bundled or required; falls back to librqbit when not running (E3); a transport only, with a guard rule rejecting any read of search plugins, RSS or tracker lists (E4), seen to fail. Tested against a local stand-in Web API; one real-qBittorrent check batched for the author (E2).
- [x] **13.3** Bandwidth limits measured: a local seeder and a timed transfer under each limit, numbers to docs/eval-results.md (E7).
- [x] **13.4** The advisor (crates/downloads advise.rs): stall probability from swarm throughput against the file's bitrate, free disk and an existing local copy, time-of-day network history, and the taste model's input (a neutral stand-in until Phase 15), giving one recommendation and one readable sentence (E8); trials recorded for E1.
- [x] **13.5** The Downloads page (masthead Downloads button with a live count, ADR-0037): queue, progress, speed, seeds/peers, ETA, the reason each was queued, pause/resume/cancel, priorities, limits; storage budget and a cleanup view. Pad-complete in tools/uiaudit/controller.mjs.
- [x] **13.6** Author checks batched in one message: real controller (Phase 8 E5), VPN adapter pulled mid-download (Phase 7 E7), a real qBittorrent round trip (E2), the review screen's feel, D77's release-build console check. Then docs/specs/downloads.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

Estimating whether a stream will stall; bitrate versus throughput; why the taste model belongs in this decision.

---

<!-- subtask log: appended during the phase -->

## Work log
- **13.1** The download queue (crates/downloads): items with priority, state (queued, active, paused, done, · `163095b`
- **13.2** The qBittorrent backend over its Web API (ADR-0029): configured by the user (address, credential · `f5345b5`
- **13.3** Bandwidth limits measured: a local seeder and a timed transfer under each limit, numbers to docs · `35decf3`
- **13.4** The advisor (crates/downloads advise.rs): stall probability from swarm throughput against the fi · `92c070d`
- **13.5** The Downloads page (masthead Downloads button with a live count, ADR-0037): queue, progress, spe · `5d06e81`
- **13.6** Author checks batched in one message: real controller (Phase 8 E5), VPN adapter pulled mid-downl · `fca4cf6`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `1d1184a`.

### Evidence per criterion

**E1** — The advisor's recommendation is correct (matches what actually happens) in at least 8 of 10 manual trials across varied swarm conditions.

> 92c070d: cargo run --release -p eval -- advise (2026-11-07), ten fresh loopback swarm conditions chosen after the model's last change and run once (bitrates 0.8, 2 and 3 MB/s; one to four capped seeders; 0.6x to 2.5x the bitrate): each measured as the app measures, advised, then played from a fresh engine at its bitrate after a 3-second buffer. 9 of 10 right (target at least 8): the miss was four slow peers at 0.86 MB/s for a 0.8 MB/s film, judged 4% likely to pause, which paused once. The first run on twelve other conditions scored 7/12 and found the engine's speed bug (7e18942); both runs and every change between them are in docs/eval-results.md. Caveat recorded there: a loopback swarm is steadier than the internet.

**E2** — A download completed through the qBittorrent backend files into the library identically to one completed through librqbit, verified by comparing the resulting rows.

> cargo test -p sinephile-downloads --test qbittorrent_real -- --ignored --nocapture (2026-11-07, qBittorrent v5.2.0 installed here; two instances isolated from the user's own by --profile, Web UIs on 127.0.0.1, each verified as the isolated one before use): the same 6,303,801-byte download through the app's Qbittorrent backend and through librqbit filed identically field by field: Done, done/total 6,303,801, file Heat/Heat.1995.1080p.mkv relative to Downloads, library size 6,303,801, match Heat 1.0 confirmed Metadata covers 1, folder registered. Seen to fail first on the real thing: qBittorrent's copy landed at Heat.1995.1080p/... until the backend used the per-title folder (contentLayout=NoSubfolder). docs/eval-results.md 'E2: a real qBittorrent round trip'.

**E3** — With qBittorrent configured but not running, a queued download falls back to librqbit rather than failing.

> f5345b5: crates/downloads/tests/queue.rs a_configured_qbittorrent_that_is_not_running_falls_back_to_librqbit: the backends are [qBittorrent at an address nothing answers, librqbit]; a download meant for qBittorrent is started on the first tick (not failed), its row says librqbit moved the bytes, and it completes from a loopback seeder. Seen to fail with the fallback removed from Manager::usable. qBittorrent's availability is its app/version answering (crates/downloads/tests/qbittorrent.rs not_running_is_unavailable_and_running_is_available).

**E4** — `tools/guard` rejects any code path that reads qBittorrent's search plugins, RSS feeds or tracker configuration.

> f5345b5: tools/guard/guard.py rule qbittorrent-content (search and RSS endpoints, the tracker-list endpoints, the setting that adds trackers to every torrent), with four must-fire vectors and two must-not-fire vectors (the transport endpoints the backend does use) in tools/guard/tests/vectors.py; python tools/guard/guard.py --selftest passes all, and was seen to fail: a quoting slip turned the rule's word boundaries into control characters and the self-test reported the two tracker vectors missed until fixed. The backend's own module lists the only endpoints it uses.

**E5** — Downloads survive an app restart and resume.

> 163095b: crates/downloads/tests/queue.rs the_queue_survives_a_restart_and_carries_on: a 16 MiB download from a slow loopback seeder stopped mid-way (engine stopped, manager dropped), a new engine on the same folder and a new manager over the same queue: the row is still active, the download is added back from the queue's own row with the pieces on disk kept (progress >= before), and it completes. Seen to fail with re-adding disabled. Engine-level persistence: crates/torrent/tests/swarm.rs a_download_survives_a_restart (Phase 7).

**E6** — A completed download is automatically matched and appears in the local library with no user action.

> 163095b: crates/downloads/tests/queue.rs a_finished_download_files_itself_into_the_library_already_matched: a download for Heat (1995) completes through the manager, lands in the Downloads folder (registered as a library folder), and has a library row matched to Heat, confirmed, with files_for_item(Heat) offering it; a download with no catalogue item is identified like any library file (identify_file).

**E7** — Bandwidth limits are actually respected, verified by measurement.

> be543da: crates/downloads/tests/limits.rs (cargo test -p sinephile-downloads --test limits -- --nocapture --test-threads=1), a loopback seeder with no limit and a 48 MiB file through the queue: a per-download limit of 1.00 MiB/s measured 1.02 MiB/s (twice); the global limit of 2.00 MiB/s (Engine::set_limits) measured 2.01 and 2.00 MiB/s; with no limit the same swarm ran at 157.2 MiB/s (48 MiB in 0.31 s), so the limit is what held it. Each held to within 15%. Numbers in docs/eval-results.md (Phase 13).

**E8** — The recommendation reason is a single readable sentence.

> 92c070d: crates/downloads/src/advise.rs tests: every branch (stream, download, tonight, no room, local copy, nobody sharing, a favourite) is checked by one_sentence (a single full stop at the end, no other sentence break, no line break, at most 160 characters); every sentence in the twenty-two trials read as one, e.g. 'It would pause to buffer if streamed (1.8 MB/s against the 2.0 MB/s it needs), so download it first, about a minute.'

### Debt incurred

- **D78** Completion notices are in-app toasts only (src/features/downloads/DownloadNotices.tsx). A Windows notification when the window is not in front needs the Tauri notification plugin; adding a plugin upgraded Tauri last time (the dialog plugin), so it waits for Phase 20's Windows integration, alongside the media controls.
- **D79** The advisor runs only for a torrent copy the film page already started (the plan's chosen candidate). A web stream is not downloadable, so nothing is offered; a title whose best copy is a web stream but which also has a torrent copy is not offered the torrent to download. Revisit with the Phase 15 taste input.
- **D80** Upstream (librqbit 9.0.1): create_torrent writes one piece hash too many when the content is a whole number of pieces (500 bytes of hashes for 24 pieces of 256 KiB); libtorrent refuses such a torrent as invalid (stays fetching metadata, then errors; a .torrent of it is never added). The app never creates torrents, so only test fixtures are affected; crates/downloads/tests/qbittorrent_real.rs uses a size that is not a whole number. Worth reporting upstream (the author's call: an outward-facing issue).

### Next phase starts by

Phase 14 subtask 14.1 (docs/phases/phase-14-profiles.md): write migration 0024_profiles in crates/persistence/migrations (avatar_colour, pin_salt, pin_hash, is_kids, kids_limit, last_used_at), extend ProfileRepository with create/rename/delete/set_pin/verify_pin/set_kids/touch plus lines in tests/repository_surface.rs, then add AppState::profile() in src-tauri/src/state.rs and replace the 14 ensure_default() calls in src-tauri/src/commands.
