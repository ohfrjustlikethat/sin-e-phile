# Phase 3 — Data Layer and Portable Storage

**Status:** complete · **Depends on:** 1 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The database schema everything else depends on, designed once, correctly.

## Deliverables

SQLite via `sqlx` with WAL mode, in portable `./data/` by default with an installed-mode option. A migration system with forward and backward migrations, tested. Core schema: `media_items` (the generic type from §6.2), `external_ids`, `titles` (multi-language, romaji/native/english variants), `people`, `credits`, `genres`, `keywords`, `series`, `seasons`, `episodes` (with both seasonal and absolute numbering columns and a reconciliation table), `collections`, `profiles`, `watch_events`, `playback_positions`, `watchlist_items`, `local_files`, `local_file_matches`, `sources_config`, `settings`. Repository-pattern access layer — no raw SQL outside `persistence/`. A generic `media_kind` discriminator with `film | episode | series | anime_film | anime_series | live_channel | manga_chapter | comic_issue` so Phases 24–25 need no migration. Backup-on-migrate, and an export/import of the whole profile as a portable archive.

## Exit criteria

- [x] **E1** All migrations run forward and backward cleanly against a populated database.
- [x] **E2** Schema documented with an entity-relationship diagram in `docs/ARCHITECTURE.md`.
- [x] **E3** A database populated with 500,000 synthetic media items answers indexed lookups in under 100 ms. (The 100 ms budget is the lookup alone; bulk insertion of the 500,000 rows has no time budget and is measured and recorded separately in `docs/PERFORMANCE.md`.)
- [x] **E4** Copying the app folder to another location and launching it preserves all data.
- [x] **E5** An ADR records why the schema is generic over media kind.

## Subtasks

- [x] **3.1** crates/persistence: sqlx + SQLite with WAL, pinned versions; runtime-checked queries with tests/repository_surface.rs as the compensating control (ADR-0026); src-tauri/src/persistence/ re-exports only, guard-enforced (ADR-0022)
- [x] **3.2** Path resolution: portable ./data/ next to the executable by default, %APPDATA% installed-mode as an opt-in; override and detection
- [x] **3.3** Migration system: forward and backward migrations, with backup-on-migrate
- [x] **3.4** Schema - identity: media_items with the eight-value media_kind discriminator, external_ids, titles (romaji/native/english variants)
- [x] **3.5** Schema - people and taxonomy: people, credits, genres, keywords
- [x] **3.6** Schema - series: series, seasons, episodes with BOTH seasonal and absolute numbering, plus the reconciliation table (SPEC.md 6.2 - designed now, not patched in Phase 12)
- [x] **3.7** Schema - user and config: profiles, watch_events, playback_positions, watchlist_items, collections, local_files, local_file_matches, sources_config, settings
- [x] **3.8** Repository-pattern access layer over the whole schema
- [x] **3.9** Export/import of a whole profile as a portable archive
- [x] **3.10** Migration round-trip integration tests: forward and backward against a populated database (E1)
- [x] **3.11** 500,000 synthetic media items; indexed lookup under 100 ms, insertion timed separately (amendment 15); numbers into docs/PERFORMANCE.md and docs/eval-results.md
- [x] **3.12** ER diagram in docs/ARCHITECTURE.md (E2), and an ADR recording why the schema is generic over media kind (E5)

## Learning note

Relational schema design; why `sqlx`'s compile-time checking matters; what WAL mode is; migrations; the repository pattern; why designing for manga now costs nothing and retrofitting later costs weeks.

---

<!-- subtask log: appended during the phase -->

