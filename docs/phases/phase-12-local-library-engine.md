# Phase 12 — Local Library Engine

**Status:** complete · **Depends on:** 4 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Point it at a folder. It figures out what everything is, correctly.

## Deliverables

Folder registration with recursive scanning and live filesystem monitoring (`ReadDirectoryChangesW` via `notify`), handling moves, renames, and deletions without a rescan. `crates/filename-parser`: a standalone, well-tested parser extracting title, year, season, episode, absolute episode number, resolution, source (BluRay/WEB/etc.), codec, audio, language, and release group from real-world filenames — including scene releases, anime fansub conventions, and multi-episode files. **Multi-signal matching**: combine parsed filename data with actual file evidence — duration from FFprobe, resolution, audio track languages, embedded container title metadata, folder context, sibling files — and score candidates from the catalogue. A tunable confidence threshold. **The review queue**: only genuinely ambiguous items surface, presented as a side-by-side poster comparison resolved with one click, with the option to search manually. Decisions are remembered and used to improve future matching of similar names. Batch resolution for series where one decision settles many files. `fixtures/filenames/` corpus (500+ hand-labelled cases) and the identification eval harness.

## Exit criteria

- [x] **E1** > 95% top-1 accuracy on the fixture corpus.
- [x] **E2** < 1% false-confident rate — being wrong while confident is the worst failure mode here.
- [x] **E3** Scanning 10,000 files completes in under 3 minutes and does not block the UI.
- [x] **E4** Renaming a file outside the app is detected and does not create a duplicate.
- [x] **E5** The review queue is genuinely pleasant to use — resolving 20 ambiguous items takes under a minute.
- [x] **E6** `crates/filename-parser` is documented and independently useful.

## Subtasks

- [x] **12.1** crates/filename-parser (MIT/Apache, pure, no I/O): title, year, season, episode (S01E02, 1x02, multi-episode ranges), absolute episode for anime-style 'Title - 059', resolution, source, codec, audio, language, release group (bracket prefix or dash suffix, recognised by position, never by a list of names). fixtures/filenames/: 500+ hand-labelled names covering scene, anime fansub, multi-episode, messy and adversarial cases; every group name invented (2.1); a per-field accuracy test. Documented with a runnable README example (E6).
- [x] **12.2** Migration 0021 library: library_folders (the user's own roots, machine-specific like local_files.path; the portability test's allow-list grows by that column with the reason), local_files gains folder_id and a missing_since for moves; match_memory (a normalised name pattern -> media item, from the user's decisions). LibraryRepository with a repository_surface line per method.
- [x] **12.3** crates/library identification: candidates from the catalogue by title (FTS) and year, scored on independent weak signals (title similarity, year, duration vs runtime from FFprobe, episode exists, folder and sibling context, remembered decisions), a confidence in [0,1], auto-accepted above a threshold set from measurement (DECISIONS_PENDING), else the review queue. tools/eval 'identify' harness over fixtures/filenames labelled with catalogue ids: top-1 (E1 > 95%) and false-confident (E2 < 1%); numbers to docs/eval-results.md.
- [x] **12.4** Scanner and watcher: register a folder, walk it recursively, probe only what identification needs, in parallel, off the UI thread with progress events; the notify crate (ReadDirectoryChangesW) applies creates, deletes, renames and moves without a rescan; a rename or move keeps the row (no duplicate, E4). Measured: 10,000 files under 3 minutes with the UI responsive (E3).
- [x] **12.5** The review queue (Settings or Library): side-by-side candidates, typographic when there is no artwork, one click or one A press resolves, manual search for the rest, one decision settles a whole series' files; remembered for similar names. The library feeds LocalFileBackend so a local film is found by search and plays from its page (discharges Phase 8's carried criterion). Pad-only flow in tools/uiaudit/controller.mjs; resolving 20 items timed (E5).
- [x] **12.6** docs/specs/library.md, learning note, README and HOW_IT_WORKS, regression check, close.

## Learning note

Parsing messy real-world input; why multiple weak signals beat one strong one; confidence thresholds and the asymmetry between being wrong and being unsure; filesystem watching on Windows.

---

<!-- subtask log: appended during the phase -->

