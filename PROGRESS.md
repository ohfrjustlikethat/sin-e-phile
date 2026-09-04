# Progress

> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by
> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two
> can never disagree). Edit the state file, then regenerate.

**Spec version 1.9.0** · 5 session(s) completed · last updated 2026-09-05

---

## Where we are right now

**Phase 4 — Metadata Backbone** (`in_progress`, branch `phase/04-metadata-backbone`)

1 of 7 exit criteria met with evidence.

> The catalogue. Three constraints carry in: ADR-0013 and ADR-0027 mean the app must be complete and good-looking with NO TMDB key and no key ever ships; ADR-0026 means SQL is runtime-checked, so every new repository method needs a line in crates/persistence/tests/repository_surface.rs; and R4 (ingestion larger or slower than expected) is this phase's named risk — measure before committing to a shape, and scope by a popularity threshold rather than ingesting everything.

### Subtasks — 8/13 complete

- [x] **4.1** tools/ingest skeleton: resumable job runner with checkpointing and progress reporting, so a killed run resumes rather than restarts · `4a78d64`
- [x] **4.2** IMDb dataset download, verification and normalisation into media_items, titles, people, credits, genres · `048180a`
- [ ] **4.3** MovieLens join for ratings and popularity (ADR-0019, on-device)
- [x] **4.4** AniList ingestion: anime catalogue, romaji/native/english titles, and seasonal episode numbering into episode_numbering. Absolute numbering is NULL by ADR-0031 - no free source publishes one. · `a170532`
- [x] **4.5** External-ID cross-mapping TMDB/IMDb/AniList/MAL with documented conflict-resolution rules · `f67d939`
- [x] **4.6** Live API clients (TMDB, AniList, Jikan, Fanart.tv): shared rate limiter, exponential backoff, persistent response cache with per-resource TTLs, graceful offline · `38d75d7`
- [ ] **4.7** Per-profile TMDB key from settings, never shipped (ADR-0027); every TMDB-dependent surface degrades to the typographic state
- [ ] **4.8** Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding, blurhash placeholders
- [ ] **4.9** First-run flow: usable during the background build, searching what is ingested so far
- [ ] **4.10** The embedding artefact producer (ADR-0014): deterministic, checksummed, resumable, recording model identity, quantisation, dimension, document-builder version and catalogue snapshot date; published as a GitHub Release asset
- [x] **4.11** 50-title anime fixture for E5: fixtures/anime/e5-hand-checked.tsv, 64 rows, checked by `ingest verify-anime` which exits non-zero on any mismatch · `f19ddda`
- [x] **4.12** Rate-limit stress test: 1,000 rapid lookups never exceed the documented limits · `38d75d7`
- [x] **4.13** Incremental catalogue refresh (ADR-0030): re-fetch title.basics/ratings/episode and insert only the tail past the highest id already stored, reusing TsvReader::seek_past. Plus AniList airing schedules, which need no key. See docs/specs/catalogue-freshness.md. · `47e7353`

### Exit criteria

- [ ] **E1** Full ingestion completes on the dev machine and the resulting database is under a documented size budget.
- [ ] **E2** Ingestion killed mid-run resumes correctly.
- [ ] **E3** Catalogue lookups work with the network disconnected.
- [ ] **E4** Rate limits are never exceeded under a stress test of 1,000 rapid lookups.
- [x] **E5** Anime titles resolve across AniList and TMDB with correct ID mapping for a hand-checked set of 50 titles including tricky cases (long-running shonen, split-cour seasons, films tied to series).
      - *Evidence:* `./target/release/ingest verify-anime` -> 64/64 pass, 2026-09-04. Fixture fixtures/anime/e5-hand-checked.tsv covers long-running shonen, split-cour seasons and films tied to series, plus 3 expected REFUSALS and 2 season claims. Numbers and the four fixture errors the first run exposed are in docs/eval-results.md. TMDB half of E5 is not covered - no key ships (ADR-0027), so it is verified per-user in subtask 4.7.
