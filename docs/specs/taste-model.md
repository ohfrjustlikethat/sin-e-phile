# The taste model (Phase 15)

What a profile likes, learned from what it does, said back in plain words, and correctable.
Everything here is in `crates/taste`; `src-tauri` only calls it (ADR-0022).

## 1. Signals

Written per profile (migration 0026):

| Signal | Where it is written | What it means |
|---|---|---|
| Completed | the player leaving past 90%, or within 300 s of the end | strong yes |
| Abandoned at a point | the player leaving after 60 s and before that | a no, weaker the later it is |
| Skimmed (under 60 s played) | not written | nothing |
| Rated 1–5 | the end card | 5 = strong yes, 1 = strong no |
| Watchlisted | the watchlist | a mild yes (0.4), never decayed |
| Opened from search, opened and not played | `signals` table | recorded; not yet weighed |

`signals::outcome` (crates/taste/src/signals.rs) decides completed / abandoned / skimmed from seconds
played, the stop point and the length.

## 2. Weighing (`modes::weigh`)

Each signal has a base weight. Abandoning weighs `-0.6 + 0.5 × share watched`, so walking out at five
minutes of a 100-minute film is about -0.58 and at eighty minutes -0.2 (E2). Every weight decays by
its age, with a half-life of 180 days, or 720 days for strongly held ones (five stars, a completed
rewatch). A title's weights sum.

## 3. Modes (`modes::modes`)

The titles with positive weight and a catalogue vector are clustered with weighted k-means (cosine, seeded
farthest-point from the strongest title, so a run repeats). k runs 3 to 8 and is chosen by silhouette over
at most 160 sampled points. Each mode has a unit direction, a weight (its share of the positive weight) and
its members, strongest first. Under three titles there is one mode.

"What you love" in the profile's own words is embedded and joins as a mode of its own, weighted 0.2
(`model::add_words`).

## 4. Recommending (`model::recommend_with_prior`)

For each mode, its nearest unseen titles; each candidate scored by similarity plus `0.5 ×` a popularity
prior (log votes, normalised over the 3,000 most-voted indexed titles); the lists interleaved in
proportion to mode weight. The prior was tuned on three users and judged on three others (`eval taste`):
40.63% of held-out favourites recovered against 35.64% for popularity alone.

## 5. Axes (`axes::axes`)

Decades, length bands (under 90, 90–129, 130+ minutes), genres and directors as weighted shares of the
liked titles; avoids from the disliked ones. The taste statements are priors worth three titles' weight:
"older films" adds to the decades before 1980, "short" to the short band. Country, original language and
subtitle tolerance need data the offline catalogue does not have (D84).

## 6. Describing (`describe`)

A mode is said from what it holds, reading its first 40 members:

- genres on at least a fifth of its titles (at most two), as words ("science fiction", not "Sci-Fi");
- decades with at least a quarter ("the 1970s and 80s", but "the 2000s and 2010s");
- a director with at least 15% of its titles ("Ann Director above all");
- its three most-voted members.

The strongest mode is "You gravitate toward …", the others "You also love …". The axes become sentences
only when clear: a length band with half the weight, the two strongest avoids.

## 7. Corrections

Per mode: more of this (×2), less (×0.5), not me (removed); weights renormalised. Stored per profile under
the credential key `taste.corrections` as JSON keyed by the mode's anchor (its strongest member, which stays
while the mode does). Applied on Home's taste rail (`home::rails`). Axes are corrected through the
statements, their prior: they describe and do not weigh recommendations, so a toggle on an axis would
change nothing.

## 8. Freshness (E5)

`ModelCache` keeps each profile's modes until its `signal_fingerprint` (counts and maxima of its viewings,
ratings and watchlist) changes. Every profile's model is built in the background at launch
(`warm_models`), and again in a background task after a watch or a rating (`refresh_model`): 126–133 ms
for a 650-film profile (`eval update`), so the commands never wait and Home is current within 2 s.

## 9. Harnesses

`eval modes [--prove]` (E1, two unrelated tastes, purity 96.6%; shuffled vectors 59.3%), `eval taste
[--tune]` (against popularity), `eval switch` (switching with the model), `eval update` (E5).
`crates/taste/tests/model.rs` holds E2 (abandon points) and E4 (decay).
