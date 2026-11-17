# 0051 — The meaning index is keyed by IMDb id, not by position

- **Status:** Accepted 2026-11-17.
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** ADR-0014 (the published embedding artefact), D44/D45 (the prefix check).
- **Found:** on a copy of the author's install, pressing "Download" for search by meaning (round 8).

## Context

The published artefact (`embeddings-v3`, 858,170 vectors) is positional: vector *n* is the nth core title of the
catalogue it was built from, in that catalogue's id order, and its header carries a digest of those ids. Ids are row
numbers, assigned as a catalogue is loaded. **No install has the producer's ids**: each loads its own snapshot of the
IMDb datasets through its own first run (ADR-0032: the files sort as text, and new titles land mid-file), so its rows
are numbered differently. `verify_prefix` refused the artefact on every machine but the one that made it, correctly,
and search by meaning has never worked on an install. On the author's copy: "position 858169 is now id 2615642".

A second gap stood behind it: which vectors are worth indexing is decided by text (a synopsis of 300+ characters;
sparse documents are louder than rich ones). The producer's catalogue has 233,018 Wikipedia synopses; an install has
none, so even correctly addressed it would have indexed nothing.

Two other defects were found on the way: the download saved the tokenizer under a name nothing loads, and the offer's
heading was drawn in the background colour.

## Decision

- **A key file names each vector by IMDb id**: `crates/embedding/src/keys.rs`. It lists only the positions the
  producer judged worth indexing, each with its IMDb number, and carries the artefact's own `id_digest` and count, so
  it applies to that artefact and no other.
- **Written where the artefact was made** by `ingest artefact-keys`, which refuses unless that catalogue's first core
  ids are exactly the artefact's. For `embeddings-v3`: 119,874 positions, 959 KB, all with an IMDb id.
- **Compiled into the app** (`crates/catalogue/assets/`), so the 314 MB published artefact is used as it is: nothing
  is republished. The build maps each IMDb number to this install's own id; a title this install lacks is skipped.
- Without a key file that names the artefact (the producer's own machine, or a future artefact), the build is
  positional and checked by `verify_prefix`, as before.
- The next artefact should carry its keys inside it (a format bump), which retires the separate file.

## Consequences

- Search by meaning works on an install: on the author's copy the index built in 30 s (61 MB), and "a heist where the
  plan goes wrong" returned Tower Heist, Heist: Who Stole the American Dream?, The Crew, Reindeer Games.
- The app's binary is 1 MB larger.
- Regenerating the artefact means regenerating its key file in the same change; the test
  `the_published_keys_name_the_published_artefact` pins the count.
