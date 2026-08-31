# Phase 3 — Learning notes

**Data layer and portable storage.** Complete.

Four sections per ADR-0016 (A4): what we built, why, new concepts as
concept + `file:line`, and the five questions. No prose tour.

---

## 1. What we built

- **`crates/persistence`** — the entire data layer, outside the Tauri crate so its
  migrations can actually be tested (ADR-0022). `tools/guard` now fails the build if
  SQL appears anywhere under `src-tauri/`.
- **Four reversible migrations** — identity, taxonomy, series, user data. 19 tables
  plus `episode_numbering` and `media_language_tracks`.
- **Repository layer** — `MediaRepository`, `EpisodeRepository`, `ProfileRepository`.
  No SQL exists anywhere else.
- **Portable archive** — profile export/import keyed on external ids.
- **31 tests**, including the 500,000-row benchmark.

## 2. Why

- **One `media_items` table, not one per kind** (ADR-0025). SQLite cannot alter a
  `CHECK` constraint — changing one means rebuilding the table and every index — so
  the two kinds Phase 24 needs are in the constraint today, at zero cost.
- **`episode_numbering` stores what each source said**, because the conversions are
  not arithmetic and Phase 12's false-confident budget is 1%.
- **`watch_events` is an append-only log**, because Phase 15 needs "watched three
  times in 2019, never since" and a boolean discards that permanently.
- **The archive keys on external ids**, because internal ids are ingestion-order and
  differ between installations.

## 3. New concepts

| Concept | Where |
|---|---|
| WAL mode, and why it suits this app | `crates/persistence/src/db.rs:62` |
| `synchronous = NORMAL` and what it risks | `crates/persistence/src/db.rs:70` |
| `foreign_keys` is per-connection and OFF by default | `crates/persistence/src/db.rs:79` |
| Reversible migrations, up/down pairs | `crates/persistence/migrations/0001_identity.down.sql:1` |
| Backup-on-migrate, and `wal_checkpoint(TRUNCATE)` | `crates/persistence/src/db.rs:171` |
| Migrations embedded in the binary, not loose files | `crates/persistence/src/db.rs:45` |
| The repository pattern | `crates/persistence/src/repositories/mod.rs:1` |
| Extension tables vs nullable columns | `crates/persistence/migrations/0003_series.up.sql:28` |
| Discriminator + `CHECK` constraint | `crates/persistence/migrations/0001_identity.up.sql:50` |
| Index collation must match the query's | `crates/persistence/migrations/0001_identity.up.sql:110` |
| Expressions are legal in an index, illegal in a `PRIMARY KEY` | `crates/persistence/migrations/0002_people_taxonomy.up.sql:26` |
| Correlated subquery + aggregate: the alias trap | `crates/persistence/src/archive.rs:131` |
| Partial unique index (`WHERE is_default = 1`) | `crates/persistence/migrations/0004_user_and_config.up.sql:22` |
| One transaction per batch, and why commits dominate | `crates/persistence/src/repositories/media.rs:216` |
| `:memory:` gives each connection its OWN database | `crates/persistence/src/db.rs:106` |
| Deterministic fixtures (LCG) for reproducible benchmarks | `crates/persistence/tests/benchmark.rs:34` |
| p50/p95/p99 and why the criterion uses p99 | `crates/persistence/tests/benchmark.rs:225` |
| Dev-mode path resolution vs `cargo clean` | `crates/persistence/src/paths.rs:33` |

### The four bugs, and what each one teaches

1. **`idx_titles_text` was silently full-scanning.** SQLite only uses an index whose
   collation matches the comparison's, and the query said `COLLATE NOCASE` while the
   index was `BINARY`. 26.679 ms → 0.081 ms, a 330× difference. *It passed the exit
   criterion either way* — the benchmark is the only reason it was found.
   `crates/persistence/migrations/0001_identity.up.sql:110`

2. **`PRIMARY KEY (…, COALESCE(character, ''))` was rejected outright.** Expressions
   are allowed in an index but not in a primary key. `character` became
   `NOT NULL DEFAULT ''`, which is better anyway.
   `crates/persistence/migrations/0002_people_taxonomy.up.sql:26`

3. **`MIN(CASE e.source …)` inside a correlated subquery** read the *outer* row and
   was rejected as "misuse of aggregate". The ordering expression now takes its
   table alias. `crates/persistence/src/archive.rs:135`

4. **`open_in` probed writability before creating the directory**, so a fresh
   portable install reported "not writable" for a directory that was merely absent.
   Found by the E4 portability test, not by running the app.
   `crates/persistence/src/db.rs:94`

And a fifth, in the tooling: **the architecture guard rejected the file it exists to
protect.** It checked the re-export rule line by line, so a rustfmt-wrapped
`pub use foo::{…}` spanning four lines failed on three of them. Now statement-based,
with the wrapped form as a permanent regression vector.
`tools/guard/guard.py:551`

## 4. Self-check questions

Written, not asked — the gate fires at the end of Phase 8 (ADR-0016 A2).

1. **WAL mode.** What problem does it solve that the default journal does not, and
   why does *this* app in particular need it? What does `synchronous = NORMAL` give
   up, and why is that an acceptable trade here but not for a bank?

2. **Reversible migrations.** Why is rolling back an *empty* database nearly
   worthless as a test? Name the two specific failures that only appear when data is
   present, and say which test catches each.

3. **The numbering problem.** Explain to someone who does not watch anime why a
   long-running series has no single correct episode number, and why the mapping
   between AniList's "59" and TVDB's "S03E07" cannot be computed. What would break
   in Phase 12 if we stored one canonical number and converted?

4. **The archive keys on external ids, never internal ones.** What exactly goes
   wrong if it keys on internal ids — and why would the bug be invisible rather than
   loud? What does the test do to prove it?

5. **The index that did nothing.** `by_exact_title` was 400× slower than the other
   two lookups and still passed the exit criterion. Explain the cause, and then
   answer the harder question: what does this say about exit criteria as a way of
   knowing whether something works?
