# Phase 4 — Metadata Backbone

**Status:** complete · **Depends on:** 3 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A local catalogue of hundreds of thousands of titles that works offline, enriched live on demand.

## Deliverables

`tools/ingest/`: a pipeline that downloads IMDb datasets, normalises them, joins TMDB data, ingests AniList's anime catalogue, and populates the database. Resumable — it must survive interruption. Progress reporting. A first-run flow that either ships a prebuilt index or builds it in the background with a good progress UI (the app is usable during the build, searching what's ingested so far). Live API clients for TMDB, AniList, Jikan, Fanart.tv with: a shared rate limiter, exponential backoff, a persistent response cache with sensible TTLs per resource type, and graceful offline behaviour. Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding, blurhash placeholders. External-ID cross-mapping (TMDB ↔ IMDb ↔ AniList ↔ MAL) with conflict resolution rules.

## Exit criteria

- [x] **E1** Full ingestion completes on the dev machine and the resulting database is under a documented size budget.
- [x] **E2** Ingestion killed mid-run resumes correctly.
- [x] **E3** Catalogue lookups work with the network disconnected.
- [x] **E4** Rate limits are never exceeded under a stress test of 1,000 rapid lookups.
- [x] **E5** Anime titles resolve across AniList and TMDB with correct ID mapping for a hand-checked set of 50 titles including tricky cases (long-running shonen, split-cour seasons, films tied to series).
- [x] **E6** The catalogue is fully usable with no TMDB key (ADR-0013): titles, years, runtimes, genres, cast, crew and ratings all present from IMDb + MovieLens alone. TMDB enrichment adds artwork and rich detail and is verified to be additive, never load-bearing.
- [x] **E7** The embedding artefact is produced and published (ADR-0014) by a reproducible script in `tools/ingest/`, run on the author's machine. It is deterministic, checksummed, resumable, and records model identity, quantisation, embedding dimension, document-builder version and catalogue snapshot date. The application refuses to load an artefact whose model identity does not match its own, and degrades to FTS5-only search when the artefact is absent.

## Subtasks

- [x] **4.1** tools/ingest skeleton: resumable job runner with checkpointing and progress reporting, so a killed run resumes rather than restarts
- [x] **4.2** IMDb dataset download, verification and normalisation into media_items, titles, people, credits, genres
- [x] **4.3** MovieLens join for ratings and popularity (ADR-0019, on-device)
- [x] **4.4** AniList ingestion: anime catalogue, romaji/native/english titles, and seasonal episode numbering into episode_numbering. Absolute numbering is NULL by ADR-0031 - no free source publishes one.
- [x] **4.5** External-ID cross-mapping TMDB/IMDb/AniList/MAL with documented conflict-resolution rules
- [x] **4.6** Live API clients (TMDB, AniList, Jikan, Fanart.tv): shared rate limiter, exponential backoff, persistent response cache with per-resource TTLs, graceful offline
- [x] **4.7** Per-profile TMDB key from settings, never shipped (ADR-0027), DPAPI-wrapped at rest; every TMDB-dependent surface degrades to the typographic state. Storage and the degradation contract are done; the settings UI is Phase 14. CLOSED with Phase 4: the Phase 4 part (storage, DPAPI, degradation contract) is done; the settings UI is Phase 14's deliverable.
- [x] **4.8** Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding (42.1% measured on real stills), blurhash placeholders. crates/artwork is done; wiring it to a TMDB image client is Phase 11. CLOSED with Phase 4: crates/artwork is done; wiring it to a TMDB image client is Phase 11's deliverable.
- [x] **4.9** First-run flow: usable during the background build, searching what is ingested so far. CatalogueRepository::readiness is the backend surface (Empty/Building/Interrupted/Ready, each carrying the searchable count); the progress UI itself is Phase 14. CLOSED with Phase 4: the readiness surface is done; the progress UI is Phase 14's deliverable.
- [x] **4.10** The embedding artefact producer (ADR-0014). crates/embedding holds the document builder, int8 quantisation and the checksummed format; tools/ingest/src/embed.rs is the resumable producer, model and tokenizer checksum-pinned; format specified in docs/specs/embedding-artefact.md. PRODUCED: 855,703 vectors, 313 MB, sha256 3fce6f06..., verified by `ingest verify-embeddings`. Publishing it is blocker B3.
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
- **4.10** The embedding artefact producer (ADR-0014). crates/embedding holds the document builder, int8 qu · `4f302d5`
- **4.3** MovieLens join for ratings and popularity (ADR-0019, on-device) · `42177d6`
- **4.7** Per-profile TMDB key from settings, never shipped (ADR-0027), DPAPI-wrapped at rest; every TMDB- · `2358993`
- **4.8** Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding (42.1% measured on r · `5840b99`
- **4.9** First-run flow: usable during the background build, searching what is ingested so far. Catalogue · `624b76f`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `bf5a644`.

### Evidence per criterion

**E1** — Full ingestion completes on the dev machine and the resulting database is under a documented size budget.

> Re-confirmed 2026-10-25 under the SI reading of R4's 4 GB (author's D47 option 1). RESULTING DATABASE 3,945,598,976 bytes = 3.946 GB, 54 MB under 4 GB, after `./target/release/ingest.exe compact` at 814fc41 (4,062,978,048 -> 3,945,598,976 bytes, 111 MiB recovered, 34 s; recorded in docs/eval-results.md Session 19). Every ingestion command now compacts after itself (df3ca35). Margin is thin: the app's launch-time refresh does not compact, so it erodes the 54 MB (docs/RISKS.md R4). Ingestion completeness unchanged from 2026-09-07: title.basics 2,701,195, title.principals 10,092,368, name.basics 2,736,235, title.akas 5,908,381, episodes 132,845 rows / 5,862 seasons / 4,529 series, MovieLens 61,912 of 62,423 films matched (99.2%). The artefact and index live outside the database and are counted separately (D33).

