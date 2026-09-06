# The vector index

**Status:** implemented (Phase 5, subtask 5.3) · **Decides:** P11 (usearch), [ADR-0014](../adr/0014-embedding-artefact-distribution.md), [ADR-0015](../adr/0015-tier-0-query-embedding.md)
**Code:** `crates/vector-index/` (index and ground truth) · `tools/ingest` (`ingest vector-index`) · `tools/eval/src/vector.rs` (the harness)

An HNSW graph over the [embedding artefact](embedding-artefact.md), derived on the
user's machine and memory-mapped at query time. This document is the reasoning behind
the parts that look arbitrary, and the measurements that decided them.

---

## 1. Why the graph is derived and not published

ADR-0014 publishes **vectors**, not graphs. Two reasons, and the second is the one that
settles it:

- A serialised HNSW is tied to the usearch version that wrote it. Publishing one makes a
  library upgrade a re-publication of a 400 MB asset, and a mismatch a support problem.
- The machine can derive it. Measured on the Tier 2 dev machine: **855,703 vectors in
  439 s**, once, at a rate that starts near 6,800/s and settles near 1,950/s as the
  graph fills. The result is **435 MB** on disk — 1.4x the artefact it indexes.

So the download stays one file with one checksum, and the index is a local build
artefact — safe to delete, rebuilt by `ingest vector-index`.

## 2. Configuration, and what each dial costs

| | Value | What it trades |
|---|---|---|
| metric | cosine | see §3 — the only defensible choice for this artefact |
| scalar kind | int8 | the artefact's own representation; no dequantisation in the hot path |
| connectivity (*M*) | 16 | graph size against recall |
| expansion_add (*efConstruction*) | 128 | **build minutes** against recall — paid once |
| expansion_search (*ef*) | 192 | **query latency** against recall — paid on every keystroke |

Only the last one is spent from E1's 80 ms budget, which is why it is the one the
harness sweeps. usearch's default of 64 measured **0.9400** recall@10 and would have
missed the 0.95 gate; 192 is the smallest value clearing it with margin. The full
sweep is in [`docs/eval-results.md`](../eval-results.md).

## 3. Cosine is not a preference here, it is forced

The artefact stores int8 values and **discards each vector's scale** (see the artefact
spec, §4 — the quantiser scales per vector, and only the values are written).

Cosine divides by both magnitudes, so a positive per-vector factor cancels top and
bottom: `cos(k·a, m·b) = cos(a, b)` for any positive `k`, `m`. The int8 vectors are
therefore the same *directions* as the float ones, and cosine over them is the same
number — not an approximation of it.

Inner product would have been wrong for exactly that reason: it keeps the magnitudes the
artefact threw away, so it would rank by an accident of each title's peak component.

## 4. The keys are catalogue ids, and that is the point

The artefact is positional and records no ids. Every consumer therefore has to
reconstruct "position *n* is which film" by re-running the producer's ordered query —
and a consumer that gets it wrong produces *plausible* results, not obviously broken
ones.

The index resolves that mapping **once, at build time**, and stores `media_items.id` as
the usearch key. After the build, no code computes an offset into the artefact again.

Two things protect the one moment it matters:

1. **A length check.** `VectorIndex::build` refuses when the id list and the artefact
   header disagree on count — which is what a catalogue that gained a core title after
   the artefact was built looks like.
2. **One definition of the core tier.** `CORE_TIER` lives in `crates/persistence`
   (`repositories/catalogue.rs`), shared by the producer's count, its batch reader, its
   resume cursor and `CatalogueRepository::core_ids`. It is in the persistence crate
   rather than the ingest tool because the **application** needs the same definition
   after a download, and the application cannot depend on a dev tool.

**Residual risk, stated rather than papered over:** a catalogue that both gained and
lost core titles in equal number passes the length check and misaligns anyway. Recorded
as debt; the real fix is a format version that carries its ids.

## 5. Memory: `view`, not `load`

`SPEC.md` §2.3 caps idle RAM at **250 MB on Tier 0**, and Tier 0 is precisely the tier
the artefact exists to serve (ADR-0015). An index that reads itself into memory
therefore cannot ship, and this is the whole reason usearch won P11 over the
alternatives: `Index::view()` maps the file and lets the operating system page in the
few thousand nodes a search actually touches.

`VectorIndex::load` exists only so the harness can measure the difference —
`eval vector --memory`, which opens the same file both ways: **6.6 MB resident mapped,
758.6 MB read in full**, the latter three times Tier 0's whole budget. Nothing in the
application calls `load`.

## 6. How it is measured

```
cargo run -p eval --release -- vector --report     # recall@10, latency, size
cargo run -p eval --release -- vector --prove      # the negative control
```

**Recall against brute force, not against usearch.** `crates/vector-index/src/brute.rs`
scans the artefact and computes cosine directly, from the artefact bytes and the
catalogue ids. usearch has its own `exact_search`, and using it would have compared
usearch against usearch — over the vectors usearch stored, through the metric usearch
implemented. A wrong key mapping would have agreed with itself and reported perfect
recall.

**`--prove` is the negative control.** It rotates the id list by one position, leaving
the artefact, the index and the metric untouched, and requires recall to collapse. A
harness that still scored well with a deliberately broken mapping would be measuring
nothing — and a wrong mapping is invisible by inspection, because it returns ten
plausible films for every query.

Numbers, with the commands that produced them, are in
[`docs/eval-results.md`](../eval-results.md).
