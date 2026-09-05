# Phase 4 — Metadata Backbone

**Status:** in_progress · **Depends on:** 3 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A local catalogue of hundreds of thousands of titles that works offline, enriched live on demand.

## Deliverables

`tools/ingest/`: a pipeline that downloads IMDb datasets, normalises them, joins TMDB data, ingests AniList's anime catalogue, and populates the database. Resumable — it must survive interruption. Progress reporting. A first-run flow that either ships a prebuilt index or builds it in the background with a good progress UI (the app is usable during the build, searching what's ingested so far). Live API clients for TMDB, AniList, Jikan, Fanart.tv with: a shared rate limiter, exponential backoff, a persistent response cache with sensible TTLs per resource type, and graceful offline behaviour. Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding, blurhash placeholders. External-ID cross-mapping (TMDB ↔ IMDb ↔ AniList ↔ MAL) with conflict resolution rules.

## Exit criteria

- [ ] **E1** Full ingestion completes on the dev machine and the resulting database is under a documented size budget.
- [x] **E2** Ingestion killed mid-run resumes correctly.
- [x] **E3** Catalogue lookups work with the network disconnected.
- [x] **E4** Rate limits are never exceeded under a stress test of 1,000 rapid lookups.
- [x] **E5** Anime titles resolve across AniList and TMDB with correct ID mapping for a hand-checked set of 50 titles including tricky cases (long-running shonen, split-cour seasons, films tied to series).
- [x] **E6** The catalogue is fully usable with no TMDB key (ADR-0013): titles, years, runtimes, genres, cast, crew and ratings all present from IMDb + MovieLens alone. TMDB enrichment adds artwork and rich detail and is verified to be additive, never load-bearing.
- [ ] **E7** The embedding artefact is produced and published (ADR-0014) by a reproducible script in `tools/ingest/`, run on the author's machine. It is deterministic, checksummed, resumable, and records model identity, quantisation, embedding dimension, document-builder version and catalogue snapshot date. The application refuses to load an artefact whose model identity does not match its own, and degrades to FTS5-only search when the artefact is absent.

## Subtasks

- [x] **4.1** tools/ingest skeleton: resumable job runner with checkpointing and progress reporting, so a killed run resumes rather than restarts
- [x] **4.2** IMDb dataset download, verification and normalisation into media_items, titles, people, credits, genres
- [ ] **4.3** MovieLens join for ratings and popularity (ADR-0019, on-device)
- [x] **4.4** AniList ingestion: anime catalogue, romaji/native/english titles, and seasonal episode numbering into episode_numbering. Absolute numbering is NULL by ADR-0031 - no free source publishes one.
- [x] **4.5** External-ID cross-mapping TMDB/IMDb/AniList/MAL with documented conflict-resolution rules
- [x] **4.6** Live API clients (TMDB, AniList, Jikan, Fanart.tv): shared rate limiter, exponential backoff, persistent response cache with per-resource TTLs, graceful offline
- [ ] **4.7** Per-profile TMDB key from settings, never shipped (ADR-0027), DPAPI-wrapped at rest; every TMDB-dependent surface degrades to the typographic state. Storage and the degradation contract are done; the settings UI is Phase 14.
- [ ] **4.8** Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding (42.1% measured on real stills), blurhash placeholders. crates/artwork is done; wiring it to a TMDB image client is Phase 11.
- [ ] **4.9** First-run flow: usable during the background build, searching what is ingested so far. CatalogueRepository::readiness is the backend surface (Empty/Building/Interrupted/Ready, each carrying the searchable count); the progress UI itself is Phase 14.
- [ ] **4.10** The embedding artefact producer (ADR-0014). crates/embedding holds the document builder, int8 quantisation and the checksummed file format, all tested. The PRODUCER still needs the ONNX model, which is gitignored and not on this machine.
- [x] **4.11** 50-title anime fixture for E5: fixtures/anime/e5-hand-checked.tsv, 64 rows, checked by `ingest verify-anime` which exits non-zero on any mismatch
- [x] **4.12** Rate-limit stress test: 1,000 rapid lookups never exceed the documented limits
- [x] **4.13** Incremental catalogue refresh (ADR-0030): re-fetch title.basics/ratings/episode and insert only the tail past the highest id already stored, reusing TsvReader::seek_past. Plus AniList airing schedules, which need no key. See docs/specs/catalogue-freshness.md.

## Risks named by this phase

- **R4** — Catalogue ingestion is far larger or slower than expected
- **R11** — Third-party API terms change under the project *(new, Phase 0)*

## Learning note

ETL pipelines; why offline-first beats API-first here; rate limiting and backoff; caching strategy and TTL choice; the anime metadata problem specifically.

---

<!-- subtask log: appended during the phase -->

## Work log
- **4.1** tools/ingest skeleton: resumable job runner with checkpointing and progress reporting, so a kill
- **4.2** IMDb dataset download, verification and normalisation into media_items, titles, people, credits, · `048180a`
- **4.5** External-ID cross-mapping TMDB/IMDb/AniList/MAL with documented conflict-resolution rules · `f67d939`
- **4.6** Live API clients (TMDB, AniList, Jikan, Fanart.tv): shared rate limiter, exponential backoff, pe · `38d75d7`
- **4.12** Rate-limit stress test: 1,000 rapid lookups never exceed the documented limits · `38d75d7`
- **4.4** AniList ingestion: anime catalogue, romaji/native/english titles, and seasonal episode numbering · `a170532`
- **4.11** 50-title anime fixture for E5: fixtures/anime/e5-hand-checked.tsv, 64 rows, checked by `ingest v
- **4.13** Incremental catalogue refresh (ADR-0030): re-fetch title.basics/ratings/episode and insert only  · `47e7353`
