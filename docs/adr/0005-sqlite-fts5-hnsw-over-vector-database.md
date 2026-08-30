# 0005 — SQLite + FTS5 + HNSW over a vector database

- **Status:** Accepted
- **Date:** 2026-08-31
- **Phase:** 0 (recording a decision locked in `SPEC.md` §5)
- **Risks:** R3, R4

## Context

Phase 5 needs search that is simultaneously semantic and exact: *"slow films about
loneliness"* must work, and typing `Heat` must put **Heat** first, every time. That
needs vector similarity **and** keyword ranking, fused.

The constraints around it are unusually tight. §2.3 budgets the whole
keystroke-to-results path at 80 ms p95 **including query embedding** (ADR-0015),
enforced on Tier 0 — under 8 GB RAM, two cores. §5 forbids any server component.
§2.5 requires everything to live in `./data/` next to the executable and keep
working when the folder is moved to a USB stick. §2.6 forbids any running cost.

So the entire storage and retrieval stack has to be embedded, file-based, portable,
and fast on weak hardware.

## Decision

Three components, all embedded, no separate process:

- **SQLite via `sqlx`** (compile-time checked queries, WAL mode) as the single
  store for the catalogue and all relational data.
- **SQLite FTS5** with BM25 for keyword search, plus trigram fuzzy matching for
  typos. Built into SQLite — no extra dependency.
- **HNSW** (`hnsw_rs` or `instant-distance`) for approximate nearest-neighbour
  search over embeddings, persisted to disk and memory-mapped.

Results are combined by **reciprocal rank fusion**, with an exact-title
short-circuit that guarantees a literal title match ranks first.

## Consequences

**Easier.** One file for the relational data, one for the vector index, both inside
`./data/`. Portability (§2.5) is a property of the design rather than something to
engineer. Zero setup for a user and zero running cost, satisfying §2.6.

**Easier for a learner, specifically.** `sqlx` checks SQL against the real schema at
compile time, so a typo in a column name is a build error rather than a runtime
surprise — genuinely valuable for someone writing their first substantial SQL.

**Easier.** Keeping FTS5 in the same database as the catalogue means keyword search
can filter on structured columns (year, runtime, language) in the same query, which
is exactly what Phase 5's query-understanding step needs when it extracts
constraints from natural language.

**Harder.** Two indexes over the same corpus must be kept consistent. An item added
to the catalogue needs a row, an FTS5 entry, **and** a vector, and a partially
updated set is a silent correctness bug — an item that is findable by title but
never by meaning. Phase 5 needs an explicit reindexing path and a consistency check.

**Harder.** HNSW is an in-memory graph structure. Memory-mapping the persisted index
keeps resident memory acceptable, but index parameters (`M`, `ef_construction`,
`ef_search`) trade recall against memory and latency, and §8 tiers them — so the
same query can return slightly different results on different hardware. That must be
documented rather than discovered.

**Harder.** Reciprocal rank fusion has no principled way to weight one retriever
against the other; the constant `k` is tuned empirically. This is why the Phase 5
eval harness and its hand-graded corpus are a deliverable rather than a nicety —
without measurement, tuning is guessing.

## Alternatives Considered

**A dedicated vector database — Qdrant, Weaviate, Milvus.** Rejected outright: every
one is a server. §5 forbids a server component, §2.6 forbids a running cost, and
§2.5's "copy the folder to a USB stick" promise cannot survive a service that must
be installed and started.

**`sqlite-vss` / `sqlite-vec` extensions.** Genuinely attractive — one file, one
query language, no consistency problem, since vectors would live in SQLite
alongside everything else. Rejected for now on maturity and on Windows build
complexity for a loadable extension, and because a standalone HNSW crate gives
direct control over the recall/memory trade that §8's tiering requires. **This is
the alternative most likely to be worth revisiting**, and doing so would need only a
new ADR, not a redesign.

**Brute-force cosine similarity over all vectors, no ANN index.** Simplest possible
approach, and exact rather than approximate. Rejected on arithmetic: hundreds of
thousands of 384-dimension comparisons per keystroke does not fit an 80 ms budget on
two cores. Worth keeping as a correctness oracle in the eval harness — comparing
HNSW's results against exhaustive search is the honest way to measure recall.

**Postgres with `pgvector`.** Excellent, and completely incompatible with every
constraint above.

**Embeddings only, dropping BM25.** Rejected because it fails the exact-title
requirement, and failing it is unacceptable: a search engine that cannot reliably
find a film by its exact name is broken regardless of how good its semantic results
are.
