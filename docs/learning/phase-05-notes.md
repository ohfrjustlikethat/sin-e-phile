# Phase 5 — Learning notes

**Semantic search engine.** 5.1–5.8 and 5.10 done; 5.9 built and waiting on one upload. Four sections per ADR-0016
(A4): what we built, why, new concepts as concept + `file:line`, and the five questions.
No code tour.

---

## 1. What we built

- **The keyword half** — `crates/persistence/src/repositories/search.rs`. An FTS5 index
  over titles, alternative titles, people and keywords, BM25-ranked with column weights,
  a trigram index for typos, and an exact-title short-circuit that bypasses ranking
  entirely.
- **The relevance harness** — `tools/eval/src/search.rs` and
  `fixtures/search/exact-titles.tsv`. 43 deliberately awkward queries; E2 at 43/43.
- **The vector half's index** — `crates/vector-index/`. HNSW over the 855,703-vector
  artefact via usearch, keyed by catalogue id, saved to disk and memory-mapped on open.
- **Its ground truth** — `crates/vector-index/src/brute.rs`. An exact cosine scan of the
  same artefact, written from the artefact bytes rather than from usearch, so the two
  can disagree.
- **The vector harness** — `tools/eval/src/vector.rs`, `eval vector --report`. Recall@10
  against that ground truth, plus a `--prove` mode that deliberately breaks the id
  mapping and requires recall to collapse.
- **The query embedder** — `crates/embedder`. The ONNX sentence-transformer, lifted out
  of `tools/ingest` so the application can embed what the user types.
- **The engine** — `crates/search-engine`. Exact title, then BM25 and vectors fused by
  reciprocal rank, then fuzzy. Degrades to keyword-only when there is no artefact.
- **The text the catalogue never had** — `crates/metadata-api/src/wikipedia.rs` and
  `crates/catalogue/src/wikipedia.rs`. 233,114 Wikipedia synopses, joined through
  Wikidata's IMDb-id property, where there had been **zero**.
- **Query understanding** — `crates/search-engine/src/query.rs`. Year ranges, runtime
  bounds and "directed by" pulled out as constraints rather than embedded.
- **The relevance number** — `fixtures/search/semantic-queries.tsv` and
  `tools/eval/src/relevance.rs`. nDCG@10, meaning and filter scored apart.
- **The catalogue pipeline, moved** — `crates/catalogue`, out of `tools/ingest`, so the
  application can refresh itself on launch (`freshness.rs`).
- **The search screen** — `src/features/search/`, and `src-tauri/src/commands/search.rs`.
- **The optional downloads** — `crates/catalogue/src/assets.rs`, with consent.

## 2. Why

- **An approximate index has to be measured before anything is measured over it.** HNSW
  can miss a neighbour and say nothing. Every relevance number in this phase — E3's
  nDCG, E4's two hard queries, the rank fusion still to come — is computed over whatever
  candidates it returned. A poor nDCG over a lossy candidate set looks exactly like a
  poor ranking, and the natural response would be to tune the wrong half.
- **The artefact is positional, and that is a hazard the index can retire.** Vector `n`
  belongs to the nth core title in id order; the file records no ids. One new core title
  and every position after it names a different film. So the position → id mapping is
  resolved once, at build time, and stored as the index's keys — after that, nothing
  computes an offset ever again.
- **A negative control is not optional for this kind of test.** "The keys map to the
  right films" cannot be checked by looking at results: a wrong mapping still returns ten
  plausible films for every query. `--prove` rotates the ids by one and requires the
  measurement to fall apart. A recall number that survives a broken mapping would be
  measuring usearch against itself.
- **The core-tier predicate has one home.** It moved to `crates/persistence` because the
  producer and the *application* both need the same definition, and the application
  cannot depend on a dev tool.

- **The documents cannot answer a semantic question, and everything else was fine.**
  Zero synopses in 855,703 items, so the embedding of every film is little more than its
  title. Every artefact check passed — checksum, determinism, model identity, recall —
  because they check the file is *correct*, not that the documents are *informative*.
  The first real query found it in one look. See P12.

- **A filter is a constraint, not a sentence to embed.** Models are poor at numbers, and
  "from the 50s" embedded whole dilutes the part that carries meaning. Extracted, it is
  `release_year BETWEEN 1950 AND 1959` — exact, indexed, and free.