## Work log
- **3.1** crates/persistence: sqlx + SQLite with WAL, pinned versions, committed .sqlx/ offline metadata s · `593fcc8`
- **3.2** Path resolution: portable ./data/ next to the executable by default, %APPDATA% installed-mode as · `593fcc8`
- **3.3** Migration system: forward and backward migrations, with backup-on-migrate · `593fcc8`
- **3.4** Schema - identity: media_items with the eight-value media_kind discriminator, external_ids, titl · `593fcc8`
- **3.5** Schema - people and taxonomy: people, credits, genres, keywords · `593fcc8`
- **3.6** Schema - series: series, seasons, episodes with BOTH seasonal and absolute numbering, plus the r · `593fcc8`
- **3.7** Schema - user and config: profiles, watch_events, playback_positions, watchlist_items, collectio · `593fcc8`
- **3.8** Repository-pattern access layer over the whole schema · `593fcc8`
- **3.9** Export/import of a whole profile as a portable archive · `593fcc8`
- **3.10** Migration round-trip integration tests: forward and backward against a populated database (E1) · `593fcc8`
- **3.11** 500,000 synthetic media items; indexed lookup under 100 ms, insertion timed separately (amendmen · `593fcc8`
- **3.12** ER diagram in docs/ARCHITECTURE.md (E2), and an ADR recording why the schema is generic over med · `593fcc8`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `65f5147`.

### Evidence per criterion

**E1** — All migrations run forward and backward cleanly against a populated database.

> crates/persistence/tests/migrations.rs - 9 tests. `migrations_roll_back_against_a_populated_database` populates a FILE database across all four migrations then rolls back one at a time to 0 and asserts no tables survive; `the_ladder_can_be_climbed_twice` re-applies afterwards, which is what catches a down script that drops a table but forgets its indexes. Command: cargo test -p sinephile-persistence.

**E2** — Schema documented with an entity-relationship diagram in `docs/ARCHITECTURE.md`.

> docs/ARCHITECTURE.md - mermaid ER diagram covering all 19 tables plus episode_numbering, with the four design decisions and the load-bearing PRAGMA configuration.

**E3** — A database populated with 500,000 synthetic media items answers indexed lookups in under 100 ms. (The 100 ms budget is the lookup alone; bulk insertion of the 500,000 rows has no time budget and is measured and recorded separately in `docs/PERFORMANCE.md`.)

> cargo test -p sinephile-persistence --release --test benchmark -- --ignored --nocapture, 500,000 synthetic rows: by_id p99 0.098ms, by_exact_title p99 0.179ms, by_external_id p99 0.120ms against a 100ms budget. Bulk insert 43.7s (11,440 rows/sec), database 145.4 MB, timed separately per amendment 15. Recorded in docs/eval-results.md and docs/PERFORMANCE.md. The benchmark found idx_titles_text silently full-scanning (26.679ms -> 0.081ms p50, 330x) because its collation did not match the query's.

**E4** — Copying the app folder to another location and launching it preserves all data.

> crates/persistence/tests/portability.rs - 5 tests. `a_copied_data_folder_keeps_everything` writes a populated installation, checkpoints the WAL, copies the whole data folder to another path with a directory-tree copy, opens it there and asserts catalogue, watch history, resume position and settings all survive; `nothing_in_the_database_records_where_it_lives` introspects the schema and fails if any new path-shaped column appears. Found a real bug: open_in probed writability before creating the directory, which failed on every fresh portable install.

**E5** — An ADR records why the schema is generic over media kind.

> docs/adr/0025-generic-media-schema.md, verified by tests/migrations.rs::all_eight_media_kinds_are_accepted.

### Debt incurred

- **D11** The data layer uses runtime-checked sqlx::query() rather than the compile-time-checked query! macros. SPEC.md's tech table gives compile-time checking as the REASON sqlx was chosen ('valuable for a learner'), so this is a deviation the author should rule on. Cost of converting: query! needs literal SQL, so archive.rs's dynamically-built preference query cannot use it at all; SQLite nullability inference is weak, so most columns need `as "col!"` annotations; and it adds sqlx-cli plus a `cargo sqlx prepare` step after every schema change. Benefit: SQL typos fail the build instead of a test. Raised at the end of Phase 3, not decided.

### Next phase starts by

Phase 3 is complete with all five exit criteria evidenced. Verify CI green on phase/03-data-layer, merge, tag phase-03. THEN: the author owes a ruling on P9 (sqlx compile-time query! macros vs runtime-checked queries) BEFORE Phase 4 starts, because Phase 4's ingestion pipeline writes far more SQL than Phase 3 did and converting later costs more.
