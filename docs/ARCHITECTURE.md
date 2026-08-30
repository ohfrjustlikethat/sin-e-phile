# Architecture

Module boundaries, data flow, and the entity-relationship diagram.

> **Status: Phase 0.** The architecture is specified in `SPEC.md` §6 but not yet
> built. This document fills in as each subsystem lands, and is updated whenever the
> shape of the system changes (`SPEC.md` §11.1).
>
> For the plain-English version, read [`HOW_IT_WORKS.md`](HOW_IT_WORKS.md) first.
> This file is the precise one.

## Filled in by

| Section | Phase |
|---|---|
| Process model and the IPC boundary | 1 |
| Entity-relationship diagram for the schema | 3 |
| Catalogue ingestion pipeline | 4 |
| Search: FTS5 + HNSW + fusion | 5 |
| The `SourceBackend` trait and the resolver | 6 |
| Torrent scheduler and the local HTTP server | 7 |
| Playback pipeline | 8 |
| Taste, recommendation and discovery | 15–17 |

## Load-bearing invariants

These hold from the phase that introduces them and must not be broken silently.

- **Business logic never lives in `src-tauri/src/commands/`.** That directory is the
  IPC surface: thin, no logic.
- **Raw SQL never appears outside `src-tauri/src/persistence/`.** Everything else
  goes through the repository layer.
- **All source access goes through the `SourceBackend` trait** (§6.1). A local file,
  a torrent, a debrid link and an HTTP stream are all `SourceCandidate`s, ranked by
  one scoring function. Changing this trait requires an ADR.
- **Tier gating goes through `tiers.rs` only.** No feature checks hardware directly.
- **One canonical `MediaItem`** (§6.2), generic over `media_kind`, so Phases 24–25
  need no migration.
