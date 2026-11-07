# Phase 15 — Taste Model

**Status:** complete · **Depends on:** 5, 14 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A rich, honest representation of what a person likes, built from what they actually do.

## Deliverables

Signal collection with weights: completed (strong positive), abandoned early (strong negative, with the abandon point recorded), rewatched (very strong), watchlisted, explicitly rated, searched for, viewed detail page without playing (weak negative), time-of-day and session-length context. **Multi-vector taste representation** — not one average vector, because a person who loves both Tarkovsky and slasher films is not well described by their midpoint. Cluster the user's positive signals into taste *modes* (typically 3–8), each with its own vector, weight, and recency. Explicit axis extraction alongside the vectors: preferred eras, runtimes, countries, languages, subtitle tolerance, pacing, directors, and — importantly — *negative* preferences learned from abandons. Temporal decay so the model tracks a changing person, with a longer half-life for strongly-held preferences. A **taste profile screen** where the user can see their own model in plain language ("You gravitate toward slow, formally rigorous films from East Asia, and separately toward 1980s American genre cinema") and correct it — this is both a trust feature and a great demo.

## Exit criteria

- [x] **E1** The model produces distinct, recognisable clusters for a synthetic profile with two deliberately unrelated tastes.
- [x] **E2** Abandonment is weighted correctly — abandoning at 5 minutes and at 80 minutes mean different things.
- [ ] **E3** The taste profile screen describes a test profile in a way the author agrees is accurate. — *carried to Phase 16*
- [x] **E4** Temporal decay is tested with a synthetic profile whose taste shifts over simulated time.
- [x] **E5** The model updates within 2 s of a watch event, without blocking the UI.

## Subtasks

