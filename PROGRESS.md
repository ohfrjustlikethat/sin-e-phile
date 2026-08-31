# Progress

> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by
> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two
> can never disagree). Edit the state file, then regenerate.

**Spec version 1.4.0** · 5 session(s) completed · last updated 2026-08-31

---

## Where we are right now

**Phase 3 — Data Layer and Portable Storage** (`complete`, branch `phase/03-data-layer`)

5 of 5 exit criteria met with evidence.

> The schema everything else depends on, designed once. Two constraints carry over: ADR-0022 puts all SQL in crates/persistence with src-tauri/src/persistence/ as re-exports only (guard-enforced), and SPEC.md 6.2 requires anime's absolute-vs-seasonal numbering and title variants designed NOW rather than patched in Phase 12. media_kind carries all eight values from the start so Phases 24-25 need no migration.

### Subtasks — 12/12 complete

- [x] **3.1** crates/persistence: sqlx + SQLite with WAL, pinned versions, committed .sqlx/ offline metadata so CI compiles without a live database; src-tauri/src/persistence/ re-exports only (guard-enforced, ADR-0022) · `593fcc8`
- [x] **3.2** Path resolution: portable ./data/ next to the executable by default, %APPDATA% installed-mode as an opt-in; override and detection · `593fcc8`
- [x] **3.3** Migration system: forward and backward migrations, with backup-on-migrate · `593fcc8`
- [x] **3.4** Schema - identity: media_items with the eight-value media_kind discriminator, external_ids, titles (romaji/native/english variants) · `593fcc8`
- [x] **3.5** Schema - people and taxonomy: people, credits, genres, keywords · `593fcc8`
- [x] **3.6** Schema - series: series, seasons, episodes with BOTH seasonal and absolute numbering, plus the reconciliation table (SPEC.md 6.2 - designed now, not patched in Phase 12) · `593fcc8`
- [x] **3.7** Schema - user and config: profiles, watch_events, playback_positions, watchlist_items, collections, local_files, local_file_matches, sources_config, settings · `593fcc8`
- [x] **3.8** Repository-pattern access layer over the whole schema · `593fcc8`
- [x] **3.9** Export/import of a whole profile as a portable archive · `593fcc8`
- [x] **3.10** Migration round-trip integration tests: forward and backward against a populated database (E1) · `593fcc8`
- [x] **3.11** 500,000 synthetic media items; indexed lookup under 100 ms, insertion timed separately (amendment 15); numbers into docs/PERFORMANCE.md and docs/eval-results.md · `593fcc8`
- [x] **3.12** ER diagram in docs/ARCHITECTURE.md (E2), and an ADR recording why the schema is generic over media kind (E5) · `593fcc8`

### Exit criteria

- [x] **E1** All migrations run forward and backward cleanly against a populated database.
      - *Evidence:* crates/persistence/tests/migrations.rs - 9 tests. `migrations_roll_back_against_a_populated_database` populates a FILE database across all four migrations then rolls back one at a time to 0 and asserts no tables survive; `the_ladder_can_be_climbed_twice` re-applies afterwards, which is what catches a down script that drops a table but forgets its indexes. Command: cargo test -p sinephile-persistence.
- [x] **E2** Schema documented with an entity-relationship diagram in `docs/ARCHITECTURE.md`.
      - *Evidence:* docs/ARCHITECTURE.md - mermaid ER diagram covering all 19 tables plus episode_numbering, with the four design decisions and the load-bearing PRAGMA configuration.
- [x] **E3** A database populated with 500,000 synthetic media items answers indexed lookups in under 100 ms. (The 100 ms budget is the lookup alone; bulk insertion of the 500,000 rows has no time budget and is measured and recorded separately in `docs/PERFORMANCE.md`.)
      - *Evidence:* cargo test -p sinephile-persistence --release --test benchmark -- --ignored --nocapture, 500,000 synthetic rows: by_id p99 0.098ms, by_exact_title p99 0.179ms, by_external_id p99 0.120ms against a 100ms budget. Bulk insert 43.7s (11,440 rows/sec), database 145.4 MB, timed separately per amendment 15. Recorded in docs/eval-results.md and docs/PERFORMANCE.md. The benchmark found idx_titles_text silently full-scanning (26.679ms -> 0.081ms p50, 330x) because its collation did not match the query's.
- [x] **E4** Copying the app folder to another location and launching it preserves all data.
      - *Evidence:* crates/persistence/tests/portability.rs - 5 tests. `a_copied_data_folder_keeps_everything` writes a populated installation, checkpoints the WAL, copies the whole data folder to another path with a directory-tree copy, opens it there and asserts catalogue, watch history, resume position and settings all survive; `nothing_in_the_database_records_where_it_lives` introspects the schema and fails if any new path-shaped column appears. Found a real bug: open_in probed writability before creating the directory, which failed on every fresh portable install.