- [ ] **E6** The catalogue is fully usable with no TMDB key (ADR-0013): titles, years, runtimes, genres, cast, crew and ratings all present from IMDb + MovieLens alone. TMDB enrichment adds artwork and rich detail and is verified to be additive, never load-bearing.
- [ ] **E7** The embedding artefact is produced and published (ADR-0014) by a reproducible script in `tools/ingest/`, run on the author's machine. It is deterministic, checksummed, resumable, and records model identity, quantisation, embedding dimension, document-builder version and catalogue snapshot date. The application refuses to load an artefact whose model identity does not match its own, and degrades to FTS5-only search when the artefact is absent.

---

## What's next

Phase 4 is 12 of 13 with only 4.3 outstanding, and 4.3 is BLOCKED on B2 - files.grouplens.org served an expired TLS certificate on 2026-09-04. FIRST: re-run `ingest movielens` and see whether the certificate has been renewed; its code is complete and tested, so a successful download finishes the subtask. If it is still expired, run `/closephase` against the exit criteria instead and record 4.3 as carried forward - do not work around the certificate.

---

## Blockers

- **B2** Subtask 4.3 cannot complete: files.grouplens.org serves an EXPIRED TLS certificate (subject files.grouplens.org, issuer InCommon ECC Server CA 2, notAfter 2026-08-28; today 2026-09-04). Plain HTTP redirects to HTTPS, so there is no path that does not hit it. NOT worked around: disabling certificate verification would ship a security downgrade to every user for a third party's expired cert, and an unallowlisted mirror has unverifiable provenance and would need an ADR. The parent grouplens.org has a VALID certificate, so this is one host and likely to be renewed. Everything that does not depend on the download is finished and tested against a synthetic archive - re-run `ingest movielens` when the certificate is renewed. No decision needed from the author.

---

## All 28 phases

Tiers are the legitimate stopping points from `SPEC.md` Appendix E. **Tier B is the definition of done** — complete it and the project has succeeded.

