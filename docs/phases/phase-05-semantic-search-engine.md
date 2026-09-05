# Phase 5 — Semantic Search Engine

**Status:** not_started · **Depends on:** 4 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Search that understands meaning, is instant, works offline, and never gets an exact title wrong.

## Deliverables

FTS5 index over titles, alternative titles, people, and keywords, with BM25 ranking and trigram fuzzy matching for typos. `ort` (ONNX Runtime) running a quantised sentence-transformer; a document text builder that composes each item's embedding input from synopsis, genres, keywords, director, mood descriptors, and era. HNSW index over the embeddings, persisted to disk, memory-mapped. **Reciprocal rank fusion** combining BM25 and vector results, with an exact-title short-circuit that guarantees a literal title match always ranks first. Query understanding: detect and extract structured filters from natural language (year ranges, "in Japanese", "under 100 minutes", "directed by") and apply them as constraints rather than as embedding input. Tier 0 path: **catalogue document** embeddings precomputed and downloaded, never generated on device — but the user's query *is* embedded on device on every tier, since that is one forward pass over ~30 tokens (§8, ADR-0015). The 80 ms p95 budget below includes it. Search UI: instant results as you type, grouped by kind, with keyboard navigation and a "why this matched" hint on semantic results. `fixtures/search/` corpus and the relevance eval harness.

## Exit criteria

- [ ] **E1** p95 keystroke-to-results < 80 ms over the full catalogue.
- [ ] **E2** Exact-title top-1 rate is 100% on the fixture corpus.
- [ ] **E3** nDCG@10 > 0.75 on the semantic query set.
- [ ] **E4** "films about grief that aren't depressing" and "like Wong Kar-wai but Korean" both return defensible results — documented with screenshots in the case study.
- [ ] **E5** Works fully offline.

## Subtasks

_none authored yet — write them when the phase is planned._

## Risks named by this phase

- **R3** — ONNX Runtime is painful to build on Windows, or too slow on Tier 0

## Learning note

What an embedding is, geometrically; BM25 in one paragraph; why hybrid search beats either alone; approximate nearest neighbour and the HNSW trade-off; reciprocal rank fusion; nDCG.

---

<!-- subtask log: appended during the phase -->

## Work log

