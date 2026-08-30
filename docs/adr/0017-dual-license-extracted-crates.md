# 0017 — Dual-license the extracted crates; keep FFmpeg out of subtitle-align

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 0 (binds Phases 6, 10, 12)
- **Resolves:** P4 · **Relates to:** ADR-0007

## Context

- `SPEC.md` §7 extracts `filename-parser`, `subtitle-align` and `source-protocol`
  precisely because they are self-contained and **genuinely reusable**.
- ADR-0007 licensed the repository GPL-3.0, which is required by libmpv and FFmpeg
  linkage in the *application*.
- The Rust ecosystem is overwhelmingly MIT/Apache-2.0, and compatibility runs one
  way only. So the three crates chosen for reuse were, as licensed, unusable by
  almost everyone who would want them — defeating the point of extracting them.
- **None of the three links libmpv or FFmpeg.** The parser is string handling;
  `source-protocol` is types and a schema. `subtitle-align` was the only one at
  risk, because a VAD signal has to come from somewhere.

## Decision

- The three extracted crates are **MIT OR Apache-2.0** (the Rust convention).
- The application remains **GPL-3.0**, from libmpv and FFmpeg linkage.
- **`subtitle-align` must not depend on FFmpeg.** It takes **PCM samples or a
  precomputed VAD signal** as input. FFmpeg extraction lives in the application,
  which is GPL anyway.

## Consequences

- The crates are publishable and genuinely usable — three real crates is a stronger
  portfolio claim than three directories.
- **The `subtitle-align` constraint is an architectural improvement, not a tax.**
  A crate that takes a signal is testable with synthetic fixtures and needs no
  subprocess, no temp files, and no FFmpeg on the test machine. Phase 10's eval
  harness gets faster and more deterministic.
- Cost: an audio-extraction boundary must be designed in Phase 10 — where the app
  decodes and the crate analyses. Doing that deliberately is better than discovering
  it later.
- The MIT/Apache side must contain no GPL-derived code. Since the crates are written
  from scratch here, this holds by construction, but any dependency added to them
  needs a licence check.

## Alternatives Considered

- **Leave everything GPL-3.0.** Simplest; makes the reuse claim rhetorical. Rejected.
- **LGPL the crates.** Rejected: wrong idiom for Rust, where static linking is the
  norm and the ecosystem expects MIT/Apache.
- **Separate repositories from the start.** Rejected for now — dual-licensing inside
  this repo keeps development in one place, and publishing separately later is a
  `cargo publish`, not a migration.
- **Let `subtitle-align` shell out to FFmpeg and dual-license anyway.** Arguably fine
  (invoking a binary is not linking), but it makes the licence question arguable
  rather than settled, and forces every test to spawn a process. Rejected on both
  counts.