- **The application cannot depend on a dev tool.** Twice this phase: the query embedder
  and then the whole catalogue pipeline. Both were in `tools/`, both had to move, and the
  second was the reason an installed app's catalogue was frozen at install time.
- **A response should carry the context that stops it being misread.** "No results" at 3%
  ingested is a lie, so `SearchResponse` ships readiness with the hits and the UI cannot
  render the lie because it never holds the data alone.

## 3. New concepts

### Rust

- **`cxx` FFI and why usearch needed no CMake** — `crates/vector-index/Cargo.toml:14`.
  The bridge is generated by the `cxx` crate at build time and compiled with `cc`, so
  MSVC alone is enough on Windows.
- **`impl FnMut(usize)` as a progress callback** — `crates/vector-index/src/lib.rs:144`.
  A generic parameter rather than `&dyn`, so the closure inlines and the loop pays
  nothing when the caller does nothing.
- **`sort_by` with `partial_cmp` and an explicit tie-break** —
  `crates/vector-index/src/brute.rs:74`. `f32` is not `Ord`, so the comparator must
  handle the case Rust refuses to assume away; sorting on `(distance, id)` is what makes
  a rerun agree with itself.
- **Newtype-free unit safety by naming** — `Neighbour::distance` is documented as cosine
  *distance* (`crates/vector-index/src/lib.rs:117`), matching usearch's convention and
  BM25's "lower is better", because two conventions in one result list is a bug waiting.

- **`Option<Semantic>` as a degradation strategy** —
  `crates/search-engine/src/lib.rs:77`. The absent case is not an error path bolted on
  afterwards; it is the same code path with an empty list, which is why the Tier 0
  fallback cannot rot.

- **`tokio::sync::Mutex` vs `std::sync::Mutex`** — `src-tauri/src/state.rs:37`. The
  engine's lock is held across an `.await` (the ONNX forward pass), which a `std` mutex
  may not be.
- **A sequence number beats cancellation** — `src/features/search/SearchScreen.tsx:82`.
  Async searches return out of order; "kur" can outrun "kurosawa". Numbering the requests
  and dropping stale answers is simpler than cancelling in-flight work and cannot race.
- **Specta refuses `i64`** — `src-tauri/src/commands/search.rs:16`. A JavaScript number
  is a double, so anything past 2^53 arrives silently wrong. The annotation answers the
  objection with the bound written down rather than suppressing it.

### Search and vectors

- **HNSW** — a navigable small-world graph in layers: the top layer is sparse and
  long-range, each layer below denser, and a search descends greedily. `CONNECTIVITY`
  (*M*), `EXPANSION_ADD` (*efConstruction*) and `EXPANSION_SEARCH` (*ef*) are its three
  dials — `crates/vector-index/src/lib.rs:60`.
- **Why cosine survives quantisation exactly** — `crates/vector-index/src/lib.rs:16`.
  The artefact stores int8 values and discards each vector's scale. Cosine divides by
  both magnitudes, so a positive per-vector factor cancels: `cos(k·a, m·b) = cos(a, b)`.
  Inner product would have been wrong for exactly the same reason.
- **Recall@k vs rank agreement** — `crates/vector-index/src/brute.rs:79`. The index's
  job is to hand the ranking the right ten candidates; the order it hands them in is
  re-decided downstream, so recall is the property and rank correlation is not.
- **Memory-mapping an index** — `crates/vector-index/src/lib.rs:206`. `view` maps the
  file and lets the OS page in what a search touches. Measured: **6.6 MB resident
  mapped against 758.6 MB read in full**, for a 435 MB file. That is why usearch won
  P11 — 758 MB is three times Tier 0's whole 250 MB budget, and Tier 0 is the tier the
  artefact exists to serve.
- **Reciprocal rank fusion** — `crates/search-engine/src/lib.rs:177`. Combine two ranked
  lists by `Σ 1/(k + rank)`, ignoring the scores entirely. BM25 is negative and
  corpus-scaled; cosine is 0..2. Normalising two distributions that both move with the
  query is a tuning problem with no stable answer; positions need no calibration.