## Work log
- **12.1** crates/filename-parser (MIT/Apache, pure, no I/O): title, year, season, episode (S01E02, 1x02, m · `0097432`
- **12.2** Migration 0021 library: library_folders (the user's own roots, machine-specific like local_files · `9de8483`
- **12.3** crates/library identification: candidates from the catalogue by title (FTS) and year, scored on  · `c0eda95`
- **12.4** Scanner and watcher: register a folder, walk it recursively, probe only what identification need · `96a73fa`
- **12.5** The review queue (Settings or Library): side-by-side candidates, typographic when there is no ar · `b96f9fd`
- **12.6** docs/specs/library.md, learning note, README and HOW_IT_WORKS, regression check, close. · `c0a782e`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `b5dccda`.

### Evidence per criterion

**E1** — > 95% top-1 accuracy on the fixture corpus.

> c0eda95: cargo run --release -p eval -- identify (2026-11-07, full catalogue): tuning set fixtures/filenames/names.psv 549 names, 2 labels not in the catalogue left out, top-1 523/528 = 99.05%; held out heldout.psv 108/108 = 100%. Method and every tuning step with its measured number in docs/eval-results.md (Phase 12, Identification). crates/library/tests/identify.rs: 7 tests (year, duration, episode over film namesake, folders, remembered decision, no guess); seen to fail when the kind signal was neutralised.

**E2** — < 1% false-confident rate — being wrong while confident is the worst failure mode here.

> c0eda95: same run, at the library's own threshold sinephile_library::ACCEPT_AT = 0.75: tuning set 2/510 accepted were wrong = 0.39% (0.37% of all names; strict reading: of accepted names, and a '?'-labelled name accepted counts as wrong); held out 0/103. Asked (review queue) 6.8% and 5.5%. The lowest threshold under 1% was 0.615 at 0.97%; 0.75 keeps a margin. 9 of 549 labels were corrected, each reason recorded in docs/eval-results.md.

**E3** — Scanning 10,000 files completes in under 3 minutes and does not block the UI.

> 96a73fa: cargo run --release -p eval -- scan (2026-11-07, full catalogue, real FFprobe): 10,000 distinct films sampled from the catalogue, one scene-named file each in its own folder, scanned in 37.1 s, 35.3 s and 35.0 s (target 180 s), 3.5-3.7 ms a file, identification 7 at a time. Not blocking the UI: the queries a page issues ran alongside, p95 24.6-24.7 ms idle and 33.5-34.9 ms during the scan (515-542 runs). Scans run on background tasks (src-tauri/src/commands/library.rs start_scan) with progress events. Accuracy on the sample: best right 99.45-99.54%, accepted wrong 1/9,279. Table in docs/eval-results.md.

**E4** — Renaming a file outside the app is detected and does not create a duplicate.

> 96a73fa: crates/library/tests/scan.rs a_scan_finds_identifies_and_keeps_a_renamed_file (renamed while closed: same row, same match, no duplicate) and the_watcher_sees_a_rename_and_keeps_one_row (the real notify watcher on a temp folder); both seen to fail with move detection disabled. Real window 2026-11-07: a 2.2 GB file in a library folder renamed in place while the app ran (the watcher reported a removal then an arrival: Missing(1) then Moved), one row, the same id, matched to Heat (1995) at 1.0; relaunched, the startup scan reported unchanged 1 moved 0; moved to another folder and renamed Heat.mkv while closed, the next launch reported moved 1, the same row and match.

**E5** — The review queue is genuinely pleasant to use — resolving 20 ambiguous items takes under a minute.

> b96f9fd: tools/uiaudit/controller.mjs (CI step, pad only): from Settings the D-pad reaches 'Check them', and 20 waiting files are answered by A alone in 5.3 s (each A answers the focused first card). Real window 2026-11-07 against the full catalogue: a library of 22 files (21 genuinely ambiguous: 12 remakes with no year, 5 Monster episodes, 4 The Office episodes with no country) scanned in 1.3 s with 21 asked; answered by keyboard, 14 answers settled all 21 (one answer per series settled its other episodes), the app taking 20-290 ms per answer from key to the next file on screen (0.8 s in all), so the minute is the person's reading time. src/features/library/ReviewScreen.test.tsx (5 tests) and crates/library/tests/review.rs one_answer_settles_the_whole_series_and_is_remembered (seen to fail without remembering). The author's own impression of the screen joins the Phase 13 batch of author checks with the screenshot.

**E6** — `crates/filename-parser` is documented and independently useful.

> 8c3a377: crates/filename-parser (MIT OR Apache-2.0, one dependency: regex =1.13.1, no I/O) documented by README.md included as the crate doc, whose example runs as a doctest (cargo test -p filename-parser: 1 doctest passed), plus examples/parse.rs for the command line. Measured: tests/corpus.rs the_corpus_reads_as_a_human_reads_it, 549 hand-labelled names in fixtures/filenames/names.psv, 100% on every field after tuning (98.36% before); names_it_was_never_tuned_on_read_as_well, 109 names in heldout.psv written after tuning, 109/109 on the first run; tests/fields.rs (7 tests: languages, edition, flags, audio, dates, episode titles, extension, comparable). Seen to fail: dropping the year made both corpus tests fail (year 32.24% < 97%). Release groups in the fixtures are invented (2.1).

### Debt incurred

- **D73** A multi-episode file (S01E01E02) is matched to its first episode with covers = 2, but LocalFileBackend only finds files matched to the exact episode asked for, so episode 2 does not offer it. Look up files whose match covers the asked episode (crates/sources/src/backends/local.rs item_for, LibraryRepository::files_for_item).
- **D74** A file removed while the app runs is marked missing by the watcher but only purged at the next scan (launch or Rescan). Missing files are already hidden from every view; purge them after a quiet period in library_loop if the rows ever matter.
- **D75** The held-out filename corpus (109 names) was written by the same hand that tuned the parser, and its identification labels were seen in --report output while tuning. A truly independent check is a real user's library: count asked, accepted and wrong after a first real scan (Phase 14's onboarding test is the natural moment).
- **D76** Two remakes with the same title and year (rare, but real) can only be told apart by duration, which needs FFprobe; a file FFprobe cannot read stays in the review queue. Container title tags (probed) are stored but not yet weighed as a signal.
- **D77** CREATE_NO_WINDOW is now set on FFprobe and the subtitle aligner's FFmpeg; confirm in a release build (windows subsystem) that no console window flashes during a scan or an alignment. Batched with the Phase 13 author checks.

### Next phase starts by

Phase 13 subtask 13.1 (docs/phases/phase-13-download-manager.md): read crates/torrent (the engine API) and src-tauri/src/commands/torrent.rs; design crates/downloads with a Backend trait and a Queue persisted by migration 0023 (downloads: id, media_item_id, magnet or source key, backend, priority, state, bytes, limits, not_before, reason); librqbit backend; on completion call sinephile_library::scan::record + identify_file for the finished file.
