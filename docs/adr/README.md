# Architecture Decision Records

Every non-obvious decision in this project gets a record here, written **at the
moment of deciding** (`SPEC.md` §11.2). An ADR is required for: choosing between
libraries, designing a public interface, choosing an algorithm where alternatives
exist, any performance trade-off, and any deviation from `SPEC.md`.

Format: Context / Decision / Consequences / Alternatives Considered. See
[0001](0001-record-architecture-decisions.md).

## Index

| # | Title | Status | Date |
|---|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted | 2026-08-30 |
| 0002 | *(reserved)* Tauri v2 over Electron | Pending — Phase 0b | |
| 0003 | *(reserved)* librqbit over libtorrent-rasterbar | Pending — Phase 0b | |
| 0004 | *(reserved)* libmpv over libVLC | Pending — Phase 0b | |
| 0005 | *(reserved)* SQLite + FTS5 + HNSW over a vector database | Pending — Phase 0b | |
| 0006 | *(reserved)* The ships-empty source posture | Pending — Phase 0b | |
| 0007 | *(reserved)* GPL-3.0 and the dependency licence audit | Pending — Phase 0b | |
| 0008 | *(reserved)* Portable-by-default storage | Pending — Phase 0b | |
| [0009](0009-posture-guard-denylist-design.md) | Posture guard denylist design | Accepted | 2026-08-30 |
| [0010](0010-source-allowlist-and-governance.md) | Source allowlist and its governance | Accepted | 2026-08-30 |
| [0011](0011-fixture-site-tag-redaction.md) | Fixture site-tag redaction policy | Accepted | 2026-08-30 |
| [0012](0012-dev-tooling-language-and-hooks.md) | Dev tooling in Python; hooks via core.hooksPath | Accepted | 2026-08-30 |
| [0013](0013-tmdb-optional-offline-first-catalogue.md) | TMDB is optional; the catalogue is offline-first | Accepted | 2026-08-30 |
| [0014](0014-embedding-artefact-distribution.md) | Embedding artefacts ship via GitHub Releases | Accepted | 2026-08-30 |
| [0015](0015-tier-0-query-embedding.md) | Tier 0 embeds queries but never documents | Accepted | 2026-08-30 |

**Numbers 0002–0008 are reserved, not skipped.** They record decisions already
locked in `SPEC.md` §5 and are written in session 0b. Reserving them keeps the
seed decisions in a contiguous block at the front of the index, where someone
reading the repository for the first time will look for them.
