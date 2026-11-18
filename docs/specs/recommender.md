# The recommender (Phase 17)

What Home's "Where your taste points" is made of, how it is judged, and how it stays current. Numbers live in
`docs/eval-results.md` ("Phase 17"); this says what they mean.

## 1. The layers

- **Layer 1, meaning** (Phase 15): a profile's taste as a few modes, each a direction in the meaning index; a title's
  closeness to its nearest mode.
- **Layer 2, people who loved the same** (`crates/recommend/src/cf.rs`): each title's 50 best neighbours by who loved
  both, built on this machine from MovieLens' ratings (ADR-0019: downloaded by the user, never shipped). A like is 4
  stars or more; each person is weighted `1 / ln(e + their likes)`; the score is
  `c / (mass_i^(1-alpha) * mass_j^alpha)` with alpha 0.65, chosen on tuning users as the best recall whose coverage
  clears 15% (cosine, 0.5, found slightly more and reached 10% of the catalogue). Beside it in the same graph, marked by
  source (migration 0031): AniList's community recommendations and TMDB's (section 6).
- **Made by the same people** (ADR-0054 step 5): the newest work of whoever directed or created the profile's
  best-loved titles (`MediaRepository::made_alike`), which reaches a title the day it arrives.
- **The ranker** (`crates/recommend/src/rank.rs`): about a thousand candidates gathered by `crates/taste/src/hybrid.rs`
  from four capped pools (each mode's nearest, the neighbours of the 500 best-loved titles, the makers' newest work,
  the 100 most-voted), one score, a rail picked for variety within caps and calibrated to the profile's modes, a reason
  for every pick.

## 2. The score

Each part is scaled to 0..1 over the candidates, then weighted (`rank::WEIGHTS`): closeness to the nearest mode, the
summed neighbour scores of the loved titles pointing here (a pair two sources agree on counted once, at its best), how
many loved titles share a maker with it, popularity (log votes), the profile's axes (-1..1: shares of a
title's decade, length, best genre and best director among what was liked, less its strongest avoided genre or
director), and a little for what is in the library. The weights and the variety setting are chosen on MovieLens'
tuning users (`eval recommend --whole --tune`), never on the users the reported numbers come from.

## 3. Variety, and its target (E3)

- **Caps**, kept by construction: at most two titles by one director, two per franchise, five from one year, in a
  rail of twenty, so none of them takes more than a quarter of it ("dominated", E3). One per franchise and three per
  year cost a sixth of the recall on the tuning users and were loosened. A franchise is read from the title (`rank::franchise_key`: "Toy Story 3" and "Toy Story" are one), a proxy,
  because no free keyless source names franchises; a false match costs a title its place, never a wrong reason.
- **Maximal marginal relevance**: each pick is the best remaining score less how closely it repeats the most similar
  pick so far (cosine in the meaning index), weighted by `rank::DIVERSITY`.
- **Calibration** (`rank::quotas`): each taste mode may fill at most its share of the rail (largest remainder, every
  mode at least one slot), so a rail mixes as the profile does (Steck, "Calibrated Recommendations", 2018). The loved
  part sums over every loved title, so without it a larger taste outscores a smaller one title for title: a profile of
  seven films and two anime got twenty films. It costs recall on MovieLens' held-out likes (+72.7% to +61.4% against
  popularity), which follow the larger taste, and widens coverage from 16.6% to 43.5%. Miscalibration (KL between the
  profile's mix and the rail's, Steck's measure) is reported by `eval recommend --whole`.
- **The target**: intra-list diversity (the mean of 1 - cosine over a rail's pairs) at least that of the same
  candidates ranked by score alone, and no rail over a cap. Both are measured over every judged user by
  `eval recommend --whole`.

## 4. Reasons (E4)

A pick's reason is the part that lifts it furthest above a typical candidate: each part of its score minus that
part's median over the candidates, weighted as in the score. That is the part that ranked it, since a score only
matters against the others. (The biggest part outright was the first rule: closeness is high for every candidate the
meaning pool brings, so it named a mode for twenty picks of twenty in the real window, Pickpocket "close to The
Godfather" among them; `rank::tests::a_reason_is_what_lifts_a_title_above_the_typical_candidate`.) "Widely loved" is
said only of a title more widely seen than most candidates. A reason names only what is true of the profile: "Loved by people who loved Stalker and Solaris" (the loved titles whose neighbours include it,
strongest first, at most three), "Close to Stalker and Solaris, which you loved" (the two strongest loved titles of the
mode it is nearest), "By Andrei Tarkovsky, who made Stalker and Solaris" (the maker behind most of the loved titles it
shares one with, and two of them), or "Widely loved". `rank::tests::every_reason_names_something_true_of_the_candidate` checks it as a
property over 300 mixed candidates; `crates/taste/tests/hybrid.rs` end to end.

## 5. How it is judged (E1, E2, E5)

MovieLens users are split by a hash of their id (`crates/recommend/src/split.rs`): 80% build the matrix, 10% tune,
10% judge; a fifth of each judged user's likes is held out by hash. The judged user's shown likes become a profile (liked
titles, modes from their vectors, axes from the catalogue); the held-out likes are the answer. Recall@20 is
`hits / min(20, held out)`, averaged; popularity gets the same 20 slots with the build users' most-liked titles not
shown. Coverage is the share of the matrix's titles that reach at least one judged user's rail. The ranking time is
the ranker alone over each user's real candidates.

## 6. Staying current (ADR-0054)

Nothing the recommender reads is a snapshot that never moves:

- **MovieLens**: at every launch `bootstrap::run` asks GroupLens whether its rolling release changed (one HEAD); when it
  has, the matrix is fetched, joined, rebuilt and stored, and the archive deleted. The release ends in 2023.
- **Meaning for titles without it** (`crates/catalogue/src/meaning.rs`, A44): after each launch's check, up to 2,000 core
  titles the published meaning file does not hold, most-voted first, get a synopsis (TMDB with a profile's key, AniList
  for anime, else the Wikipedia lead found through Wikidata, or the article's opening when the lead is short) and a
  vector in a companion graph beside the published one (`sinephile_vector_index::Companion`); search and the taste rail
  open both. A title nobody could describe is asked again after a week (last year's) or 90 days.
- **Live neighbours** (`crates/catalogue/src/live.rs`): each profile's liked titles are asked of AniList (anime, no
  key) and TMDB (a profile's own key) once a week, hourly with the catalogue check; a live list's k-th row scores
  MovieLens' mean at rank k, so neither scale outweighs the other.
- **A title nobody has rated** reaches a rail through its meaning (once described) and its makers (section 1).
- **Rising now**: each ratings refresh keeps a changed count's predecessor (migration 0032); the 200 biggest gains are
  kept for Home's "Rising now" rail.
- **Settings** lists each source, when it last updated and what it did (`crates/catalogue/src/current.rs`).

E7 is tested end to end in `crates/catalogue/tests/staying_current.rs`: a fixture refresh adds a film, the meaning job
gives it a vector, and the taste rail reaches it with a true reason on the same launch.
