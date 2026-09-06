# Phase 5 — Semantic Search Engine

**Status:** in_progress · **Depends on:** 4 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Search that understands meaning, is instant, works offline, and never gets an exact title wrong.

## Deliverables

FTS5 index over titles, alternative titles, people, and keywords, with BM25 ranking and trigram fuzzy matching for typos. `ort` (ONNX Runtime) running a quantised sentence-transformer; a document text builder that composes each item's embedding input from synopsis, genres, keywords, director, mood descriptors, and era. HNSW index over the embeddings, persisted to disk, memory-mapped. **Reciprocal rank fusion** combining BM25 and vector results, with an exact-title short-circuit that guarantees a literal title match always ranks first. Query understanding: detect and extract structured filters from natural language (year ranges, "in Japanese", "under 100 minutes", "directed by") and apply them as constraints rather than as embedding input. Tier 0 path: **catalogue document** embeddings precomputed and downloaded, never generated on device — but the user's query *is* embedded on device on every tier, since that is one forward pass over ~30 tokens (§8, ADR-0015). The 80 ms p95 budget below includes it. Search UI: instant results as you type, grouped by kind, with keyboard navigation and a "why this matched" hint on semantic results. `fixtures/search/` corpus and the relevance eval harness.

## Exit criteria

- [ ] **E1** p95 keystroke-to-results < 80 ms over the full catalogue.
- [x] **E2** Exact-title top-1 rate is 100% on the fixture corpus.
- [ ] **E3** nDCG@10 > 0.75 on the semantic query set.
- [ ] **E4** "films about grief that aren't depressing" and "like Wong Kar-wai but Korean" both return defensible results — documented with screenshots in the case study.
- [ ] **E5** Works fully offline.

## Subtasks

- [x] **5.1** FTS5 index over titles, alternative titles, people and keywords, with BM25 ranking. Migration; unicode61 with diacritic folding, plus a trigram index for typo tolerance. Verified available: SQLite 3.46.0 bundled with sqlx supports fts5, trigram, unicode61 and bm25().
- [x] **5.2** The exact-title short-circuit. E2 demands a 100% top-1 rate, so a literal title match must bypass ranking entirely rather than merely score highly. titles.normalised (migration 0009) already exists for this.
- [x] **5.3** HNSW over the 855,703-vector artefact, persisted and memory-mapped. P11 resolved: usearch, chosen because Index::view() mmaps rather than loads - a 313 MB index in RAM breaks the SPEC.md 2.3 Tier 0 budget of 250 MB, which is the tier the artefact exists to serve. Recall must be measured against brute force before any relevance number is reported.
- [x] **5.4** Reciprocal rank fusion of BM25 and vector results, with the exact-title short-circuit above both. Built as crates/search-engine, NOT in crates/persistence: fusion needs usearch and ONNX Runtime, and those in the persistence crate would make a native model runtime a dependency of repository_surface.rs. RRF k=60 is the literature default and is NOT yet measured - nothing exists to measure it against until 5.6's semantic fixture.
- [ ] **5.5** Query understanding: extract year ranges, languages, runtime bounds and 'directed by' as structured FILTERS rather than feeding them to the embedder.
- [ ] **5.6** fixtures/search/ and the relevance harness. exact-titles.tsv (43 queries) and `eval search` are done and E2 is at 100%. `eval vector` is done: recall@10 0.9670 against brute force, with a --prove negative control. semantic-queries.tsv and nDCG@10 (E3) still need the query embedder and the fusion from 5.4.
- [ ] **5.7** Open the database in the application (debt D26) and RE-MEASURE COLD START against Phase 1's 515/660 ms before and after.
- [ ] **5.8** Search UI: results as you type, grouped by kind, keyboard navigable, with a 'why this matched' hint. Must consult CatalogueRepository::readiness before saying 'no results'.
- [ ] **5.9** Tier 0 artefact download with consent and the size shown (ADR-0014, debt D27), and the FTS5-only degradation when it is absent.
- [x] **5.10** crates/embedder: lift the ONNX sentence-transformer out of tools/ingest so the application can embed a query (ADR-0015 embeds queries on every tier). One implementation shared by producer and query path, because tokenizer settings, truncation, pooling and normalisation each fail SILENTLY when they differ. `eval embed --report` proves agreement byte-identically against the artefact and was seen to fail (2/10) when MAX_TOKENS was changed.

## Risks named by this phase

- **R3** — ONNX Runtime is painful to build on Windows, or too slow on Tier 0

## Learning note

What an embedding is, geometrically; BM25 in one paragraph; why hybrid search beats either alone; approximate nearest neighbour and the HNSW trade-off; reciprocal rank fusion; nDCG.

---

<!-- subtask log: appended during the phase -->

## Work log
- **5.1** FTS5 index over titles, alternative titles, people and keywords, with BM25 ranking. Migration; u · `9d4123a`
- **5.2** The exact-title short-circuit. E2 demands a 100% top-1 rate, so a literal title match must bypas · `9d4123a`
- **5.3** HNSW over the 855,703-vector artefact, persisted and memory-mapped. P11 resolved: usearch, chose · `554c50d`
- **5.4** Reciprocal rank fusion of BM25 and vector results, with the exact-title short-circuit above both · `bdbd560`
- **5.10** crates/embedder: lift the ONNX sentence-transformer out of tools/ingest so the application can e · `396f256`
