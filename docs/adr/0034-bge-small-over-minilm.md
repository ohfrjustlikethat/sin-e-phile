# 0034 — `bge-small-en-v1.5` replaces `all-MiniLM-L6-v2`

- **Status:** Accepted
- **Date:** 2026-09-07
- **Phase:** 5
- **Resolves:** P13, blocker B4 (a §10.9 escalation)
- **Supersedes the model pin in:** ADR-0014, ADR-0015 (both keep their reasoning; only the model changes)
- **Risk:** R3

## Context

Three genuinely distinct attempts to make E3 and E4 achievable, each measured, none
sufficient (`docs/eval-results.md`):

1. **Enrich the documents** — 233,114 Wikipedia synopses where there were zero.
2. **Index only items with descriptive text** — 855,703 → 189,470.
3. **Remove the title from the document** (`document::VERSION` 2), plus a 300-character
   minimum to drop trivia-length hubs — → 119,874.

The measurement that ended the sequence, against *"a grieving janitor becomes guardian of
his teenage nephew in a Massachusetts fishing town"*:

| | `all-MiniLM-L6-v2` |
|---|---|
| **Manchester by the Sea** — synopsis contains the query almost verbatim | **0.1944** |
| **Fish Hooky** — synopsis: "the 120th Our Gang short to be released" | **0.4194** |

Pure production trivia outscored a near-verbatim plot match by more than two to one, and
giving the correct answer its whole 1,847-character synopsis reached only 0.2689. That is
**hubness**: short generic documents sit near the centre of the embedding space and are
therefore close to every query. It cannot be fixed with more text, because those
documents have no more text.

**The diagnosis is the model, not the data.** `all-MiniLM-L6-v2` is trained for
**symmetric similarity** — are these two sentences equivalent? E3 and E4 need
**asymmetric retrieval** — does this long document answer this short query? They are
different tasks, and the project had been asking the wrong one.

## Decision

**`bge-small-en-v1.5`**, int8-quantised, replaces `all-MiniLM-L6-v2`.

### Why this one

- **Trained for retrieval**, with an explicit query/passage asymmetry.
- **384 dimensions**, identical to MiniLM — so the artefact format, the quantiser, the
  HNSW index, the 313 MB budget and every measurement built on them are untouched. The
  swap is a constant, a pooling strategy and a prefix.
- **MIT licensed**, no key, no account, same distribution story.
- 32.4 MB against MiniLM's 22.9 MB — a real but small increase in an optional download.

### Two things that are not optional, and both fail silently

**CLS pooling.** BGE is trained with the `[CLS]` token as the sentence representation;
MiniLM is trained with mean pooling. Using the wrong one produces perfectly valid, quietly
worse vectors. [`Pooling`] is therefore an explicit parameter rather than an assumption,
and `Embedder::pinned` is the constructor every call site uses so the three settings
cannot drift apart.

**The query prefix.** `"Represent this sentence for searching relevant passages: "` is
applied to queries and never to documents. Prefixing both sides discards the asymmetry as
completely as prefixing neither, so `embed_query` and `embed_document` are separate
methods and `embed` is not called directly by anything outside the crate.

### Verified before committing the compute

A re-embed is 75 minutes, so the choice was measured first —
`eval embed --compare`, same query, same documents:

| item | MiniLM | **BGE** | synopsis |
|---|---|---|---|
| **Manchester by the Sea** | 0.1944 | **0.4947** | 1,847 |
| Fish Hooky (trivia) | 0.4194 | 0.4694 | 126 |
| The Big Lebowski | — | 0.4325 | 1,424 |
| Toy Story | — | 0.3716 | 2,967 |
| Alien | — | 0.3295 | 2,582 |

The correct answer moves from **below** the trivia to clearly above it, with a real spread
against unrelated films. That is the property MiniLM did not have.

## Consequences

- **Every prior artefact is invalid** and is refused rather than read: the model identity
  in the header no longer matches. The `embeddings-v1` release is superseded, which was
  already true after ADR-0033.
- A full re-embed and index rebuild. No schema, format or API change.
- `MODEL_SHA256` and `TOKENIZER_SHA256` are re-pinned. The model is fetched from the same
  host as before.
- **E3 and E4 are still not claimed.** This ADR records a better model and one
  measurement, not a passing criterion. `eval search --query` decides.

## Alternatives Considered

- **`e5-small-v2`.** Same size, same 384 dimensions, mean pooling, `query:` / `passage:`
  prefixes. An equivalent choice; BGE benchmarks slightly better on retrieval and the
  decision was not close enough to be worth a second 75-minute run.
- **A cross-encoder re-ranker over the top 50.** What a production system would do, and
  it would fix ordering properly. Rejected: it runs *per query*, straight into E1's 80 ms
  Tier 0 budget, when the whole architecture exists to keep that budget.
- **Accept E3/E4 as not met.** Still available if the numbers disappoint, and now it would
  be a conclusion rather than a shrug.