| | # | Phase | Tier | Depends on | Sessions | Criteria met |
|---|---|---|---|---|---|---|
| [x] | 0 | Bootstrap and Project Infrastructure | A | nothing | 1 | 8/8 |
| [x] | 1 | Application Shell and Capability Tiers | A | 0 | 1–2 | 7/7 |
| [x] | 2 | Design System and Visual Language | A | 1 | 1–2 | 5/5 |
| [x] | 3 | Data Layer and Portable Storage | A | 1 | 1–2 | 5/5 |
| [~] | 4 | Metadata Backbone | A | 3 | 2–3 | 1/7 |
| [ ] | 5 | Semantic Search Engine | A | 4 | 2 | 0/5 |
| [ ] | 6 | Source Resolver and Addon Protocol | A | 3 | 1–2 | 0/6 |
| [ ] | 7 | Torrent Engine and Streaming Server | A | 6 | 2–3 | 0/8 |
| [ ] | 8 | Player Core — MILESTONE: FIRST DEMOABLE BUILD 🏁 | A | 5, 7 | 2–3 | 0/6 |
| [ ] | 9 | Intelligent Source Selection | B | 8 | 1–2 | 0/6 |
| [ ] | 10 | Subtitle Pipeline | B | 8 | 2 | 0/6 |
| [ ] | 11 | Player Experience Layer | B | 8, 9, 10 | 2 | 0/5 |
| [ ] | 12 | Local Library Engine | B | 4 | 2–3 | 0/6 |
| [ ] | 13 | Download Manager and the Stream-vs-Download Advisor | B | 7, 9, 12 | 1–2 | 0/8 |
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
- **D12** (raised in Phase 4) A title released AFTER the last ingestion run is not in the catalogue at all: the IMDb datasets are a snapshot and IMDb refreshes daily. With a TMDB key, subtask 4.6's live clients can fill the gap on demand; with no key - which ADR-0027 makes the default - nothing currently covers it. Surfaced by the author asking whether new releases would be available. Owner: subtask 4.9's first-run flow.
- **D13** (raised in Phase 4) WEEKLY TV EPISODE FRESHNESS has no owner. The author asked whether a newly-aired episode would be available seamlessly, the way a streaming app manages it. Finding a SOURCE is Phases 6/7/9 and needs nothing special - a new episode resolves like any other title. KNOWING THE EPISODE EXISTS is the gap: title.episode is a snapshot, IMDb refreshes daily and we do not. For anime, AniList publishes airing schedules through a public API with no key, so subtask 4.4 can solve it properly. For general TV it needs either the user's TMDB key or a periodic re-fetch of title.episode (one of the smaller files). Neither is currently specified. Owner: propose a mechanism before Phase 11, which is where next-episode handling lands.
- **D14** (raised in Phase 4) EXPECTATION TO MANAGE, not a defect: the author asked whether the bittorrent client will rival qBittorrent for seamless downloads. SPEC.md Phase 7's stated goal is 'torrents that stream, not torrents that download'. For STREAMING a purpose-built deadline scheduler should beat qBittorrent, which is not optimised for sequential playback. For BULK DOWNLOAD THROUGHPUT, matching a decade of tuned peer selection and disk I/O is a much harder bar and is not a stated goal - R2 already rates librqbit's streaming control as a live risk. Phase 7 should measure against qBittorrent on both axes and report honestly rather than let the comparison stay implicit.
- **D15** (raised in Phase 4) RESOLVED by migration 0007 on data-quality grounds, not the stated ones. The 2,250,937 redundant title rows are gone and a search no longer matches the same film six times. The 270 MB saving I predicted did not happen: the new unique index carries the title TEXT where the old one carried two short codes, so the index grew by more than the table shrank. Net -23 MB. See docs/eval-results.md.
- **D16** (raised in Phase 4) R4 headroom is 461 MB after episodes (3,635 MB of 4,096 MB). The embedding artefact, AniList and MovieLens all still have to fit. The >=10 core vote threshold in tools/ingest/src/imdb.rs::CatalogueScope::DEFAULT is the lever if they do not - tightening to >=50 cuts the core from 854,752 titles to roughly 484,000, which reduces credits and akas proportionately. Decide ONCE, after those three are measured. Four storage predictions in this phase have been wrong in both directions; a fifth would be no better.
- **D17** (raised in Phase 4) titles.variant calls a title 'native' whenever its script is non-Latin, regardless of whether that language has anything to do with the work. 93,900 rows are marked native with a language other than Japanese - a Greek or Ukrainian release title of a Japanese film is an ALTERNATIVE title, not a native one. Fixing it properly needs media_items.original_language plumbed into tools/ingest/src/akas.rs::variant and a repair pass like tools/ingest/src/repair.rs::english_variants. Not urgent: nothing reads 'native' as 'the original title' yet, and Phase 5 searches every variant.
- **D18** (raised in Phase 4) AniList refuses to paginate past 5,000 entries, so the catalogue is swept per seasonYear. Entries with NO seasonYear are reachable only through the unfiltered popularity pass, which hits the same 5,000 cap - so an undated AniList entry outside the 5,000 most popular is never ingested. Acceptable because the matcher needs a year as half its evidence, but it is a real coverage gap and should be stated rather than discovered later.
- **D19** (raised in Phase 4) crates/metadata-api/src/anilist.rs::default_limit is hard-coded to AniList's documented 90 requests/minute, but the service has been enforcing something lower for a long time. The backoff absorbs the 429s correctly, so nothing fails - it just spends a request to rediscover the limit each time. AniList returns X-RateLimit-Limit and X-RateLimit-Remaining; reading those and reconfiguring the limiter would honour whatever the server actually allows.
- **D20** (raised in Phase 4) data/sinephile.db holds the UNION of three AniList sweeps - 7,383 items carry an anilist id where a single clean sweep produces 7,328. Every mapping was made by the same matcher under the same rules, so none is wrong, but the database is not byte-reproducible from one run. Clearing the anilist/mal rows in external_ids and reverting anime_* kinds before a final sweep would fix it; not worth 13 minutes until the catalogue is being frozen for Phase 27.
- **D21** (raised in Phase 4) Episodes are loaded for all anime plus non-anime series with >=5,000 votes: 539,817 episodes, 21,218 seasons. The threshold is one constant, passed as `ingest episodes --min-votes N`, and the loader skips what it already holds so widening is safe. Measured cost is 410 bytes per episode across three independent runs. Dropping to >=1,000 costs 344 MB and would leave 117 MB, which is less than ADR-0014's own rate for the embedding artefact - so this cannot widen until embeddings and MovieLens are measured.
- **D22** (raised in Phase 4) PHASE 12 INHERITS THIS. episode_numbering.absolute_number is NULL on all 539,817 episodes and no free source publishes one (ADR-0031). A filename reading 'Series - 59' cannot be resolved from the catalogue for anime, so Phase 12's matcher must route it to the review queue rather than assume a lookup succeeds - a correct outcome, not a wrong answer, so the <1% false-confident budget is unaffected. The fix, when Phase 12 wants it: stop collapsing AniList seasons onto one catalogue item, give each its own episode_numbering rows, and the absolute number becomes derivable from stored facts instead of computed from guesses. NEVER derive it by cumulating counts across seasons.
- **D23** (raised in Phase 4) PROJECT_STATE.json holds exit criteria and subtasks TWICE - once in phases[] and once in current_phase - and tools/state/validate_state.py --check passes while the two disagree. Marking E5 met in phases[] left current_phase saying false, and only tools/statecheck caught it, via the phase document. The schema was meant to make this class of drift structural (author's ruling, 2026-09-01). It does not cover the `met`/`evidence`/`status` fields. Fix: add a schema rule, or a validate_state check, that current_phase mirrors its phases[] entry - the copies should not be editable independently at all.
- **D24** (raised in Phase 4) `ingest refresh` re-downloads title.basics (216 MB) every run because gzip cannot be seeked and IMDb publishes no changelog. Weekly is fine; daily would not be. If the cadence ever tightens, check whether IMDb serves conditional requests - the http_cache table from migration 0008 already stores ETag and Last-Modified, and crates/metadata-api handles revalidation, so a 304 would make this nearly free.
- **D25** (raised in Phase 4) The TMDB key is DPAPI-wrapped, so it is bound to the Windows user account and a copied ./data/ folder cannot decrypt it. That is deliberate (crates/persistence/src/secrets.rs) and degrades to TmdbAccess::Absent, but it means the PORTABLE promise of ADR-0008 has one documented exception: move the folder to another machine or account and the key must be re-entered. Everything else in the folder still works. Worth a line in the settings UI when Phase 14 builds it, so the user is told rather than surprised.

---

## Decisions pending

- **P1** — decide by Phase 27 Source-only distribution versus Phase 27 packaging and the 2.3 installed-size budget. See docs/DECISIONS_PENDING.md.
- **P5** — decide by Phase 12 Where the Phase 12 review-queue confidence threshold sits, given >95% top-1 and <1% false-confident pull against each other. Measure, do not guess. See docs/DECISIONS_PENDING.md.
- **P6** — decide by Phase 27 Windows Sandbox pass on a genuinely bare machine. The E1 CI job proves a clean checkout builds, but windows-latest ships Rust, Node and MSVC preinstalled, so it does not prove SETUP.md is complete from nothing.
- **P8** — decide by Phase 21 Spike C measured query-embedding latency on the Tier 2 dev machine only. ADR-0015 also asked for a constrained VM approximating Tier 0. Unpadded headroom is ~18x so this does not block Phase 5, but the padded worst case with a 3-4x Tier 0 penalty lands at 24-33 ms, close to the 30 ms trigger. Measure before Phase 21 signs off the 80 ms search budget.
