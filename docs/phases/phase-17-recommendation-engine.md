# Phase 17 — Recommendation Engine

**Status:** in_progress · **Depends on:** 15, 16 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Recommendations that are right, that go beyond the obvious, and that can be explained.

## Deliverables

**Layer 1 — content-based**: similarity in embedding space against each taste mode, filtered and boosted by the explicit axes from Phase 15. **Layer 2 — item-item collaborative filtering**: precompute an item-item similarity matrix offline from MovieLens (and AniList/IMDb rating signals), shipped or built during ingestion, giving genuine "people who loved this also loved" signal with no server and no other users. Handle the popularity bias that raw co-occurrence produces — normalise so that a beloved obscure film can surface above a mediocre blockbuster. **Layer 3 — the hybrid ranker**: blend both signals with candidate generation (retrieve ~1,000 candidates cheaply, rank precisely), apply diversity constraints (maximal marginal relevance so a rail isn't eight films by one director), availability weighting (something you can actually watch right now ranks higher), and freshness. **Explanations**: every recommendation carries a human-readable reason grounded in the model ("Because you finished three Béla Tarr films", "Loved by people who share your taste in 1970s political thrillers"). Cold-start handling for items with no ratings. **Staying current** (ADR-0054): on every launch, after the catalogue check, the titles it added get a synopsis (TMDB with the profile's key, AniList for anime, else Wikipedia through Wikidata) and a meaning vector; AniList's community recommendations (anime, no key) and TMDB's recommendations (films and series, the profile's key) stand beside MovieLens in the neighbour graph, marked by source and asked again when a week old; the matrix is built from MovieLens' newest release and rebuilt when GroupLens publishes a new one (ml-25m stays the fixed evaluation set); a title nobody has rated gets neighbours by meaning and by shared people the day it arrives; IMDb's daily votes give the ranker what is rising; the rails are ranked again after each step, and Settings says when each source last updated. `fixtures/recommender/` and the recommender eval harness.

## Exit criteria

- [x] **E1** Recall@20 beats a popularity baseline by > 40% relative on a held-out split — i.e. `(recall_model / recall_baseline) - 1 > 0.40`, not 40 percentage points.
- [x] **E2** Catalogue coverage > 15% — the recommender does not only ever surface the same 2,000 films.
- [x] **E3** Intra-list diversity meets the documented target; no rail is dominated by one director, franchise, or year.
- [x] **E4** Every recommendation has a truthful explanation.
- [ ] **E5** Ranking 1,000 candidates completes in < 150 ms on Tier 0.
- [ ] **E6** Case study written with real numbers.
- [ ] **E7** A title the catalogue gains at a launch can be recommended on that same launch with a truthful reason, and each source (catalogue, meaning, MovieLens, AniList, TMDB) says when it last updated.

## Subtasks

- [x] **17.1** Layer 2, item-item collaborative filtering (crates/recommend, new): built on this machine from the MovieLens ratings streamed from the archive (never stored or shipped, ADR-0019/ADR-0044), positive co-occurrence normalised against popularity (cosine with shrinkage, or lift damped by log popularity, chosen on a tuning split) so an obscure favourite can outrank a blockbuster; the top neighbours per title persisted (migration 0029, repository methods with surface lines); titles with no ratings fall back to content neighbours (cold start).
- [x] **17.2** Layer 1 and Layer 3, the hybrid ranker: about 1,000 candidates cheaply (each taste mode's nearest in embedding space, the CF neighbours of what was liked, the popularity pool), scored by a blend of content similarity, CF, the popularity prior, the axes (boosts) and avoids (penalties), availability and freshness; maximal marginal relevance with caps per director, franchise and year; ranking 1,000 candidates timed (E5, under 150 ms).
- [x] **17.3** Explanations: every recommendation carries a reason built from the signal that put it there (the liked titles behind a CF score, the mode behind a content score, the axis behind a boost), tested so that every reason names something true of the profile (E4); Home's taste rails use the ranker and show the reasons, pad-complete.
- [x] **17.4** fixtures/recommender/ and eval recommend: Recall@20 against popularity on a held-out split (E1, more than 40% relative), catalogue coverage over the users (E2, above 15%), intra-list diversity against a documented target and no rail dominated by one director, franchise or year (E3), ranking time (E5); seen to fail for each.
- [ ] **17.6** Staying current (ADR-0054), on every launch after the catalogue check: the titles it added get a synopsis (TMDB with the profile's key, AniList for anime, else Wikipedia through Wikidata) and a meaning vector inserted into the index (every tier, capped at 2,000 a launch); AniList's and TMDB's recommendations stored beside MovieLens' neighbours by source and asked again when a week old; the matrix built from MovieLens' newest release and rebuilt when its Last-Modified changes; neighbours by meaning and shared people for a title nobody has rated; IMDb's previous votes kept for a rising signal; rails re-ranked; Settings shows each source's last update (E7, tested end to end with a fixture refresh).
- [ ] **17.5** The case study (docs/case-studies/recommender.md) with real numbers (E6); docs/specs/recommender.md, learning note, README and HOW_IT_WORKS, regression, close.

## Improvements assigned (SPEC.md Appendix F)

Deliverables, not exit criteria: each is done or carried by name at close (ADR-0046).

- [ ] **I40** (M) Films and TV Shows show a simple working grid (the library first, then widely loved) until Phase 19 builds them
- [ ] **I41** (M) "Playable now": a rail of public-domain films from the built-in Internet Archive source, so a fresh install can play something at once

## Learning note

Collaborative versus content-based filtering; the item-item matrix and why it works without other users; popularity bias and how to correct it; candidate generation versus ranking; maximal marginal relevance; Recall@K and coverage.

---

<!-- subtask log: appended during the phase -->

## Plan (2026-11-18, ADR-0040)

**17.1, the item-item matrix:**
- `crates/recommend` (new, no Tauri, std only): likes from a ratings stream. MovieLens' `ratings.csv` is sorted by user,
  so one pass builds each user's list; a rating of 4.0 or more is a like. Each user is weighted 1/ln(e + likes), so one
  person with 5,000 likes does not outweigh fifty people with fifty.
- Co-occurrence computed item by item over a by-item and a by-user index, with a dense accumulator per thread
  (`std::thread::scope`, no new dependency). Items with fewer than 5 likes are left out of the matrix.
- One score family, tuned rather than picked by taste: asymmetric cosine `c_ij / (n_i^(1-a) * n_j^a)` times shrinkage
  `c_ij / (c_ij + l)`. a = 0.5 is cosine; a towards 1 is lift, which favours the obscure. a and l are chosen on tuning
  users; E1 is measured on users never used for tuning or building (a test pins that the split cannot leak).
- The top 50 neighbours per title, in catalogue ids through the `external_ids` join (61,912 MovieLens films on the dev
  copy), stored by migration 0029 `item_neighbours` with a repository whose every method has a surface line.
- Built on the user's machine: the first run's enrichment fetches `ml-25m.zip` (ADR-0019: never redistributed), joins
  it, and builds the matrix; `ingest neighbours` runs the same call for a copy built by hand. Time and memory measured.
- Cold start (a title with no ratings gets content neighbours) lands with the ranker's candidate generation in 17.2,
  where the vector index already is.

**Amended 2026-11-18 (ADR-0054, spec 4.7.0):** the app builds from MovieLens' rolling release (`ml-latest`), checked by
its validator at every launch and rebuilt when GroupLens publishes anew; ml-25m stays the evaluation set. GroupLens'
certificate fails Windows' own root store (curl: untrusted root) but passes the Mozilla roots rustls carries: the app's
downloader fetched `ml-latest-small.zip` over verified HTTPS on 2026-11-18. Subtask 17.6 and E7 carry the rest of
"current on every launch".

**17.2-17.3 so far (2026-11-18):** the ranker is pure (`crates/recommend/src/rank.rs`); the gathering is in
`crates/taste/src/hybrid.rs` (three capped pools, `MediaRepository::facts` for a thousand titles in three queries per
500, the axes as a -1..1 score); Home's taste rail uses it and its notes carry each pick's reason. Still to do: the
weights and diversity tuned on MovieLens' tuning users and E1-E3, E5 measured on the whole recommender (17.4); a look at
Home in the real window.

## Work log
- **17.1** Layer 2, item-item collaborative filtering (crates/recommend, new): built on this machine from t · `9721428`
- **17.2** Layer 1 and Layer 3, the hybrid ranker: about 1,000 candidates cheaply (each taste mode's neares · `c183f27`
- **17.4** fixtures/recommender/ and eval recommend: Recall@20 against popularity on a held-out split (E1,  · `c183f27`
- **17.3** Explanations: every recommendation carries a reason built from the signal that put it there (the · `1cad203`
