# Phase 12 — Local Library Engine

**Status:** in_progress · **Depends on:** 4 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Point it at a folder. It figures out what everything is, correctly.

## Deliverables

Folder registration with recursive scanning and live filesystem monitoring (`ReadDirectoryChangesW` via `notify`), handling moves, renames, and deletions without a rescan. `crates/filename-parser`: a standalone, well-tested parser extracting title, year, season, episode, absolute episode number, resolution, source (BluRay/WEB/etc.), codec, audio, language, and release group from real-world filenames — including scene releases, anime fansub conventions, and multi-episode files. **Multi-signal matching**: combine parsed filename data with actual file evidence — duration from FFprobe, resolution, audio track languages, embedded container title metadata, folder context, sibling files — and score candidates from the catalogue. A tunable confidence threshold. **The review queue**: only genuinely ambiguous items surface, presented as a side-by-side poster comparison resolved with one click, with the option to search manually. Decisions are remembered and used to improve future matching of similar names. Batch resolution for series where one decision settles many files. `fixtures/filenames/` corpus (500+ hand-labelled cases) and the identification eval harness.

## Exit criteria

- [ ] **E1** > 95% top-1 accuracy on the fixture corpus.
- [ ] **E2** < 1% false-confident rate — being wrong while confident is the worst failure mode here.
- [ ] **E3** Scanning 10,000 files completes in under 3 minutes and does not block the UI.
- [ ] **E4** Renaming a file outside the app is detected and does not create a duplicate.
- [ ] **E5** The review queue is genuinely pleasant to use — resolving 20 ambiguous items takes under a minute.
- [x] **E6** `crates/filename-parser` is documented and independently useful.

## Subtasks

- [x] **12.1** crates/filename-parser (MIT/Apache, pure, no I/O): title, year, season, episode (S01E02, 1x02, multi-episode ranges), absolute episode for anime-style 'Title - 059', resolution, source, codec, audio, language, release group (bracket prefix or dash suffix, recognised by position, never by a list of names). fixtures/filenames/: 500+ hand-labelled names covering scene, anime fansub, multi-episode, messy and adversarial cases; every group name invented (2.1); a per-field accuracy test. Documented with a runnable README example (E6).
- [x] **12.2** Migration 0021 library: library_folders (the user's own roots, machine-specific like local_files.path; the portability test's allow-list grows by that column with the reason), local_files gains folder_id and a missing_since for moves; match_memory (a normalised name pattern -> media item, from the user's decisions). LibraryRepository with a repository_surface line per method.
- [ ] **12.3** crates/library identification: candidates from the catalogue by title (FTS) and year, scored on independent weak signals (title similarity, year, duration vs runtime from FFprobe, episode exists, folder and sibling context, remembered decisions), a confidence in [0,1], auto-accepted above a threshold set from measurement (DECISIONS_PENDING), else the review queue. tools/eval 'identify' harness over fixtures/filenames labelled with catalogue ids: top-1 (E1 > 95%) and false-confident (E2 < 1%); numbers to docs/eval-results.md.
- [ ] **12.4** Scanner and watcher: register a folder, walk it recursively, probe only what identification needs, in parallel, off the UI thread with progress events; the notify crate (ReadDirectoryChangesW) applies creates, deletes, renames and moves without a rescan; a rename or move keeps the row (no duplicate, E4). Measured: 10,000 files under 3 minutes with the UI responsive (E3).
- [ ] **12.5** The review queue (Settings or Library): side-by-side candidates, typographic when there is no artwork, one click or one A press resolves, manual search for the rest, one decision settles a whole series' files; remembered for similar names. The library feeds LocalFileBackend so a local film is found by search and plays from its page (discharges Phase 8's carried criterion). Pad-only flow in tools/uiaudit/controller.mjs; resolving 20 items timed (E5).
- [ ] **12.6** docs/specs/library.md, learning note, README and HOW_IT_WORKS, regression check, close.

## Learning note

Parsing messy real-world input; why multiple weak signals beat one strong one; confidence thresholds and the asymmetry between being wrong and being unsure; filesystem watching on Windows.

---

<!-- subtask log: appended during the phase -->

## Work log
- **12.1** crates/filename-parser (MIT/Apache, pure, no I/O): title, year, season, episode (S01E02, 1x02, m · `0097432`
- **12.2** Migration 0021 library: library_folders (the user's own roots, machine-specific like local_files · `9de8483`
