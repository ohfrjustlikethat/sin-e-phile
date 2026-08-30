# 0018 — TMDB text in embeddings is inference, not training; text source is swappable

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 0 (binds Phases 4, 5)
- **Resolves:** P2 · **Risk:** R11

## Context

- TMDB's API terms **prohibit using the data for AI/ML training**.
- Phase 5's document builder composes embedding input partly from TMDB synopses,
  then runs a sentence-transformer over it.
- Computing an embedding **updates no model weights**. It is a forward pass that
  produces a representation of the text — inference, not training.
- That reading is almost certainly right. "Almost certainly" is not a standard to
  build a phase on, and an unwritten assumption is worse than a wrong decision
  because nobody knows it was made.

## Decision

- **Record the reading as a decision:** using TMDB text as embedding *input* is
  inference, not training, and is permitted. It is now citable, not assumed.
- **Ask TMDB directly.** Draft enquiry at `docs/correspondence/tmdb-ai-clause.md`,
  for the author to send. A written answer settles it permanently.
- **Hedge structurally.** Phase 5's document builder takes a **swappable text source
  via config** — `tmdb` / `imdb` / `wikidata`, composable — so an unfavourable answer
  is a config change and a re-embed, not a rewrite.

## Consequences

- Phase 5 proceeds now rather than waiting on a reply.
- The swappable source is **worth having regardless**: it makes the eval harness able
  to measure how much TMDB synopses actually contribute to nDCG, which is a real
  question the project should answer anyway.
- Cost: one indirection in the document builder, and the embedding artefact must
  record which text source produced it (ADR-0014 already requires a document-builder
  version field — extend it to name the source).
- If the answer is unfavourable, the cost is a re-embed of the catalogue: hours of
  compute, no code changes.

## Alternatives Considered

- **Assume it is fine and move on.** Rejected — this is precisely the silent-drift
  failure §2.8 exists to prevent.
- **Exclude TMDB text pre-emptively.** Rejected as over-caution that degrades search
  quality against a reading that is probably wrong.
- **Wait for TMDB's reply before building Phase 5.** Rejected: blocks a phase on an
  unbounded external timeline when a config seam removes the risk.
