# 0019 — Compute the MovieLens item-item matrix on the user's machine

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 0 (binds Phases 4, 16)
- **Resolves:** P3 · **Risk:** R11 · **Relates to:** ADR-0014

## Context

- GroupLens states it **does not generally permit public redistribution** of the
  MovieLens datasets.
- Phase 16 needs an item-item similarity matrix derived from those ratings.
- Whether a derived aggregate counts as redistribution is genuinely arguable. The
  project does not need to win that argument.

## Decision

**Never ship a derived matrix.** Ingestion downloads MovieLens on the user's machine
and computes the item-item matrix locally. The redistribution question never arises.

**Deliberately a different answer from §8's shipped embeddings** (ADR-0014), and the
distinction is the point: embeddings are computed from metadata this project
assembles, so they are ours to publish. A matrix derived from GroupLens' ratings is
not clearly ours.

## Consequences

- No licence question, no permission needed, no term that can change under us.
- **Cost lands on first run**, and lands hardest on Tier 0 — exactly the hardware
  §2.3's budgets protect. Phase 4 must measure this before committing to a dataset
  size; **R4's trigger (ingestion > 2 hours) covers it.**
- Mitigations available if it is too slow: use a smaller MovieLens set on Tier 0,
  compute in the background while the app is usable (Phase 4 already requires this
  shape for ingestion), and cache the result so it is a one-time cost.
- The computation becomes part of the shipped codebase rather than an offline script
  — which is better for the portfolio, since the CF derivation is visible in the
  repository rather than hidden in a build step.

## Alternatives Considered

- **Ship the derived matrix.** Rejected: needs an argument about what "derived"
  means, and the downside is a takedown request against the project's own
  distribution channel.
- **Ask GroupLens.** Reasonable, and rejected as unnecessary — the on-device answer
  removes the question entirely and costs only first-run time.
- **Drop collaborative filtering.** Rejected: it is Layer 2 of Phase 16 and supplies
  signal no content-based method can.
- **Substitute another ratings dataset.** Rejected for now; MovieLens is the standard
  benchmark and its licence permits exactly this use. Worth revisiting only if
  Phase 4 measurement shows the compute cost is unacceptable.