- **Why the embedder had to be ONE implementation** — `crates/embedder/src/lib.rs:8`.
  Tokenizer settings, truncation, pooling and normalisation are four chances for the
  query path and the producer to differ, and all four fail silently. Changing
  `MAX_TOKENS` from 256 to 32 dropped agreement to 2/10 — an ordinary-looking edit that
  would have degraded search with no error anywhere.
- **Reciprocal rank fusion** — `crates/search-engine/src/lib.rs`. Combine two ranked
  lists by `Σ 1/(k + rank)`, ignoring the scores: BM25 is negative and corpus-scaled,
  cosine is 0..2, and normalising two distributions that both move with the query is a
  tuning problem with no stable answer.
- **nDCG, and why it is normalised** — `tools/eval/src/relevance.rs`. Discount each hit
  by `log2(rank + 1)`, then divide by the best the fixture's own grades could achieve, so
  a query with four known answers does not outweigh one with three.
- **Hubness** — `crates/persistence/src/repositories/catalogue.rs`, `MIN_SYNOPSIS`. Short
  generic documents sit near the centre of an embedding space and are therefore close to
  *every* query. A 126-character synopsis of production trivia outscored a near-verbatim
  plot match 0.4194 to 0.1944. More text cannot fix a document that has none.
- **Symmetric similarity is not retrieval** — ADR-0034. `all-MiniLM-L6-v2` is trained to
  say whether two sentences mean the same thing; retrieval asks whether a long document
  answers a short query. Different task, and the reason for the model swap.
- **Asymmetric models need their prefixes** — `crates/embedder/src/lib.rs`. BGE instructs
  the query side and leaves the passage side bare. Prefixing both discards the asymmetry
  as completely as prefixing neither, and it fails silently either way.
- **CLS versus mean pooling** — `crates/embedder/src/lib.rs`, `Pooling`. BGE puts the
  sentence in token zero; MiniLM averages. The wrong choice yields valid, quietly worse
  vectors.
- **Conditional requests** — `crates/catalogue/src/freshness.rs`. A 216 MB file asked
  "have you changed?" costs one HEAD and a string compare.
- **A library default is not a chosen value** — `crates/vector-index/src/lib.rs:74`.
  usearch's `ef` of 64 measured 0.9400 recall@10 and missed the 0.95 gate. The sweep
  (64 → 384) is in `docs/eval-results.md`; 192 ships.

## 4. The questions

Written, not asked — Phase 5 is not a tier boundary (`SPEC.md` §10.10). They accumulate
until the end of Phase 8. Nine rather than five: this phase ran long and taught more than
five things, and dropping four to keep the count would be tidiness winning over the
point.

1. The exact-title short-circuit returns a film *without ranking it*. Why is that
   necessary for E2's 100%, when BM25 with a large enough title weight would almost
   always put the same film first?
2. The artefact stores int8 vectors and throws away each one's scale. Why does that not
   damage cosine, and what would it have broken if the metric were inner product?
3. `--prove` shifts every catalogue id by one position and requires recall to collapse.
   What could have been silently wrong that no ordinary passing run would have caught?
4. HNSW is approximate. Why measure recall against brute force *before* measuring nDCG,
   rather than just measuring nDCG — which is the number the exit criterion asks for?
5. The index is derived on the machine, and the artefact is downloaded. What would have
   to be true for shipping the graph instead to be the better choice?
6. The artefact passed every check Phase 4 wrote — checksum, determinism, model identity,
   and later recall@10 of 0.967 — and the documents inside it still could not answer
   "films about grief". What class of check was missing, and what would it have looked
   like?
7. *Fish Hooky*, whose synopsis says only that it was "the 120th Our Gang short to be
   released", scored 0.4194 against a plot query that *Manchester by the Sea* — whose
   synopsis describes that plot almost word for word — scored 0.1944 on. Why does a
   shorter, less relevant document win, and why does giving the right answer more text
   not fix it?
8. The screenshots found a bug the eval harness could not: eleven correct Hitchcock films
   labelled "matched words" when they had matched no words. What is the general class of
   defect that only a rendered interface can expose?
9. `freshness::remember` records the publisher's validator after the download and before
   the load, rather than at the end of the refresh. Both other placements were tried and
   both were wrong. What goes wrong at each end?
