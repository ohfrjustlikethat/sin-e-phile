# Phase 6 — Source Resolver and Addon Protocol

**Status:** in_progress · **Depends on:** 3 · **Sessions:** 1–2

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
- [ ] **E5** `docs/specs/addon-protocol.md` is complete enough that a third party could implement a backend from it alone.
- [ ] **E6** Repository-wide grep confirms zero indexer names or URLs anywhere, including tests and docs.

## Subtasks

- [x] **6.1** crates/source-protocol (MIT OR Apache-2.0, no Tauri, no network): the SPEC.md §6.1 SourceBackend trait via async-trait, and MediaQuery, SourceCandidate (quality, language, codec, size, health, provenance), PlayableStream, BackendCapabilities. The addon manifest types, JSON Schema at docs/schemas/addon-manifest.schema.json, and a parse-only validator.
- [x] **6.2** Manifest hardening (E3): 64 KiB cap, depth and string-length limits, https only (plain http for loopback), every endpoint on the manifest's own host, a protocol version field. A corpus of hostile manifests plus a randomised no-panic test.
- [x] **6.3** crates/sources (GPL): the resolver. Queries every enabled backend concurrently, each under its own timeout, then dedups and merges. A deliberately slow backend must not delay the others, proved by a timing test seen to fail with the timeout removed (E4).
- [x] **6.4** Backend registry: enable/disable, ordering, per-backend health (successes, failures, mean latency), persisted by migration with repository_surface.rs lines for every new method.
- [x] **6.5** LocalFileBackend (stub until Phase 12) and DirectHttpBackend (user-supplied URLs and M3U). Each added by one file plus one registration line, the diff checked (E1).
- [x] **6.6** InternetArchiveBackend: public-domain feature-film search, then item metadata to a playable file. Tested against recorded responses, plus an ignored live test.
- [x] **6.7** E2 evidence: a live query resolves to a stream URL that ffprobe decodes; command and output recorded in docs/eval-results.md.
- [x] **6.8** AddonBackend: an addon is a manifest plus a remote service speaking the protocol's find/resolve JSON endpoints. Responses size-capped and time-limited like manifests.
- [ ] **6.9** UI, Settings -> Sources: backend list (enable, order, health); paste a manifest URL, see in plain language what it declares, confirm, enable, with no raw JSON in the primary path; a catalogue browser over a user-supplied URL (no default ships).
- [ ] **6.10** Search-triggered backfill (ADR-0030), minimal: when search misses and the resolver finds a candidate, a background write creates a media_items row marked as backfilled, from the title and year the backend reports. Filename parsing and IMDb merge stay with Phase 12.
- [ ] **6.11** docs/specs/addon-protocol.md written for a third party, tested by implementing a throwaway backend from it alone (E5); repository-wide guard over tree and history, seen to fail (E6).

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
