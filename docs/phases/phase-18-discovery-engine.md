# Phase 18 — Discovery Engine

**Status:** in_progress · **Depends on:** 17 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The thing that makes this app worth using: recommendations that are *surprising and right*.

## Deliverables

**Surprise me** (ADR-0043): one button and a pad shortcut that picks a film the viewer will probably like, from their taste and their sources, and opens it ready to play. **Mood shelves** (ADR-0043): rows by mood ("Slow and beautiful", "Tense, under 100 minutes") built from the semantic index, not genres, offered to Home's rails. A **contextual bandit** over recommendation strategies. Arms include: safe-familiar, adjacent-stretch (one axis away from a taste mode), cross-mode bridge (something sitting between two of the user's clusters), blind-spot (a well-regarded region of the catalogue the user has never touched — a country, a decade, a movement), canon-gap (acclaimed work the user's taste predicts they'd like but hasn't seen), and deep-cut (highly-rated obscurity matching a taste mode). Reward signal from actual outcomes: played, completed, abandoned-early, watchlisted, dismissed. **The exploration budget**: a user-facing "Comfort ↔ Adventure" slider setting the baseline, adapted by the bandit based on how the user actually responds — finish the difficult ones and it pushes further; bail repeatedly and it eases off. Rails are labelled with honest intent, and the labels are part of the product: *"Because you loved Stalker"*, *"A stretch — but we think you're ready"*, *"Your blind spot: Iranian New Wave"*, *"Beloved by people like you, seen by almost no one"*, *"You've never watched anything from 1974"*. A dismissal mechanism ("not for me", "not right now", "already seen") that feeds back properly. Guard against degenerate behaviour: never recommend the same item in two rails; never let one strategy dominate; enforce a minimum exploration floor so the model can't collapse into a bubble even if the user always picks the safe option.

## Exit criteria

- [ ] **E1** The bandit demonstrably shifts its strategy distribution in simulation when reward patterns change.
- [ ] **E2** The exploration floor holds: even a maximally-conservative simulated user still receives genuine discovery.
- [ ] **E3** Rail labels accurately describe why each item is there.
- [ ] **E4** Novelty metric (mean inverse popularity of recommendations) meets its target while relevance is maintained.
- [ ] **E5** The author tests it on their own taste and finds at least three films they hadn't heard of and want to watch. This is the real acceptance test.
- [ ] **E6** Case study written — this is the flagship one.

## Subtasks

- [ ] **18.1** The strategy arms (crates/recommend/src/strategy.rs, new): safe-familiar, adjacent-stretch (one axis away from a taste mode), cross-mode bridge (between two of the profile's modes), blind-spot (a well-regarded region the profile never touched: a country, a decade, a movement), canon-gap (acclaimed work the taste predicts and the profile has not seen), deep-cut (highly rated obscurity near a mode); each yields candidates from Phase 17's pools and the meaning index with an honest label ('Because you loved Stalker', 'Your blind spot: Iranian New Wave'), tested so every label is true (E3).
- [ ] **18.2** The contextual bandit (Thompson sampling) over the arms, rewarded by outcomes (played, completed, abandoned early, watchlisted, dismissed), with an exploration floor and no arm dominating; a simulation harness (`eval discovery`) with simulated users whose reward patterns change (E1), a maximally conservative user who still gets discovery (E2), and the novelty metric, mean inverse popularity, against its target with relevance held (E4).
- [ ] **18.3** The exploration budget and dismissals: a Comfort to Adventure setting per profile adapted by the bandit; 'not for me', 'not right now', 'already seen' on every recommended card (pad-reachable) feeding back; never the same item in two rails.
- [ ] **18.4** Surprise me (one button and a pad shortcut: a film the viewer will probably like, from their taste and their sources, opened ready to play) and mood shelves from the semantic index offered to Home's rails, with I42 ('I have 90 minutes'). Search-by-meaning quality, carried from Phase 5 (E3 nDCG@10 0.0472, E4 the two named queries), is worked here, where mood shelves depend on it.
- [ ] **18.5** The flagship case study (E6), the author's acceptance test on their own taste (E5, asked in the author batch), docs/specs, learning note, README and HOW_IT_WORKS, regression against every recorded number, close.

## Improvements assigned (SPEC.md Appendix F)

Deliverables, not exit criteria: each is done or carried by name at close (ADR-0046).

- [ ] **I42** (S) "I have 90 minutes": a length limit on Surprise me and on Home

## Learning note

Multi-armed and contextual bandits; the exploration/exploitation trade-off; Thompson sampling or UCB, whichever is used, explained properly; why filter bubbles are an optimisation failure, not an inevitability; how to evaluate serendipity.

---

<!-- subtask log: appended during the phase -->

## Work log