- [x] **E5** An ADR records why the schema is generic over media kind.
      - *Evidence:* docs/adr/0025-generic-media-schema.md, verified by tests/migrations.rs::all_eight_media_kinds_are_accepted.

---

## What's next

Phase 3 is complete with all five exit criteria evidenced. Verify CI green on phase/03-data-layer, merge, tag phase-03. THEN: the author owes a ruling on P9 (sqlx compile-time query! macros vs runtime-checked queries) BEFORE Phase 4 starts, because Phase 4's ingestion pipeline writes far more SQL than Phase 3 did and converting later costs more.

---

## Blockers

None.

---

## All 28 phases

Tiers are the legitimate stopping points from `SPEC.md` Appendix E. **Tier B is the definition of done** — complete it and the project has succeeded.

| | # | Phase | Tier | Depends on | Sessions | Criteria met |
|---|---|---|---|---|---|---|
| [x] | 0 | Bootstrap and Project Infrastructure | A | nothing | 1 | 8/8 |
| [x] | 1 | Application Shell and Capability Tiers | A | 0 | 1–2 | 7/7 |
| [x] | 2 | Design System and Visual Language | A | 1 | 1–2 | 5/5 |
| [x] | 3 | Data Layer and Portable Storage | A | 1 | 1–2 | 5/5 |
| [ ] | 4 | Metadata Backbone | A | 3 | 2–3 | 0/7 |
| [ ] | 5 | Semantic Search Engine | A | 4 | 2 | 0/5 |
| [ ] | 6 | Source Resolver and Addon Protocol | A | 3 | 1–2 | 0/6 |
| [ ] | 7 | Torrent Engine and Streaming Server | A | 6 | 2–3 | 0/6 |
| [ ] | 8 | Player Core — MILESTONE: FIRST DEMOABLE BUILD 🏁 | A | 5, 7 | 2–3 | 0/6 |
| [ ] | 9 | Intelligent Source Selection | B | 8 | 1–2 | 0/6 |
| [ ] | 10 | Subtitle Pipeline | B | 8 | 2 | 0/6 |
| [ ] | 11 | Player Experience Layer | B | 8, 9, 10 | 2 | 0/5 |
| [ ] | 12 | Local Library Engine | B | 4 | 2–3 | 0/6 |
| [ ] | 13 | Download Manager and the Stream-vs-Download Advisor | B | 7, 9, 12 | 1–2 | 0/5 |
| [ ] | 14 | Profiles and First-Run Onboarding | B | 3, 5 | 2 | 0/6 |
| [ ] | 15 | Taste Model | B | 5, 14 | 2 | 0/5 |
| [ ] | 16 | Recommendation Engine | B | 15 | 2–3 | 0/6 |
| [ ] | 17 | Discovery Engine | B | 16 | 2 | 0/6 |
| [ ] | 18 | Browsing Surfaces 🏁 | B | 17 | 2–3 | 0/5 |
| [ ] | 19 | Watchlist and External Sync | C | 18 | 1–2 | 0/5 |
| [ ] | 20 | Windows Platform Integration | C | 12, 18 | 2–3 | 0/5 |
| [ ] | 21 | Performance Engineering and the Low-End Path | B | 18 (20 optional) | 2 | 0/4 |
| [ ] | 22 | Vision Layer (Tier 2) | D | 11 | 1–2 | 0/5 |
| [ ] | 23 | Binge Intelligence | C | 11, 12 | 1–2 | 0/4 |
| [ ] | 24 | Live Channels | D | 18 | 2 | 0/5 |
| [ ] | 25 | Manga and Comics | D | 3, 5, 16 | 2–3 | 0/5 |
| [ ] | 26 | Connected Playback | D | 11 | 2 | 0/5 |
| [ ] | 27 | Hardening, Packaging, and Portfolio Finalisation | B | everything | 2–3 | 0/5 |

Legend: `[x]` complete · `[~]` in progress · `[!]` blocked · `[?]` awaiting review · `[ ]` not started. 🏁 marks Phase 8 (first demoable build) and Phase 18 (complete product).

---

## Known debt

