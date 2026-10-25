# Phase 4 — Learning notes

**Metadata backbone.** 11 of 13 subtasks. Four sections per ADR-0016 (A4): what we
built, why, new concepts as concept + `file:line`, and the five questions. No code tour.

---

## 1. What we built

- **`tools/ingest`** — the offline pipeline. A resumable job runner (`job.rs`), IMDb
  loaders (`load.rs`, `credits.rs`, `akas.rs`, `episodes_load.rs`), normalised titles
  (`normalise.rs`), AniList matching and ingestion (`matching.rs`, `anime.rs`), the
  incremental refresh (`refresh.rs`), the MovieLens join (`movielens.rs`), a one-off
  data repair (`repair.rs`), the E5 checker (`verify.rs`) and the embedding producer
  (`embed.rs`).
- **`crates/metadata-api`** — the HTTP boundary: token-bucket rate limiting, backoff
  with full jitter, three-state cache freshness, a `Transport` trait with a fake.
- **`crates/artwork`** — blurhash (written out, not a dependency), lossy WebP
  re-encoding, a disk cache with an LRU budget.
- **`crates/embedding`** — the document builder, int8 quantisation, and the checksummed
  artefact format, shared by producer and application.
- **`crates/persistence`** grew per-profile credentials (DPAPI-wrapped) and catalogue
  readiness.
- **Ten migrations.** The catalogue: 2,702,737 titles, 855,703 core, 10,079,841
  credits, 6,176,950 title rows, 539,817 episodes, 7,391 anime mapped to AniList.

## 2. Why

- **A wrong match is worse than no match.** A missed anime shows an IMDb entry with no
  Japanese title. A wrong one attaches *Fullmetal Alchemist*'s episodes to
  *Brotherhood*, and nothing downstream ever notices. So the matcher refuses ties, and
  every refusal is counted by reason.
- **Scope is chosen from measurements, not predictions.** Four storage predictions in
  this phase were wrong in both directions. Every scope decision since has been made by
  loading a sample and weighing it — episodes at 410 bytes each, WebP at 42.1%, the
  artefact at 313 MB.
- **A partial catalogue must not present itself as a complete one.** "No results" means
  something different at 3% ingested than at 100%.

## 3. New concepts

### Rust

- **`Arc<T>` vs `&T` under a higher-ranked bound** — `crates/metadata-api/src/anilist.rs`.
  `Job::run_step` takes `for<'t> FnMut(&'t mut SqliteTx, …) -> BatchFuture<'t>`.
  `for<'t>` means "for **every** lifetime", so a future returned from that closure may
  not borrow anything with a *particular* lifetime. The fix is not a cleverer
  annotation; it is to stop borrowing. Phase 11 needs the same shape for Tauri state.
- **Deferred transactions** — `anime.rs`. SQLite's `BEGIN` takes no lock until the
  first statement, which is what makes an HTTP fetch inside a transaction safe.
- **FFI and `unsafe`** — `crates/persistence/src/secrets.rs`. Three rules held to:
  every `unsafe` block carries a `SAFETY:` note naming why it is sound; memory Windows
  allocates is freed with `LocalFree` exactly once; and a failure returns `None` rather
  than a partially-initialised value.
- **`&str` is not indexable by byte** — `crates/artwork/src/blurhash.rs`,
  `crates/embedding/src/document.rs`. `&hash[2..6]` panics if a multi-byte character
  straddles a boundary. Both places take values from a database, so one corrupt row
  would have taken the process down.

### Data, and what it does to a design

- **Normalisation** — `matching.rs`. Lowercase, drop punctuation, collapse spaces;
  deliberately no transliteration.
- **Narrowing by strength of evidence** — `matching.rs`. When candidates tie, prefer a
  known agreeing year over no year, then the shape AniList states. Each stage may
  discard a worse-evidenced rival; none may create a match or discard the last one.
- **int8 quantisation** — `crates/embedding/src/quantise.rs`. Per-vector symmetric
  scaling. The property that matters is not accuracy but *order*: cosine moves by under
  0.005, so ranking is unchanged.
- **Blurhash** — `crates/artwork/src/blurhash.rs`. A DCT onto a 4×3 cosine basis, ~30
  characters. Must be computed in linear light, not sRGB.

## 4. The bugs, and what each one teaches

Every one of these is a real defect found in this phase, not a hypothetical.

1. **Counting rows instead of items.** Three spellings of *Death Note* were three
   rival candidates for one film, so the four most famous anime in the sample were all
   refused as ambiguous *against themselves* — 97 of 250. **Unit tests could not have
   caught this**: a fixture never has three spellings of one title. *Fixtures test the
   logic you thought of; real data tests the logic you didn't.*

2. **A stale comment outranking the schema.** An `ON CONFLICT` target copied from
   migration 0001's comment, which migration 0007 had superseded. *An applied migration
   is history, not documentation — ask the database what its indexes are.*

3. **`is_resuming` reporting a fresh run on every real resume.** It counted only
   *completed* steps; a crash lands mid-step. *A predicate that is true in the rare
   case and false in the common one is worse than no predicate.*

4. **IMDb's files sort as TEXT, not as numbers.** `"tt10001008" < "tt1000101"`, so
   `seek_past(our maximum)` walked into rows we already held. *Never assume a dataset's
   ordering. Checking it is four lines.*

5. **A release region overriding a known language.** 41,193 rows called French and
   Spanish titles "English", because the region was read before the language. *A
   release region is where a title was used, not what language it is in.*

6. **Preferring `accessed()` for cache eviction.** Windows does not update last-access
   times, so least-recently-*used* silently became least-recently-*written* — evicting
   the poster on the home screen and keeping the one scrolled past. *The comment
   explaining the hazard was already written; the code did the opposite.*

7. **A fixture that was wrong four times and the code zero.** The E5 hand-check failed
   4 of 62 on its first run, and every failure was mine — including an IMDb id that was
   *Regular Show* rather than *Nichijou*. *Recorded in the fixture header, because a
   fixture whose failures are quietly edited away until it passes is worth nothing.*

8. **A test that passed vacuously.** The cache budget was a guessed constant, three
   images fitted, eviction never ran, and the assertion proved nothing. It now measures
   two images before choosing a limit. *A green test that cannot fail is not evidence.*

## 5. Self-check questions

Asked when the phase closes, not before.

1. `match_title` refuses when two candidates are equally good. Describe a concrete case
   where that refusal costs a correct match — and argue why we accept the cost.
2. The narrowing stages prefer a dated candidate over an undated one. Why is that not
   the same kind of decision as preferring the candidate with more votes?
3. `Job::run_step` guarantees a crash never duplicates or loses work. What property
   provides that, and why can the embedding producer not have it?
4. The artefact refuses to load when its model identity does not match. Why is that a
   more important check than the checksum?
5. Quantising to int8 loses information. What is the property that actually has to
   survive, and how do we know it does?
