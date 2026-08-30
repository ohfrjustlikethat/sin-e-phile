# Architecture Decision Records

Every non-obvious decision in this project gets a record here, written **at the
moment of deciding** (`SPEC.md` §11.2). An ADR is required for: choosing between
libraries, designing a public interface, choosing an algorithm where alternatives
exist, any performance trade-off, and any deviation from `SPEC.md`.

Format: Context / Decision / Consequences / Alternatives Considered. See
[0001](0001-record-architecture-decisions.md).

**Terse from ADR-0016 onward** — bullets, roughly 15 lines. ADRs are mandatory and
are not what gets cut; length is. The Alternatives section is the one that must
never be empty: if nothing was genuinely rejected, there was no decision.

## Index

| # | Title | Status | Date |
|---|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted | 2026-08-30 |
| [0002](0002-tauri-v2-over-electron.md) | Tauri v2 over Electron | Accepted | 2026-08-30 |
| [0003](0003-librqbit-over-libtorrent.md) | librqbit over libtorrent-rasterbar | Accepted | 2026-08-30 |
| [0004](0004-libmpv-over-libvlc.md) | libmpv over libVLC | Accepted | 2026-08-30 |
| [0005](0005-sqlite-fts5-hnsw-over-vector-database.md) | SQLite + FTS5 + HNSW over a vector database | Accepted | 2026-08-31 |
| [0006](0006-ships-empty-source-posture.md) | The ships-empty source posture | Accepted | 2026-08-31 |
| [0007](0007-gpl-3-and-dependency-licence-audit.md) | GPL-3.0, and the dependency licence audit | Accepted | 2026-08-31 |
| [0008](0008-portable-by-default-storage.md) | Portable-by-default storage | Accepted | 2026-08-31 |
| [0009](0009-posture-guard-denylist-design.md) | Posture guard denylist design | Accepted | 2026-08-30 |
| [0010](0010-source-allowlist-and-governance.md) | Source allowlist and its governance | Accepted | 2026-08-30 |
| [0011](0011-fixture-site-tag-redaction.md) | Fixture site-tag redaction policy | Accepted | 2026-08-30 |
| [0012](0012-dev-tooling-language-and-hooks.md) | Dev tooling in Python; hooks via core.hooksPath | Accepted | 2026-08-30 |
| [0013](0013-tmdb-optional-offline-first-catalogue.md) | TMDB is optional; the catalogue is offline-first | Accepted | 2026-08-30 |
| [0014](0014-embedding-artefact-distribution.md) | Embedding artefacts ship via GitHub Releases | Accepted | 2026-08-30 |
| [0015](0015-tier-0-query-embedding.md) | Tier 0 embeds queries but never documents | Accepted | 2026-08-30 |
| [0016](0016-lean-documentation-and-session-profile.md) | Lean documentation and session profile | Accepted | 2026-08-31 |
| [0017](0017-dual-license-extracted-crates.md) | Dual-license the extracted crates; no FFmpeg in subtitle-align | Accepted | 2026-08-31 |
| [0018](0018-tmdb-embedding-text-and-swappable-source.md) | TMDB text in embeddings is inference; text source is swappable | Accepted | 2026-08-31 |
| [0019](0019-movielens-matrix-computed-on-device.md) | Compute the MovieLens item-item matrix on the user's machine | Accepted | 2026-08-31 |

**0002–0008 record decisions that were already locked in `SPEC.md` §5 before this
repository existed.** They were written in session 0b, after 0009–0015, which is why
their numbers are lower than their dates. The numbers were reserved rather than
allocated on a first-come basis, so that the seed decisions sit in a contiguous
block at the front of the index — which is where someone reading the repository for
the first time will look for them.

**0009–0015 came out of the Phase 0 specification audit** (see `SESSION_LOG.md`
entry 1). Seven contradictions or gaps in `SPEC.md` needed new design before any
code could be written; each was ruled on by the author, recorded here, and only then
amended into the spec, per §2.8's amend-first rule.
