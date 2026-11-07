# Phase 15 — Taste Model

**Status:** in_progress · **Depends on:** 5, 14 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A rich, honest representation of what a person likes, built from what they actually do.

## Deliverables

Signal collection with weights: completed (strong positive), abandoned early (strong negative, with the abandon point recorded), rewatched (very strong), watchlisted, explicitly rated, searched for, viewed detail page without playing (weak negative), time-of-day and session-length context. **Multi-vector taste representation** — not one average vector, because a person who loves both Tarkovsky and slasher films is not well described by their midpoint. Cluster the user's positive signals into taste *modes* (typically 3–8), each with its own vector, weight, and recency. Explicit axis extraction alongside the vectors: preferred eras, runtimes, countries, languages, subtitle tolerance, pacing, directors, and — importantly — *negative* preferences learned from abandons. Temporal decay so the model tracks a changing person, with a longer half-life for strongly-held preferences. A **taste profile screen** where the user can see their own model in plain language ("You gravitate toward slow, formally rigorous films from East Asia, and separately toward 1980s American genre cinema") and correct it — this is both a trust feature and a great demo.

## Exit criteria

- [x] **E1** The model produces distinct, recognisable clusters for a synthetic profile with two deliberately unrelated tastes.
- [x] **E2** Abandonment is weighted correctly — abandoning at 5 minutes and at 80 minutes mean different things.
- [ ] **E3** The taste profile screen describes a test profile in a way the author agrees is accurate.
- [x] **E4** Temporal decay is tested with a synthetic profile whose taste shifts over simulated time.
- [x] **E5** The model updates within 2 s of a watch event, without blocking the UI.

## Subtasks

- [x] **15.1** Signals (migration 0026 and the player): a watch recorded when the player leaves (completed past 90%, otherwise abandoned with its point), rewatches, searches that led to a title, detail pages opened and not played (a weak negative), with the time of day and the session's length; repository methods with surface lines; the player and the detail page write them.
- [x] **15.2** Taste modes (crates/taste): the positive signals weighted (completed, rewatched, rated, watchlisted, imported) and decayed by age, with a longer half-life for strongly held ones; clustered into 3 to 8 modes (k-means over the catalogue vectors, k by silhouette), each with a vector, weight and recency; recommendations interleaved across modes in proportion to weight. Judged on eval taste against popularity (Phase 14 E2's carried half) and on a synthetic two-taste profile (E1).
- [x] **15.3** Axes and negatives: preferred eras, runtimes, countries, languages, subtitle tolerance and directors from the signals, the taste statements as priors; negative preferences from abandons (5 minutes in means less than 80, E2); a temporal-decay test on a profile whose taste shifts over simulated time (E4).
- [x] **15.4** The taste profile screen (src/features/taste/TasteProfile.tsx): the model described in plain language (modes named by what they hold, axes in words), each mode and axis correctable (more, less, not me), pad-complete; Home's taste rail follows the modes.
- [ ] **15.5** The author check batch after this phase (ADR-0040): Phase 8 E5 controller, Phase 7 E7 VPN, Phase 13 review feel and D77, Phase 14 E1/E4/E6, and E3 here (the screen describes a profile accurately); then docs/specs/taste-model.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

User modelling; why a single average vector fails; clustering; implicit versus explicit signals; temporal decay and half-lives; the value of letting a user see and edit their own model.

---

<!-- subtask log: appended during the phase -->

## Work log
- **15.1** Signals (migration 0026 and the player): a watch recorded when the player leaves (completed past · `db5dfb3`
- **15.2** Taste modes (crates/taste): the positive signals weighted (completed, rewatched, rated, watchlis · `85f5cc2`
- **15.3** Axes and negatives: preferred eras, runtimes, countries, languages, subtitle tolerance and direc · `9310430`
- **15.4** The taste profile screen (src/features/taste/TasteProfile.tsx): the model described in plain lan · `86eec57`
