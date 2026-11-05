# Phase 6 — Source Resolver and Addon Protocol

**Status:** complete · **Depends on:** 3 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The interface every source in the system will implement, forever. Get this right.

## Deliverables

The `SourceBackend` trait (§6.1) and its types: `MediaQuery`, `SourceCandidate` (with quality, language, codec, size, health, provenance), `PlayableStream`, `BackendCapabilities`. A resolver that queries backends concurrently with per-backend timeouts, deduplicates, and merges results. Backend registry with enable/disable, ordering, and per-backend health tracking. **The addon manifest format**: a declarative TOML/JSON schema describing a source, published as `crates/source-protocol` with a JSON Schema and a specification document in `docs/specs/addon-protocol.md`. Three reference backends: `LocalFileBackend` (stub until Phase 12), `InternetArchiveBackend` (real, working, legal), `DirectHttpBackend` (user-supplied URLs). An addon installation UI: paste a manifest URL, see what it declares, confirm, enable — with plain-language explanations and no raw JSON in the primary path. A catalogue browser that fetches a user-supplied catalogue URL; **no default URL ships**.

## Exit criteria

- [x] **E1** Adding a new backend requires implementing one trait and registering it — no changes elsewhere.
- [x] **E2** The Internet Archive backend returns real, playable results.
- [x] **E3** A malformed or hostile addon manifest is rejected with a clear error and cannot crash or hang the app.
- [x] **E4** Backend timeout: one slow backend never delays results from the others.
- [x] **E5** `docs/specs/addon-protocol.md` is complete enough that a third party could implement a backend from it alone.
- [x] **E6** Repository-wide grep confirms zero indexer names or URLs anywhere, including tests and docs.

## Subtasks

- [x] **6.1** crates/source-protocol (MIT OR Apache-2.0, no Tauri, no network): the SPEC.md §6.1 SourceBackend trait via async-trait, and MediaQuery, SourceCandidate (quality, language, codec, size, health, provenance), PlayableStream, BackendCapabilities. The addon manifest types, JSON Schema at docs/schemas/addon-manifest.schema.json, and a parse-only validator.
- [x] **6.2** Manifest hardening (E3): 64 KiB cap, depth and string-length limits, https only (plain http for loopback), every endpoint on the manifest's own host, a protocol version field. A corpus of hostile manifests plus a randomised no-panic test.
- [x] **6.3** crates/sources (GPL): the resolver. Queries every enabled backend concurrently, each under its own timeout, then dedups and merges. A deliberately slow backend must not delay the others, proved by a timing test seen to fail with the timeout removed (E4).
- [x] **6.4** Backend registry: enable/disable, ordering, per-backend health (successes, failures, mean latency), persisted by migration with repository_surface.rs lines for every new method.
- [x] **6.5** LocalFileBackend (stub until Phase 12) and DirectHttpBackend (user-supplied URLs and M3U). Each added by one file plus one registration line, the diff checked (E1).
- [x] **6.6** InternetArchiveBackend: public-domain feature-film search, then item metadata to a playable file. Tested against recorded responses, plus an ignored live test.
- [x] **6.7** E2 evidence: a live query resolves to a stream URL that ffprobe decodes; command and output recorded in docs/eval-results.md.
- [x] **6.8** AddonBackend: an addon is a manifest plus a remote service speaking the protocol's find/resolve JSON endpoints. Responses size-capped and time-limited like manifests.
- [x] **6.9** UI, Settings -> Sources: backend list (enable, order, health); paste a manifest URL, see in plain language what it declares, confirm, enable, with no raw JSON in the primary path; a catalogue browser over a user-supplied URL (no default ships).
- [x] **6.10** Search-triggered backfill (ADR-0030), minimal: when search misses and the resolver finds a candidate, a background write creates a media_items row marked as backfilled, from the title and year the backend reports. Filename parsing and IMDb merge stay with Phase 12.
- [x] **6.11** docs/specs/addon-protocol.md written for a third party, tested by implementing a throwaway backend from it alone (E5); repository-wide guard over tree and history, seen to fail (E6).

## Learning note

Trait objects and `async_trait` in Rust; designing an interface you can't change later; concurrent queries with timeouts; why declarative-over-executable matters for untrusted input.