- [x] **15.1** Signals (migration 0026 and the player): a watch recorded when the player leaves (completed past 90%, otherwise abandoned with its point), rewatches, searches that led to a title, detail pages opened and not played (a weak negative), with the time of day and the session's length; repository methods with surface lines; the player and the detail page write them.
- [x] **15.2** Taste modes (crates/taste): the positive signals weighted (completed, rewatched, rated, watchlisted, imported) and decayed by age, with a longer half-life for strongly held ones; clustered into 3 to 8 modes (k-means over the catalogue vectors, k by silhouette), each with a vector, weight and recency; recommendations interleaved across modes in proportion to weight. Judged on eval taste against popularity (Phase 14 E2's carried half) and on a synthetic two-taste profile (E1).
- [x] **15.3** Axes and negatives: preferred eras, runtimes, countries, languages, subtitle tolerance and directors from the signals, the taste statements as priors; negative preferences from abandons (5 minutes in means less than 80, E2); a temporal-decay test on a profile whose taste shifts over simulated time (E4).
- [x] **15.4** The taste profile screen (src/features/taste/TasteProfile.tsx): the model described in plain language (modes named by what they hold, axes in words), each mode and axis correctable (more, less, not me), pad-complete; Home's taste rail follows the modes.
- [x] **15.5** The author check batch after this phase (ADR-0040): Phase 8 E5 controller, Phase 7 E7 VPN, Phase 13 review feel and D77, Phase 14 E1/E4/E6, and E3 here (the screen describes a profile accurately); then docs/specs/taste-model.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

User modelling; why a single average vector fails; clustering; implicit versus explicit signals; temporal decay and half-lives; the value of letting a user see and edit their own model.

---

<!-- subtask log: appended during the phase -->

## Work log
- **15.1** Signals (migration 0026 and the player): a watch recorded when the player leaves (completed past · `db5dfb3`
- **15.2** Taste modes (crates/taste): the positive signals weighted (completed, rewatched, rated, watchlis · `85f5cc2`
- **15.3** Axes and negatives: preferred eras, runtimes, countries, languages, subtitle tolerance and direc · `9310430`
- **15.4** The taste profile screen (src/features/taste/TasteProfile.tsx): the model described in plain lan · `86eec57`
- **15.5** The author check batch after this phase (ADR-0040): Phase 8 E5 controller, Phase 7 E7 VPN, Phase · `7563446`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `8b6c22a`.

### Evidence per criterion

**E1** — The model produces distinct, recognisable clusters for a synthetic profile with two deliberately unrelated tastes.

> eval modes (2026-11-08): a synthetic profile loving the 30 most-voted horror films and the 30 most-voted documentaries, clustered by sinephile_taste::modes on the real catalogue vectors: 3 modes, weighted purity 96.6% (target 90%), horror in one mode and documentaries in two of their own, three identical runs; seen to fail with --prove (vectors shuffled between titles: 59.3%, NOT MET). Unit test crates/taste/src/modes.rs two_unrelated_tastes_become_two_separate_modes (seen to fail with one mode forced). docs/eval-results.md 'E1: two unrelated tastes'.

**E2** — Abandonment is weighted correctly — abandoning at 5 minutes and at 80 minutes mean different things.

> 9310430: crates/taste/tests/model.rs abandoning_at_five_minutes_and_at_eighty_mean_different_things (record_viewing at 300 s and 4,800 s of a 100-minute film, read back through model::signals and weigh: -0.575 against -0.2, and the early abandon's genre first among the axes' avoids); crates/taste/src/modes.rs abandoning_at_five_minutes_means_more_than_at_eighty; seen to fail with the abandon weight made flat.

**E3** — The taste profile screen describes a test profile in a way the author agrees is accurate.

**Not met; carried to Phase 16.** It needs the author to agree the description is accurate; batched with the author checks after Phase 15, so Phase 16 receives it.

> MEASURED: the screen exists and describes real modes in sentences (86eec57; crates/taste/tests/describe.rs each_mode_is_named_by_what_it_holds; on the real catalogue, eval modes prints 'You gravitate toward horror and drama, mostly from the 2000s and 2010s (The Silence of the Lambs, The Shining, Alien)'; pad-complete in tools/uiaudit/controller.mjs). Whether it is accurate is the author's judgement.

**E4** — Temporal decay is tested with a synthetic profile whose taste shifts over simulated time.

> 9310430: crates/taste/tests/model.rs a_taste_that_moves_is_followed_and_the_old_one_fades (a synthetic profile: twelve horror films completed two years ago, twelve documentaries last month; the strongest mode is the documentaries with over 70% of the weight, and with the dates swapped it is the horror); crates/taste/src/modes.rs signals_fade_with_age_and_strong_ones_fade_slower; seen to fail with decay switched off.

**E5** — The model updates within 2 s of a watch event, without blocking the UI.

> 0bfada4: cargo run --release -p eval -- update (2026-11-08): a 650-film profile (MovieLens user 3) watches five unmet well-known films; from each viewing's write to the rebuilt model through ModelCache: 125.7-133.2 ms (target under 2,000 ms), and every watched film is a member of the model it timed; cached with nothing new 0.3 ms. Not blocking the UI: end_viewing and rate_title return once the signal is written and the rebuild runs in a background task (src-tauri/src/commands/home.rs refresh_model). Seen to fail: ModelCache::get made to ignore the fingerprint gives 'every watch in its model: false', E5 NOT MET. docs/eval-results.md.

### Debt incurred

- **D84** The offline catalogue has no countries or original languages (IMDb's free datasets do not carry them; the columns are empty), so the taste model cannot learn country or language preferences, or subtitle tolerance, from behaviour; the statements stand in. Fill them from TMDB for profiles with a key (and Wikidata, which is keyless) and add the axes in crates/taste/src/axes.rs.

### Next phase starts by

Phase 16 subtask 16.1 (docs/phases/phase-16-recommendation-engine.md): create crates/recommend with item-item CF built from the local MovieLens ratings in data/datasets/ml-25m.zip (as tools/eval/src/taste.rs reads them): positive pairs (4 stars and up) per user, co-occurrence normalised against popularity with the normaliser chosen on a tuning split, top 50 neighbours per title stored by a migration 0027 table item_neighbours (crates/persistence, surface lines), built from tools/ingest; cold start falls back to content neighbours.