**E2** — Ingestion killed mid-run resumes correctly.

> `cargo test -p sinephile-ingest --test resume` -> 10 passing, 2026-09-05. a_killed_run_resumes_and_repeats_nothing asserts 50 items exist EXACTLY once after a kill at item 20; progress_survives_a_restart; a_step_that_does_not_advance_its_cursor_is_stopped; and an_interrupted_run_resumes_at_the_next_page in tests/anime.rs. Demonstrated for real as well: the AniList sweep was interrupted and resumed during development, and a_crash_mid_step_reports_that_it_is_resuming was corrected after `is_resuming` was found reporting a fresh run on every real resume.

**E3** — Catalogue lookups work with the network disconnected.

> STRUCTURAL: crates/persistence has no HTTP client in its dependency tree - sqlx is configured sqlite-only with no postgres/mysql feature, and the rest is serde, thiserror, time, tracing, windows-sys. The catalogue layer is incapable of a network call, so lookups cannot depend on one. Exercised by 72 passing tests in `cargo test -p sinephile-persistence`, including the 500,000-row indexed-lookup benchmark, none of which touch a network.

**E4** — Rate limits are never exceeded under a stress test of 1,000 rapid lookups.

> `cargo test -p sinephile-metadata-api` -> limiter::tests::a_thousand_rapid_requests_never_exceed_the_limit, 2026-09-05. 1,000 acquisitions against a 40-per-10-seconds limit must take at least (1000-40)/4 = 240 simulated seconds; the test fails if they are faster. Uses tokio's paused clock, so it measures the limiter rather than the wall.

**E5** — Anime titles resolve across AniList and TMDB with correct ID mapping for a hand-checked set of 50 titles including tricky cases (long-running shonen, split-cour seasons, films tied to series).

> `./target/release/ingest verify-anime` -> 64/64 pass, 2026-09-04. Fixture fixtures/anime/e5-hand-checked.tsv covers long-running shonen, split-cour seasons and films tied to series, plus 3 expected REFUSALS and 2 season claims. Numbers and the four fixture errors the first run exposed are in docs/eval-results.md. TMDB half of E5 is not covered - no key ships (ADR-0027), so it is verified per-user in subtask 4.7.

**E6** — The catalogue is fully usable with no TMDB key (ADR-0013): titles, years, runtimes, genres, cast, crew and ratings all present from IMDb + MovieLens alone. TMDB enrichment adds artwork and rich detail and is verified to be additive, never load-bearing.

> MEASURED on the real catalogue, 2026-09-05, with NO TMDB key ever supplied: 0 rows in external_ids with source='tmdb' and 0 TMDB responses in http_cache. The catalogue nonetheless holds 2,702,737 titles, 855,703 core-tier, 10,079,841 credits, 6,176,950 title rows, 4,450,735 genre links, 1,086,210 ratings, 2,008,183 runtimes and 539,817 episodes - all from IMDb and AniList alone. ADR-0027's per-profile key surface exists (crates/persistence/src/repositories/credentials.rs) and defaults to TmdbAccess::Absent.

**E7** — The embedding artefact is produced and published (ADR-0014) by a reproducible script in `tools/ingest/`, run on the author's machine. It is deterministic, checksummed, resumable, and records model identity, quantisation, embedding dimension, document-builder version and catalogue snapshot date. The application refuses to load an artefact whose model identity does not match its own, and degrades to FTS5-only search when the artefact is absent.

