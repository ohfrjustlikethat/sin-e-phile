# 0015 — Tier 0 embeds queries but never documents

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0 (verified in Phase 1 Spike C, implemented in Phase 5)
- **Amends:** `SPEC.md` §8, Phase 1, Phase 5
- **Risk:** R3

## Context

`SPEC.md` §8 states that Tier 0 machines have "embeddings precomputed and
shipped/downloaded, **never computed on device**". Phase 5 repeats this.

But semantic search requires embedding the *user's query* — a fresh string, typed
moments ago, that cannot possibly have been precomputed. Taken literally, §8 means
Tier 0 has no semantic search at all: it can hold a vector index it has no way to
query.

R3 notices this and says query embedding "would need a smaller model or a hash-based
fallback". Phase 5 does not mention it. So the spec's three statements on the
subject do not agree, and the disagreement decides whether Tier 0 — the tier every
performance budget in §2.3 is enforced against — has the project's headline feature.

## Decision

The rule is about **documents, not queries**, and is restated precisely:

> **Tier 0 never embeds catalogue documents on device. Tier 0 does embed one short
> query per search.**

The asymmetry is one of scale, not of kind. Embedding the catalogue is hundreds of
thousands of forward passes over long composed documents — hours of CPU, and exactly
what §8 is protecting a weak machine from. Embedding a query is **one forward pass
over roughly 30 tokens**, on a quantised INT8 MiniLM-class model. These differ by
five or six orders of magnitude, and collapsing them into a single prohibition was
an imprecision in §8, not a deliberate choice.

Both `SPEC.md` §8 and Phase 5 are amended to state the distinction explicitly.

### Two conditions, both binding

**1. Spike C must measure query-embedding latency specifically.** Not document
latency, not throughput, not an amortised average. The measurement is: cold model
already loaded, single ~30-token query, wall-clock time to a returned vector, at
p50 and p95, on the dev machine and on a constrained VM approximating Tier 0.

**2. The §2.3 80 ms p95 search budget is measured *including* query embedding.**
The budget covers the whole keystroke-to-results path — embed, HNSW search, FTS5
search, reciprocal rank fusion, render. Query embedding does not get its own
allowance outside the budget, because the user experiences the total.

### The escalation trigger

**If Spike C returns query-embedding p95 above ~30 ms, escalate under §10.9.** Do
not quietly widen the 80 ms budget.

30 ms is chosen as the point at which embedding consumes more than a third of the
total budget, leaving too little for ANN search, BM25, fusion and render. Above it,
the real options are: a smaller or more aggressively quantised model; caching
embeddings for repeated queries; debouncing so that not every keystroke triggers a
full search; or FTS5-first with semantic results arriving progressively. Each is a
legitimate answer, and each is a decision for the author, not a budget adjustment
made silently at 2am. This trigger is recorded in `docs/RISKS.md` under R3 so that
it is decided in advance rather than under frustration — which is what §16 Appendix
D asks of every trigger.

## Consequences

**Easier.** Tier 0 gets genuine semantic search rather than keyword search with an
unusable vector index. Since §2.3's budgets are enforced against Tier 0, and Tier 0
is the machine the project claims to be excellent on, this is the difference between
the headline feature working for the target user and not.

**Easier.** ONNX Runtime is now required at runtime on **all** tiers, which
simplifies the build and the tier-gating logic: one code path, one model load, and
`tiers.rs` gates only whether *document* embedding is permitted.

**Harder — and this raises R3's importance.** ONNX Runtime becoming unusable on
Windows would now break semantic search on every tier, not just degrade Tier 0. R3's
likelihood is unchanged but its impact rises from Moderate to Moderate/Severe, and
`docs/RISKS.md` records that. Spike C is correspondingly more load-bearing than the
spec's ordering implies, and its result should be treated as a gate on Phase 5 —
which is a reason to run it early in Phase 1 rather than last.

**Harder.** The model must be resident in memory during search, which counts against
the §2.3 250 MB Tier 0 idle RAM budget. A quantised MiniLM-class model is roughly
25–45 MB resident. Spike C measures this too; if it threatens the budget, lazy-load
the model on first search rather than at startup, which also helps the < 4 s Tier 0
cold-start budget.

## Alternatives Considered

**Hash-based query fallback on Tier 0** — a locality-sensitive or random-projection
embedding for queries only. Rejected: query and document vectors must live in the
*same* space to be comparable, so this would require hashing the documents too, and
the resulting quality is far below a real sentence transformer. This remains R3's
fallback if Spike C fails badly, but it is a degradation, not a design.

**FTS5-only on Tier 0, no semantic search.** The literal reading of §8, and rejected
because §8's own rule says every tier-gated feature must degrade to something
*good*, never to something broken or empty. Losing "slow films about loneliness" —
the capability §4 names as the whole point of Phase 5 — on the tier the project
optimises for is not a good degradation. Retained as the failure fallback in
ADR-0014, not as the design.

**Precompute embeddings for common queries.** Rejected: the query space is
unbounded, and the queries that matter most are the unusual, specific ones. It would
help exactly the queries that need help least.

**A second, smaller model for queries only.** Rejected unless Spike C forces it:
two models means two memory footprints, and query and document vectors from
different models are not comparable without a learned projection, which is a large
amount of machinery for a problem that may not exist.
