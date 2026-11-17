# 0053 — Search by meaning ships inside the installer

- **Status:** Accepted 2026-11-17. The author asked for it ("i need this feature to ship with the application
  installer"), round 9 of the fix-list questions (`docs/design/author-brief-2026-11-17.md`).
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** ADR-0014 (static downloads, with consent) and ADR-0050 (a new install fetches it during setup):
  an installed copy fetches nothing for search by meaning. `SPEC.md` §2.7; spec_version 4.6.0.
- **Found with it (D90):** the author's install had downloaded everything, and "Build the index" never built.

## Context

On the author's install the meaning file (329.5 MB) was the right size with a sound header and 5 MB of its own data a
megabyte late, 66 MiB in: a resumed download had been answered from an earlier byte than it asked for, and the
downloader trusted every `206` to start where it asked. The artefact's own checksum caught it, and then four things
let it stand: the bad file was kept; the screen judged files by their header alone, so it said everything was
downloaded and offered "Build the index"; the build failed on the checksum every time; and the button's error was
shown but never logged. Separately, the author wants search by meaning to work from the installer, with no download.

## Decision

- **The installer carries the three files** (the language model, its tokenizer, the published meaning file) in
  `meaning/` beside the executable, laid out as a data folder is. They come from the release machine's data folder,
  and `tools/bundle/check_meaning.py` refuses to build the installer unless each one's whole-file sha256 is the
  published file's. `npm run release` (`src-tauri/tauri.release.conf.json`) builds it; `cargo build` and
  `npm run tauri build` stay without them, for development.
- **The installed copy is read first** (`MeaningFiles` in `crates/catalogue/src/assets.rs`), the data folder
  otherwise. A downloaded copy that the installed one replaces is removed.
- **The index builds by itself once the catalogue is searchable**, from what is on disk, with nothing fetched: on a
  new install after its first titles load, on an existing one at the next launch. About 30 s.
- **The downloads that remain are fixed.** A resume is a continuation only when its `Content-Range` starts where it
  asked; otherwise the whole file is fetched again. A download that fails its checks is deleted. Before building,
  every file is checked in full, not by its header. Failures are logged.
- A copy without the installed files (built with cargo, or portable without them) keeps ADR-0050's behaviour: setup
  fetches on a new install, and an existing one is offered the download. The launch-time build never downloads.

## Consequences

- The installer grows by about 350 MB.
- The posture is unchanged: the meaning file is the one already published as a release asset (ADR-0014), carrying no
  catalogue (ADR-0044); the installer now carries the same file. The model is MIT-licensed and carried with its
  identity pinned by hash.
- An upgrade fixes an install like the author's by itself: the installed copy replaces the corrupt download, which is
  deleted, and the index builds at the next launch.