> MET 2026-09-15. PRODUCED: 855,703 vectors, 313 MB, deterministic, checksummed, resumable, recording model identity, quantisation, dimension, document-builder version, snapshot date and text source (format v2). PUBLISHED: release embeddings-v2 on github.com/ohfrjustlikethat/sin-e-phile, asset embeddings-bge-small-en-v1.5-int8.bin, 328,590,240 bytes, confirmed by `gh release view` and by the URL returning HTTP 200 at that exact length. DOWNLOADED AND VERIFIED end to end through the application's own code path (`assets::download` then `assets::verify`): internal checksum, model identity and document-builder version all pass - see docs/eval-results.md. THE REFUSAL IS EVIDENCED TOO: the superseded embeddings-v1 artefact is refused by name, 'artefact was built for model "all-MiniLM-L6-v2-int8", this build has "bge-small-en-v1.5-int8"'. DEGRADES to FTS5-only when absent: Engine::keyword_only, selected by the app at launch and surfaced in the UI.

### Debt incurred

- **D12** A title released AFTER the last ingestion run is not in the catalogue at all: the IMDb datasets are a snapshot and IMDb refreshes daily. With a TMDB key, subtask 4.6's live clients can fill the gap on demand; with no key - which ADR-0027 makes the default - nothing currently covers it. Surfaced by the author asking whether new releases would be available. Owner: subtask 4.9's first-run flow.
- **D13** WEEKLY TV EPISODE FRESHNESS has no owner. The author asked whether a newly-aired episode would be available seamlessly, the way a streaming app manages it. Finding a SOURCE is Phases 6/7/9 and needs nothing special - a new episode resolves like any other title. KNOWING THE EPISODE EXISTS is the gap: title.episode is a snapshot, IMDb refreshes daily and we do not. For anime, AniList publishes airing schedules through a public API with no key, so subtask 4.4 can solve it properly. For general TV it needs either the user's TMDB key or a periodic re-fetch of title.episode (one of the smaller files). Neither is currently specified. Owner: propose a mechanism before Phase 11, which is where next-episode handling lands.
- **D14** EXPECTATION TO MANAGE, not a defect: the author asked whether the bittorrent client will rival qBittorrent for seamless downloads. SPEC.md Phase 7's stated goal is 'torrents that stream, not torrents that download'. For STREAMING a purpose-built deadline scheduler should beat qBittorrent, which is not optimised for sequential playback. For BULK DOWNLOAD THROUGHPUT, matching a decade of tuned peer selection and disk I/O is a much harder bar and is not a stated goal - R2 already rates librqbit's streaming control as a live risk. Phase 7 should measure against qBittorrent on both axes and report honestly rather than let the comparison stay implicit.
- **D15** RESOLVED by migration 0007 on data-quality grounds, not the stated ones. The 2,250,937 redundant title rows are gone and a search no longer matches the same film six times. The 270 MB saving I predicted did not happen: the new unique index carries the title TEXT where the old one carried two short codes, so the index grew by more than the table shrank. Net -23 MB. See docs/eval-results.md.
- **D16** RESOLVED. The >= 10 core vote threshold survives. The embedding artefact is 313 MB for 855,703 core titles at 384 int8 dimensions - arithmetic over the fixed format in crates/embedding/src/artefact.rs, not a prediction - against 452 MB of R4 headroom, and it lives outside the database anyway. See docs/eval-results.md.
- **D17** titles.variant calls a title 'native' whenever its script is non-Latin, regardless of whether that language has anything to do with the work. 93,900 rows are marked native with a language other than Japanese - a Greek or Ukrainian release title of a Japanese film is an ALTERNATIVE title, not a native one. Fixing it properly needs media_items.original_language plumbed into tools/ingest/src/akas.rs::variant and a repair pass like tools/ingest/src/repair.rs::english_variants. Not urgent: nothing reads 'native' as 'the original title' yet, and Phase 5 searches every variant.
- **D18** AniList refuses to paginate past 5,000 entries, so the catalogue is swept per seasonYear. Entries with NO seasonYear are reachable only through the unfiltered popularity pass, which hits the same 5,000 cap - so an undated AniList entry outside the 5,000 most popular is never ingested. Acceptable because the matcher needs a year as half its evidence, but it is a real coverage gap and should be stated rather than discovered later.
- **D19** crates/metadata-api/src/anilist.rs::default_limit is hard-coded to AniList's documented 90 requests/minute, but the service has been enforcing something lower for a long time. The backoff absorbs the 429s correctly, so nothing fails - it just spends a request to rediscover the limit each time. AniList returns X-RateLimit-Limit and X-RateLimit-Remaining; reading those and reconfiguring the limiter would honour whatever the server actually allows.
- **D20** data/sinephile.db holds the UNION of three AniList sweeps - 7,383 items carry an anilist id where a single clean sweep produces 7,328. Every mapping was made by the same matcher under the same rules, so none is wrong, but the database is not byte-reproducible from one run. Clearing the anilist/mal rows in external_ids and reverting anime_* kinds before a final sweep would fix it; not worth 13 minutes until the catalogue is being frozen for Phase 27.
- **D21** Episodes are loaded for all anime plus non-anime series with >=5,000 votes: 539,817 episodes, 21,218 seasons. The threshold is one constant, passed as `ingest episodes --min-votes N`, and the loader skips what it already holds so widening is safe. Measured cost is 410 bytes per episode across three independent runs. Dropping to >=1,000 costs 344 MB and would leave 117 MB, which is less than ADR-0014's own rate for the embedding artefact - so this cannot widen until embeddings and MovieLens are measured.
- **D22** PHASE 12 INHERITS THIS. episode_numbering.absolute_number is NULL on all 539,817 episodes and no free source publishes one (ADR-0031). A filename reading 'Series - 59' cannot be resolved from the catalogue for anime, so Phase 12's matcher must route it to the review queue rather than assume a lookup succeeds - a correct outcome, not a wrong answer, so the <1% false-confident budget is unaffected. The fix, when Phase 12 wants it: stop collapsing AniList seasons onto one catalogue item, give each its own episode_numbering rows, and the absolute number becomes derivable from stored facts instead of computed from guesses. NEVER derive it by cumulating counts across seasons.
- **D23** RESOLVED. tools/state/validate_state.py now refuses when current_phase and its phases[] entry disagree on any criterion's met/evidence or any subtask's status/commit. Recorded as debt the first time it bit (E5); fixed the second time (E7), because a class of error that recurs inside one session will not be prevented by remembering.
- **D24** RESOLVED, and it decided a design. `ingest refresh` re-downloaded title.basics (216 MB) every run because gzip cannot be seeked and IMDb publishes no changelog; this entry guessed that conditional requests would fix it. MEASURED 2026-09-07: IMDb serves ETag and Last-Modified, and a GET carrying If-None-Match returns HTTP 304 with 0 BYTES. So a launch-time refresh check costs one round trip when nothing has changed, and IMDb republishes daily so at most one real download a day. That is what makes the author's 'refresh every time the app opens' shippable rather than a 216 MB tax per launch. See docs/eval-results.md.
- **D25** The TMDB key is DPAPI-wrapped, so it is bound to the Windows user account and a copied ./data/ folder cannot decrypt it. That is deliberate (crates/persistence/src/secrets.rs) and degrades to TmdbAccess::Absent, but it means the PORTABLE promise of ADR-0008 has one documented exception: move the folder to another machine or account and the key must be re-entered. Everything else in the folder still works. Worth a line in the settings UI when Phase 14 builds it, so the user is told rather than surprised.
- **D26** RESOLVED 2026-09-13 (subtask 5.7). AppState now holds a Db, opened on a background task and installed when ready, so nothing on the path to the first painted frame waits for a disk. Cold start re-measured at 328-522 ms across five launches against Phase 1's 515/660 baseline - no regression. Callers that arrive before the open completes get None rather than a stall.
- **D27** The embedding artefact is PRODUCED but not PUBLISHED. ADR-0014 says it ships as a versioned GitHub Release asset with its checksum, downloaded on consent with the size shown. Producing it is automated; uploading a 313 MB file to a Release is a manual step the author takes, and the download-with-consent flow is Phase 5's, since nothing reads the artefact until search does. sha256 3fce6f062c25220a80425cce7e9f83a3b80412af3b34fe4e9071d3b80645a2b0.

### Next phase starts by

Phases 4 and 5 are closed (merge bf5a644, tags phase-04 and phase-05). In progress on branch chore/masthead: the masthead (SPEC.md §9.4, ADR-0037), built ahead of Phase 18 at the author's request. Replace src/app/TitleBar.tsx and src/app/NavRail.tsx with one src/app/Masthead.tsx (wordmark, the five destinations as words with an oxblood underline on the active one, a labelled Search button with its '/' hint, Settings, window controls; empty space is the drag region), remove railExpanded and toggleRail from src/lib/store.ts and --rail-* from src/styles/tokens.css, and verify that it fits at 1024x640. After that: plan Phase 6 (source-resolver) by reading SPEC.md §15 Phase 6 and authoring its subtasks in PROJECT_STATE.json. AUTHOR ACTION OUTSTANDING: publish data/embeddings-bge-small-en-v1.5-int8.bin as GitHub release embeddings-v3; the downloader already points there.
