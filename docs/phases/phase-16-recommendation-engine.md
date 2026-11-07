# Phase 16 — Recommendation Engine

**Status:** in_progress · **Depends on:** 15 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Recommendations that are right, that go beyond the obvious, and that can be explained.

## Deliverables

**Layer 1 — content-based**: similarity in embedding space against each taste mode, filtered and boosted by the explicit axes from Phase 15. **Layer 2 — item-item collaborative filtering**: precompute an item-item similarity matrix offline from MovieLens (and AniList/IMDb rating signals), shipped or built during ingestion, giving genuine "people who loved this also loved" signal with no server and no other users. Handle the popularity bias that raw co-occurrence produces — normalise so that a beloved obscure film can surface above a mediocre blockbuster. **Layer 3 — the hybrid ranker**: blend both signals with candidate generation (retrieve ~1,000 candidates cheaply, rank precisely), apply diversity constraints (maximal marginal relevance so a rail isn't eight films by one director), availability weighting (something you can actually watch right now ranks higher), and freshness. **Explanations**: every recommendation carries a human-readable reason grounded in the model ("Because you finished three Béla Tarr films", "Loved by people who share your taste in 1970s political thrillers"). Cold-start handling for items with no ratings. `fixtures/recommender/` and the recommender eval harness.

## Exit criteria

- [ ] **E1** Recall@20 beats a popularity baseline by > 40% relative on a held-out split — i.e. `(recall_model / recall_baseline) - 1 > 0.40`, not 40 percentage points.
- [ ] **E2** Catalogue coverage > 15% — the recommender does not only ever surface the same 2,000 films.
- [ ] **E3** Intra-list diversity meets the documented target; no rail is dominated by one director, franchise, or year.
- [ ] **E4** Every recommendation has a truthful explanation.
- [ ] **E5** Ranking 1,000 candidates completes in < 150 ms on Tier 0.
- [ ] **E6** Case study written with real numbers.

## Subtasks

- [ ] **16.1** Layer 2, item-item collaborative filtering (crates/recommend, new): built during ingestion from the local MovieLens ratings (never shipped, ADR-0044), positive co-occurrence normalised against popularity (cosine with shrinkage, or lift damped by log popularity, chosen on a tuning split) so an obscure favourite can outrank a blockbuster; the top neighbours per title persisted (migration 0027, repository methods with surface lines); titles with no ratings fall back to content neighbours (cold start).
- [ ] **16.2** Layer 3, the hybrid ranker: about 1,000 candidates cheaply (each mode's nearest, the CF neighbours of what was liked, the popularity pool), scored by a blend of content similarity, CF, the popularity prior, the axes (boosts) and avoids (penalties), availability and freshness; maximal marginal relevance with caps per director, franchise and year; ranking 1,000 candidates timed (E5, under 150 ms).
- [ ] **16.3** Explanations: every recommendation carries a reason built from the signal that put it there (the liked titles behind a CF score, the mode behind a content score, the axis behind a boost), tested so that every reason names something true of the profile (E4); Home's taste rail uses the ranker and shows the reasons, pad-complete.
- [ ] **16.4** fixtures/recommender/ and eval recommend: Recall@20 against popularity on a held-out split (E1, more than 40% relative), catalogue coverage over the users (E2, above 15%), intra-list diversity against a documented target and no rail dominated by one director, franchise or year (E3), ranking time (E5); seen to fail for each.
- [ ] **16.5** The case study (docs/case-studies/recommender.md) with real numbers (E6); the received author checks (Phase 7 E7, Phase 8 E5, Phase 14 E1/E4/E6, Phase 15 E3) recorded as the author reports them; docs/specs/recommender.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

Collaborative versus content-based filtering; the item-item matrix and why it works without other users; popularity bias and how to correct it; candidate generation versus ranking; maximal marginal relevance; Recall@K and coverage.

---

<!-- subtask log: appended during the phase -->

## Work log