---

<!-- subtask log: appended during the phase -->

## Work log
- **6.1** crates/source-protocol (MIT OR Apache-2.0, no Tauri, no network): the SPEC.md §6.1 SourceBackend · `ff88924`
- **6.2** Manifest hardening (E3): 64 KiB cap, depth and string-length limits, https only (plain http for  · `ff88924`
- **6.3** crates/sources (GPL): the resolver. Queries every enabled backend concurrently, each under its o · `6f99ed7`
- **6.4** Backend registry: enable/disable, ordering, per-backend health (successes, failures, mean latenc · `a024811`
- **6.5** LocalFileBackend (stub until Phase 12) and DirectHttpBackend (user-supplied URLs and M3U). Each  · `bd80a1f`
- **6.6** InternetArchiveBackend: public-domain feature-film search, then item metadata to a playable file · `9ab7929`
- **6.7** E2 evidence: a live query resolves to a stream URL that ffprobe decodes; command and output reco · `fc50bf4`
- **6.8** AddonBackend: an addon is a manifest plus a remote service speaking the protocol's find/resolve  · `ebaba3f`
- **6.9** UI, Settings -> Sources: backend list (enable, order, health); paste a manifest URL, see in plai · `1136e78`
- **6.10** Search-triggered backfill (ADR-0030), minimal: when search misses and the resolver finds a candi · `61ab278`
- **6.11** docs/specs/addon-protocol.md written for a third party, tested by implementing a throwaway backe · `4e81c94`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `ea25da1`.

### Evidence per criterion

**E1** — Adding a new backend requires implementing one trait and registering it — no changes elsewhere.

> Commit bd80a1f adds LocalFileBackend as crates/sources/src/backends/local.rs plus its registration in backends/mod.rs (git show --stat bd80a1f: 2 files, +98 -3) and touches nothing else; the resolver, registry and property tests (crates/sources/tests/backends.rs, every_backend_has_a_unique_well_formed_id, committed earlier in d234361) picked it up unedited. The registry discovers backends only through backends::all().

**E2** — The Internet Archive backend returns real, playable results.

> docs/eval-results.md 'Phase 6 — Source resolver': live test live_the_archive_returns_a_reachable_whole_film (cargo test -p sinephile-sources --test archive -- --ignored, commit 9ab7929) found 3 candidates for Night of the Living Dead (1968); the resolved URL returned HEAD 200, ffprobe showed h264 640x480 + aac, 5744.97 s, and ffmpeg -t 30 -f null decoded 900 frames with exit 0 and no errors. Recorded-response tests (finds_whole_films_from_recorded_responses) run in CI.

**E3** — A malformed or hostile addon manifest is rejected with a clear error and cannot crash or hang the app.

> Manifest: crates/source-protocol/tests/hostile_manifests.rs — 25 hostile fixtures each refused by its own named rule, 50 MB / 64K-bracket / bad-UTF-8 inputs refused in <250 ms, 10,000 seeded mutations with no panic (commit ff88924; seen to fail with same-host, depth and bidi checks removed). Fetch: crates/metadata-api/tests/untrusted_http.rs — body cap enforced with no or a lying Content-Length, no redirects (9036577; seen to fail). Addon at runtime: crates/sources/tests/addons.rs a_hostile_addon_costs_only_itself — a redirecting, a 5 MB-flooding and a 30 s-stalling addon each fail alone and the search returns in ~8 s, the default timeout; a_stored_manifest_that_no_longer_passes_is_not_loaded; wire.rs a_remote_stream_may_never_name_a_local_file (ebaba3f; seen to fail with redirects accepted, the stored-manifest recheck removed, and file streams allowed). Panics: resolver a_panicking_backend_fails_alone (6f99ed7).

**E4** — Backend timeout: one slow backend never delays results from the others.

> cargo test -p sinephile-sources --test resolver: a_fast_backend_answers_before_a_slow_one_times_out (fast batch at <50 ms virtual while the slow backend times out at 300 ms), the_whole_search_is_bounded_by_the_timeout_not_the_slowest_backend (find_all returns at the 500 ms timeout although one backend would take 1 h), a_timed_out_backend_is_stopped_not_abandoned, a_panicking_backend_fails_alone. Commit 6f99ed7. Seen to fail with the timeout removed (took 3600s), the abort removed, and backends asked sequentially (took 520ms).

**E5** — `docs/specs/addon-protocol.md` is complete enough that a third party could implement a backend from it alone.

> docs/specs/addon-protocol.md (commit 4e81c94) specifies every manifest field and limit, both endpoints' requests and answers, timeouts, catalogues and versioning. tools/protocol/example_addon.py, a Python stdlib addon written against that document, passes the app's own client via crates/sources/tests/conformance.rs (SINEPHILE_ADDON=http://127.0.0.1:8765/manifest.json cargo test -p sinephile-sources --test conformance -- --ignored: manifest ok, 1 candidate, resolve ok, unknown key refused as not-found); a copy with an endpoint on another host fails it with the same-host message. Caveat: the example was written by the same author as the code, so an implementation by an outside person remains the stronger test.

**E6** — Repository-wide grep confirms zero indexer names or URLs anywhere, including tests and docs.

> python tools/guard/guard.py --tree: clean over 517 tracked files; --history: clean over all 167 commits on phase/06-source-resolver (2026-11-05, after 4e81c94); --selftest: all 48 checks pass, the guard fires when it should. Seen to fail in real use this phase: the staged guard refused commits for undeclared domains (subdomains of the reserved example domains, and a licence host in recorded fixtures) and for a 40-hex example info hash, each fixed by removing the content, never by suppressing the guard. The hashed denylist is empty by design (tools/guard/denylist.txt header, ADR-0009); the allowlist check is the enforcement.

### Debt incurred

- **D54** The Internet Archive's items also carry an 'Archive BitTorrent' file and, often, machine-generated SubRip subtitles (seen in NightOfTheLivingDead-MPEG). The Phase 6 backend ignores both: the torrent is Phase 7's (a second, P2P candidate for the same film), the subtitles Phase 10's.
- **D55** A valid Archive candidate can be briefly unreachable: on 2026-11-05 one download redirected to a storage node whose name did not resolve, and minutes later to one that did (docs/eval-results.md, Phase 6 E2). Phase 8's player must treat a failed open as 'try the next candidate', not as the film being unavailable, and the resolver's health record should count it.
- **D56** E5's example addon (tools/protocol/example_addon.py) was written by the same author as the protocol code, against the document. An implementation by someone outside the project is still the stronger test of 'a third party could implement a backend from it alone'; Phase 27 is a natural moment to ask for one.
- **D57** LocalFileBackend.find returns nothing until Phase 12's library scanner exists; it is registered now so order and settings already include it. Search backfill builds only from backend-reported titles and years; filename parsing and merging a backfilled row into an IMDb match are Phase 12's (docs/specs/catalogue-freshness.md layer 3).
- **D58** metadata-api's FakeTransport returns queued responses LAST-IN-FIRST-OUT, although its doc comment said first-in-first-out. catalogue/tests/anime.rs depends on the stack order, so the comment was corrected rather than the behaviour (commit 9036577). Any new test queuing several responses must push them in reverse.
- **D59** The Sources settings screen was audited from headless-Chrome renders with a fake IPC bridge (scratch harness, not committed) at 1440x900 and 1024x640, and restyled once (1749551). It has not been seen in the real WebView2 window; the Phase 18 overhaul covers Settings.

### Next phase starts by

Phases 4 and 5 are closed (merge bf5a644). The masthead is built on branch chore/masthead and merges to main once CI is green. Next: plan Phase 6 (source-resolver). Read SPEC.md §15 Phase 6, author its subtasks in PROJECT_STATE.json (both copies), and present the plan before code. AUTHOR ACTIONS OUTSTANDING: (1) publish data/embeddings-bge-small-en-v1.5-int8.bin as GitHub release embeddings-v3, because the downloader already points there and v2 can't be read; (2) with the machine idle, run `powershell -ExecutionPolicy Bypass -File tools/shots/capture.ps1 -SettleMs 9000 -QueryMs 3500` so the case-study shots show the current search screen.
