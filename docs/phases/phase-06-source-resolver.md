# Phase 6 — Source Resolver and Addon Protocol

**Status:** in_progress · **Depends on:** 3 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The interface every source in the system will implement, forever. Get this right.

## Deliverables

The `SourceBackend` trait (§6.1) and its types: `MediaQuery`, `SourceCandidate` (with quality, language, codec, size, health, provenance), `PlayableStream`, `BackendCapabilities`. A resolver that queries backends concurrently with per-backend timeouts, deduplicates, and merges results. Backend registry with enable/disable, ordering, and per-backend health tracking. **The addon manifest format**: a declarative TOML/JSON schema describing a source, published as `crates/source-protocol` with a JSON Schema and a specification document in `docs/specs/addon-protocol.md`. Three reference backends: `LocalFileBackend` (stub until Phase 12), `InternetArchiveBackend` (real, working, legal), `DirectHttpBackend` (user-supplied URLs). An addon installation UI: paste a manifest URL, see what it declares, confirm, enable — with plain-language explanations and no raw JSON in the primary path. A catalogue browser that fetches a user-supplied catalogue URL; **no default URL ships**.

## Exit criteria

- [ ] **E1** Adding a new backend requires implementing one trait and registering it — no changes elsewhere.
- [ ] **E2** The Internet Archive backend returns real, playable results.
- [ ] **E3** A malformed or hostile addon manifest is rejected with a clear error and cannot crash or hang the app.
- [ ] **E4** Backend timeout: one slow backend never delays results from the others.
- [ ] **E5** `docs/specs/addon-protocol.md` is complete enough that a third party could implement a backend from it alone.
- [ ] **E6** Repository-wide grep confirms zero indexer names or URLs anywhere, including tests and docs.

## Subtasks

_none authored yet — write them when the phase is planned._

## Learning note

Trait objects and `async_trait` in Rust; designing an interface you can't change later; concurrent queries with timeouts; why declarative-over-executable matters for untrusted input.

---

<!-- subtask log: appended during the phase -->

## Work log