- **D1** (raised in Phase 0) The hashed denylist matches exact tokens only — no substring, fuzzy, or homoglyph matching. Accepted in ADR-0009; the structural matcher covers the shapes that carry real risk. Revisit only if a near-miss is ever observed.
- **D2** (raised in Phase 0) A fresh clone is unprotected by the git hooks until tools/doctor runs once, because core.hooksPath is per-clone config. CI is the backstop. Accepted in ADR-0012.
- **D3** (raised in Phase 0) Bare-domain detection is disabled inside source files (attribute access is shaped identically). URLs are still checked everywhere, as is the denylist. Documented in tools/guard/README.md.
- **D4** (raised in Phase 0) tools/state/validate_state.py implements a subset of JSON Schema draft 2020-12 by hand, because ADR-0012 fixed these tools as stdlib-only. It rejects any schema construct it does not implement rather than passing silently, but it is not a conformant validator. Revisit only if the schema needs constructs it lacks.
- **D5** (raised in Phase 1) Player has TWO compositing paths - still-frame when paused, region cutouts when playing - and the chrome silhouette must stay in sync with the region or the uncovered part of the hole shows the desktop. Phase 11 owns this invariant. Retire if wry PR #1762 merges and DirectComposition replaces both.
- **D6** (raised in Phase 1) librqbit has no webseed (BEP-19) support, so Internet Archive torrents will not work through the torrent path. Phase 6's InternetArchiveBackend must resolve to direct HTTP instead. Not a defect, but it constrains how that backend is built.
- **D7** (raised in Phase 1) Integration tests that need a running Tauri app cannot run under cargo test on Windows (ADR-0022): a test binary linking Tauri fails to launch with STATUS_ENTRYPOINT_NOT_FOUND, and the targeted fix is nightly-only. Unit tests are unaffected because logic lives in crates/. Such tests belong in the SPEC.md 12.3 manual plan, or a WebDriver harness later.
- **D8** (raised in Phase 1) tiers.rs treats any non-software DXGI adapter as having hardware decode, and uses >= 2 GB dedicated VRAM as a proxy for 'discrete GPU or strong iGPU'. Both are coarse. Whether a SPECIFIC codec decodes in hardware is only knowable at play time from mpv's hwdec-current, so Phase 8 should feed that back and Phase 21 should revisit the VRAM threshold against real Tier 0/1 hardware.
- **D9** (raised in Phase 2) Rail's roving tabindex sets tabIndex imperatively on the first focusable descendant of each mounted item rather than threading it through the render prop. Correct today because `render` returns caller-owned markup, but it silently does nothing if a card's first focusable element is not its main control. Revisit in Phase 9, when real screens use Rail with more complex cards.
- **D10** (raised in Phase 2) The UI audit (tools/uiaudit) drives the design gallery, not real product screens - those do not exist until Phase 9. Its budgets prove the Rail component holds 60fps, not that any real screen does. Revisit in Phase 9: point the audit at the Home screen too.
- **D11** (raised in Phase 3) The data layer uses runtime-checked sqlx::query() rather than the compile-time-checked query! macros. SPEC.md's tech table gives compile-time checking as the REASON sqlx was chosen ('valuable for a learner'), so this is a deviation the author should rule on. Cost of converting: query! needs literal SQL, so archive.rs's dynamically-built preference query cannot use it at all; SQLite nullability inference is weak, so most columns need `as "col!"` annotations; and it adds sqlx-cli plus a `cargo sqlx prepare` step after every schema change. Benefit: SQL typos fail the build instead of a test. Raised at the end of Phase 3, not decided.

---

## Decisions pending

- **P1** — decide by Phase 27 Source-only distribution versus Phase 27 packaging and the 2.3 installed-size budget. See docs/DECISIONS_PENDING.md.
- **P5** — decide by Phase 12 Where the Phase 12 review-queue confidence threshold sits, given >95% top-1 and <1% false-confident pull against each other. Measure, do not guess. See docs/DECISIONS_PENDING.md.
- **P6** — decide by Phase 27 Windows Sandbox pass on a genuinely bare machine. The E1 CI job proves a clean checkout builds, but windows-latest ships Rust, Node and MSVC preinstalled, so it does not prove SETUP.md is complete from nothing.
- **P8** — decide by Phase 21 Spike C measured query-embedding latency on the Tier 2 dev machine only. ADR-0015 also asked for a constrained VM approximating Tier 0. Unpadded headroom is ~18x so this does not block Phase 5, but the padded worst case with a 3-4x Tier 0 penalty lands at 24-33 ms, close to the 30 ms trigger. Measure before Phase 21 signs off the 80 ms search budget.
- **P9** — decide by Phase 4 Convert the data layer to sqlx's compile-time-checked query! macros, or accept runtime-checked queries? SPEC.md 2 names compile-time checking as the REASON sqlx was chosen. Phase 3 shipped runtime-checked; see known_debt for the cost on both sides. See docs/DECISIONS_PENDING.md.
