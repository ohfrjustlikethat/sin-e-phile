# The recommender (Phase 17)

What Home's "Where your taste points" is made of, how it is judged, and how it stays current. Numbers live in
`docs/eval-results.md` ("Phase 17"); this says what they mean.

## 1. Three layers

- **Layer 1, meaning** (Phase 15): a profile's taste as a few modes, each a direction in the meaning index; a title's
  closeness to its nearest mode.
- **Layer 2, people who loved the same** (`crates/recommend/src/cf.rs`): each title's 50 best neighbours by who loved
  both, built on this machine from MovieLens' ratings (ADR-0019: downloaded by the user, never shipped). A like is 4
  stars or more; each person is weighted `1 / ln(e + their likes)`; the score is
  `c / (mass_i^(1-alpha) * mass_j^alpha)` with alpha 0.65, chosen on tuning users as the best recall whose coverage
  clears 15% (cosine, 0.5, found slightly more and reached 10% of the catalogue).
- **Layer 3, the ranker** (`crates/recommend/src/rank.rs`): about a thousand candidates gathered by
  `crates/taste/src/hybrid.rs` from three capped pools (each mode's nearest, the matrix neighbours of the 60 best-loved
  titles, the 100 most-voted), one score, a rail picked for variety within caps, a reason for every pick.

## 2. The score

Each part is scaled to 0..1 over the candidates, then weighted (`rank::WEIGHTS`): closeness to the nearest mode, the
summed matrix scores of the loved titles pointing here, popularity (log votes), the profile's axes (-1..1: shares of a
title's decade, length, best genre and best director among what was liked, less its strongest avoided genre or
director), and a little for what is in the library. The weights and the variety setting are chosen on MovieLens'
tuning users (`eval recommend --whole --tune`), never on the users the reported numbers come from.

## 3. Variety, and its target (E3)

- **Caps**, kept by construction: at most two titles by one director, one per franchise, three from one year, in a
  rail. A franchise is read from the title (`rank::franchise_key`: "Toy Story 3" and "Toy Story" are one), a proxy,
  because no free keyless source names franchises; a false match costs a title its place, never a wrong reason.
- **Maximal marginal relevance**: each pick is the best remaining score less how closely it repeats the most similar
  pick so far (cosine in the meaning index), weighted by `rank::DIVERSITY`.
- **The target**: intra-list diversity (the mean of 1 - cosine over a rail's pairs) at least that of the same
  candidates ranked by score alone, and no rail over a cap. Both are measured over every judged user by
  `eval recommend --whole`.

## 4. Reasons (E4)

A pick's reason is the part that added most to its score, so it is true by construction, and it names only what is
true of the profile: "Loved by people who loved Stalker and Solaris" (the loved titles whose neighbours include it,
strongest first, at most three), "Close to Stalker and Solaris, which you loved" (the two strongest loved titles of the
mode it is nearest), or "Widely loved". `rank::tests::every_reason_names_something_true_of_the_candidate` checks it as a
property over 300 mixed candidates; `crates/taste/tests/hybrid.rs` end to end.

## 5. How it is judged (E1, E2, E5)

MovieLens users are split by a hash of their id (`crates/recommend/src/split.rs`): 80% build the matrix, 10% tune,
10% judge; a fifth of each judged user's likes is held out by hash. The judged user's shown likes become a profile (liked
titles, modes from their vectors, axes from the catalogue); the held-out likes are the answer. Recall@20 is
`hits / min(20, held out)`, averaged; popularity gets the same 20 slots with the build users' most-liked titles not
shown. Coverage is the share of the matrix's titles that reach at least one judged user's rail. The ranking time is
the ranker alone over each user's real candidates.

## 6. Staying current (ADR-0054)

At every launch, after the catalogue check, `bootstrap::run` asks GroupLens whether its rolling release changed (one
HEAD); when it has, the matrix is fetched, joined, rebuilt and stored, and the archive deleted. MovieLens' rolling
release ends in 2023, so subtask 17.6 adds what keeps the rest current: meaning vectors for new titles, AniList's and
TMDB's live recommendations beside the matrix, neighbours by meaning and people for a title nobody has rated, a rising
signal from IMDb's votes, and each source's last update in Settings.
