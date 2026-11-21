# Phase 18 — Discovery Engine

**Status:** complete · **Depends on:** 17 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The thing that makes this app worth using: recommendations that are *surprising and right*.

## Deliverables

**Surprise me** (ADR-0043, ADR-0056): one button, a text button in the top bar just left of Search, and a pad shortcut (RT), that picks a film the viewer will probably like, from their taste and their sources, and opens it ready to play. **Ratings for any title** (ADR-0056, the author's answers): Letterboxd-style half stars, 0.5 to 5, no reviews; a show rated season by season, the show showing the average of the seasons rated ("your average"); stars and watching both count in the taste model; rated from the title page (a row of stars under the title and year; beside each season's header for a show), any card's menu, the end card and a rate-a-batch screen (opened from the taste page and from a Home card while a profile has fewer than ten ratings); filled stars oxblood. **Continue watching** (ADR-0056): any title can be removed from it (hidden, its place kept, back only if played again) from the card's menu, an × on hover and the title page; the line under the row is gone. **Mood shelves** (ADR-0043): rows by mood ("Slow and beautiful", "Tense, under 100 minutes") built from the semantic index, not genres, offered to Home's rails. A **contextual bandit** over recommendation strategies. Arms include: safe-familiar, adjacent-stretch (one axis away from a taste mode), cross-mode bridge (something sitting between two of the user's clusters), blind-spot (a well-regarded region of the catalogue the user has never touched — a country, a decade, a movement), canon-gap (acclaimed work the user's taste predicts they'd like but hasn't seen), and deep-cut (highly-rated obscurity matching a taste mode). Reward signal from actual outcomes: played, completed, abandoned-early, watchlisted, dismissed. **The exploration budget**: a user-facing "Comfort ↔ Adventure" slider setting the baseline, adapted by the bandit based on how the user actually responds — finish the difficult ones and it pushes further; bail repeatedly and it eases off. Rails are labelled with honest intent, and the labels are part of the product: *"Because you loved Stalker"*, *"A stretch — but we think you're ready"*, *"Your blind spot: Iranian New Wave"*, *"Beloved by people like you, seen by almost no one"*, *"You've never watched anything from 1974"*. A dismissal mechanism ("not for me", "not right now", "already seen") that feeds back properly. Guard against degenerate behaviour: never recommend the same item in two rails; never let one strategy dominate; enforce a minimum exploration floor so the model can't collapse into a bubble even if the user always picks the safe option. **Countries and movements** (ADR-0055): the IMDb datasets carry neither, so the hourly check asks Wikidata, by IMDb id, where each title comes from (P495) and which film movement it belongs to (P135): what profiles have met first, then every title with 1,000 votes, most voted first. A label may say "never" of a country or a movement only once every title the profile met has been asked.

## Exit criteria

- [x] **E1** The bandit demonstrably shifts its strategy distribution in simulation when reward patterns change.
- [x] **E2** The exploration floor holds: even a maximally-conservative simulated user still receives genuine discovery.
- [x] **E3** Rail labels accurately describe why each item is there.
- [x] **E4** Novelty metric (mean inverse popularity of recommendations) meets its target while relevance is maintained.
- [x] **E5** The author tests it on their own taste and finds at least three films they hadn't heard of and want to watch. This is the real acceptance test.
- [x] **E6** Case study written — this is the flagship one.

## Subtasks

- [x] **18.1** The strategy arms (crates/recommend/src/strategy.rs, new): safe-familiar, adjacent-stretch (one axis away from a taste mode), cross-mode bridge (between two of the profile's modes), blind-spot (a well-regarded region the profile never touched: a country, a decade, a movement), canon-gap (acclaimed work the taste predicts and the profile has not seen), deep-cut (highly rated obscurity near a mode); each yields candidates from Phase 17's pools and the meaning index with an honest label ('Because you loved Stalker', 'Your blind spot: Iranian New Wave'), tested so every label is true (E3).
- [x] **18.2** The contextual bandit (Thompson sampling) over the arms, rewarded by outcomes (played, completed, abandoned early, watchlisted, dismissed), with an exploration floor and no arm dominating; a simulation harness (`eval discovery`) with simulated users whose reward patterns change (E1), a maximally conservative user who still gets discovery (E2), and the novelty metric, mean inverse popularity, against its target with relevance held (E4).
- [x] **18.3** The exploration budget and dismissals: a Comfort to Adventure setting per profile adapted by the bandit; 'not for me', 'not right now', 'already seen' on every recommended card (pad-reachable) feeding back; never the same item in two rails.
- [x] **18.4** Surprise me (one button and a pad shortcut: a film the viewer will probably like, from their taste and their sources, opened ready to play) and mood shelves from the semantic index offered to Home's rails, with I42 ('I have 90 minutes'). Search-by-meaning quality, carried from Phase 5 (E3 nDCG@10 0.0472, E4 the two named queries), is worked here, where mood shelves depend on it.
- [x] **18.5** The flagship case study (E6), the author's acceptance test on their own taste (E5, asked in the author batch), docs/specs, learning note, README and HOW_IT_WORKS, regression against every recorded number, close.
- [x] **18.6** The author's fix list of 2026-11-20 (ADR-0056, spec 4.9.0): half-star ratings for any title (shows by season, the average shown; stars and watching both count) from the title page, every card's menu, the end card and a rate-a-batch screen; remove from Continue watching; Surprise me in the masthead and I42's length choice removed; the line under Continue watching removed; a cancelled download that restarted itself (the queue's race).
- [x] **18.7** The author's E5 findings of 2026-11-21: a long subtitle list that ran off the window (0d33fe9); a deleted download whose torrent stayed in the engine and was played from gone files (4f0e1c8); Surprise me rebuilding all of Home per press, 377 ms to 1.7 ms (525bb0a).

## Improvements assigned (SPEC.md Appendix F)

Deliverables, not exit criteria: each is done or carried by name at close (ADR-0046).

- [ ] **I42** (S) ~~"I have 90 minutes": a length limit on Surprise me and on Home~~ Removed by the author (ADR-0056) after it was built

## Learning note

Multi-armed and contextual bandits; the exploration/exploitation trade-off; Thompson sampling or UCB, whichever is used, explained properly; why filter bubbles are an optimisation failure, not an inevitability; how to evaluate serendipity.

---

<!-- subtask log: appended during the phase -->

## Plan (2026-11-19, ADR-0040)

**What the data allows, checked first** (a copy of the development catalogue): 652,952 films rated, 57,013 with 1,000
votes or more, 329 rated 8.0+ with 100,000 votes, 4,986 rated 7.5+ with 1,000 to 10,000 votes. **No title has a country
or an original language**: the IMDb datasets carry neither. A blind spot by country or movement ("Your blind spot:
Iranian New Wave") therefore needs a source, and the app already asks Wikidata at launch (ADR-0054's article mapping).
Decided (ADR-0055): the same launch asks Wikidata for country of origin (P495, as ISO 3166 codes into the existing
`media_items.countries`) and film movement (P135, English labels, a new table), most-voted first, 500 ids a query at
WDQS's one query in three seconds: about six minutes for every film with 1,000 votes, spread over launches.

**18.1, the arms** (`crates/recommend/src/strategy.rs`, pure, std only; gathering in `crates/taste/src/discover.rs`):
- A title as the arms see it: its closeness to each taste mode, rating, votes, year, length, genres, countries,
  movements, and the loved titles that point to it. The profile: its modes (members name them), and every region
  (decade, genre, country, movement) of what it has met, with counts.
- "Close to a mode" is defined by rank, not by a cosine guessed once: within the mode's 300 nearest titles in the meaning
  index (`close`), "within reach" within its 2,000 nearest (`reach`). The gatherer reads both cosines from the index.
- **safe-familiar**: Phase 17's rail, its reasons unchanged. **adjacent-stretch**: close to a mode and outside that
  mode on exactly one axis (decade, genre, country), "Like Stalker, but from the 1990s". **cross-mode bridge**: within
  reach of two modes at once, from the midpoint of their directions, "Between Stalker and Spirited Away".
  **blind-spot**: a decade, genre, country or movement with nothing met, its acclaimed titles (7.5+, 5,000 votes)
  nearest the taste, "You've never watched anything from the 1950s". **canon-gap**: 7.8+ with 50,000 votes, within
  reach of a mode, "Acclaimed, and close to Stalker". **deep-cut**: 7.5+ with at most 15,000 votes, close to a mode or
  loved by people who loved one of its members, "Loved by people who loved Stalker, seen by almost no one".
- A label is data (`Label`), words are made from it, and `holds(label, title, profile)` re-derives the claim from the
  facts. E3's test: on generated profiles and catalogues, every pick of every arm holds; and each arm's label seen to
  fail when a fact is changed under it.

**18.2, the bandit**: Thompson sampling, a Beta posterior per arm per profile, context the Comfort to Adventure budget;
an exploration floor (every discovery arm at least 1 in 20 slots on average, discovery at least a quarter of the
slots) and a cap (no arm above 40%). `eval discovery`: simulated users drawn from MovieLens test users, whose reward
patterns switch half way (E1), and one who rewards only safe-familiar (E2).

**E4's target, fixed before anything is measured.** Novelty is mean self-information, `-log2(p)`, `p` a title's share
of MovieLens build users who liked it (the usual form of "mean inverse popularity"; a title liked by half the users
scores 1 bit, by one in a thousand about 10). Target: the discovery rails' mean novelty is **at least 1 bit above
Phase 17's taste rail** on the same test users (titles on average half as widely liked, or less), **while relevance
holds**: Recall@20 of the slots Home shows (taste rail plus discovery) stays more than 40% above popularity, Phase 17's
E1 bar.

**18.3** dismissals (not for me, not right now, already seen) and the per-profile budget; one item per Home.
**18.4** Surprise me, mood shelves, I42, and search-by-meaning quality (Phase 5 E3, E4). **18.5** the case study, E5.

**Decisions taken 2026-11-19 (ADR-0040), and what the measurements changed:**
- Countries and movements from Wikidata (ADR-0055, spec 4.8.0, A45); "never" of a country only once every met title
  was asked. A blind-spot rail holds one region.
- The bandit: probability matching over 100 joint draws, as few rails as fit, cap half of discovery (first 40%: E1's
  first run missed its bar, +12/-6), floor 20%, ceiling 80%, half-life 60 days, three parts of the day borrowing 0.3.
- E4's novelty leaves out titles no build user liked (fixed before its first run). Each arm's picks are ordered by Phase
  17's relevance; Home shows the familiar's whole share (a second taste rail past twenty).
- Pad: X is "options for what has focus" (not for me, not right now, already seen); RT is Surprise me. "Not for me" is
  also a taste signal (as an early abandon), "already seen" makes a title met, "not right now" hides it 14 days.
- Surprise me: films only, from what Home recommends now, on this PC first, at random among the best five. I42: any
  length, under 90 minutes, under 2 hours; a title with no known length is left out under a limit.
- E4 relevance short after three approaches: blocker B17, option A (a population prior estimated on tuning users) next.

## Work log
- **18.1** The strategy arms (crates/recommend/src/strategy.rs, new): safe-familiar, adjacent-stretch (one  · `c362b80`
- **18.3** The exploration budget and dismissals: a Comfort to Adventure setting per profile adapted by the · `d7b5afa`
- **18.2** The contextual bandit (Thompson sampling) over the arms, rewarded by outcomes (played, completed · `193e4d4`
- **18.4** Surprise me (one button and a pad shortcut: a film the viewer will probably like, from their tas · `890a469`
- **18.6** The author's fix list of 2026-11-20 (ADR-0056, spec 4.9.0): half-star ratings for any title (sho · `a1816c7`
- **18.5** The flagship case study (E6), the author's acceptance test on their own taste (E5, asked in the  · `b1d59eb`
- **18.7** The author's E5 findings of 2026-11-21: a long subtitle list that ran off the window (0d33fe9);  · `525bb0a`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `677caa4`.

### Evidence per criterion

**E1** — The bandit demonstrably shifts its strategy distribution in simulation when reward patterns change.

> eval discovery --simulate on fcc4110 (1,000 simulated viewers, 120 days, favourite arm changes day 61): new favourite's share of discovery 12.8% -> 47.8% (+35.0 points, bar +15), old 49.0% -> 28.0% (-21.0, bar -15), rose for 98.9% of viewers (bar 80%). Seen to fail with --frozen (+0.1 points, NOT met). First run on 7bc4ef3 missed the bar (+12.0/-6.0) and is recorded in docs/eval-results.md 18.2.

**E2** — The exploration floor holds: even a maximally-conservative simulated user still receives genuine discovery.

> eval discovery --simulate on fcc4110: 1,000 maximally conservative viewers (Comfort, every discovery card 'not for me') for 120 days: discovery's least share on any day 20.0% (bar 20%), 0 of 120,000 days without a discovery rail, every discovery arm shown on at least 23.3% of each viewer's days (bar 10%). Seen to fail with --no-floor (0.0% on every day, NOT met). docs/eval-results.md 18.2.

**E3** — Rail labels accurately describe why each item is there.

> strategy::tests::every_label_of_every_arm_holds_on_generated_profiles (300 generated profiles, every pick's Label re-derived by strategy::holds; seen to fail with arms taking titles 0.1 short of close and with the blind spot ignoring met regions, 6784097); sinephile-taste tests/discover.rs every_label_holds_and_an_episode_counts_as_its_series; strategy::tests::a_blind_spot_rail_holds_one_region (a rail named for a region holds only it); and on real users: `eval discovery` on a1dd9a9 over 15,640 MovieLens test users checked 461,488 discovery labels (467,741 on 0e06e59) with strategy::holds, none false (docs/eval-results.md, 18.2).

**E4** — Novelty metric (mean inverse popularity of recommendations) meets its target while relevance is maintained.

> `eval discovery` on b2842d4 (the arms after the real window's fixes, popularity in the arms' order at 0.1 and the bandit's prior at 2, both chosen on tuning users by rules fixed before they ran), all 15,640 MovieLens test users: discovery 1.71 bits more novel than the taste rail (bar 1); Recall@N of everything shown 0.3630 against popularity's top N 0.2593, +40.0% (bar more than 40%): met by under 0.05 point (the unrounded ratio above 1.40, at most 1.4004), inside sampling noise and said so. The test users have judged three versions (+40.4%, +39.9%, +40.0%). Seen to fail: the same harness gave +28.8% before the prior. docs/eval-results.md 18.2 to 18.5.

**E5** — The author tests it on their own taste and finds at least three films they hadn't heard of and want to watch. This is the real acceptance test.

> manual: the author ran it on their own profile in the installed release (installer of 2026-11-21 from a1816c7, after rating films with the half stars built for it, ADR-0056) and said "E5 seems okay" (2026-11-21). They named no films; the three the criterion asks for are their judgment, not a list recorded here. docs/case-studies/discovery.md, E5.

**E6** — Case study written — this is the flagship one.

> docs/case-studies/discovery.md (the flagship case study: the problem, the five arms and the bandit, how each criterion was judged with bars fixed first, what the author's test found, what went wrong, the side effect on search, what would be done differently); every number in docs/eval-results.md, Phase 18.

### Next phase starts by

Phase 19, 19.1: make search by meaning understand negation for Phase 5 E3/E4. In crates/search-engine/src/lib.rs, split a query's negated clause ('that aren't', 'but not', 'without', 'not too': a small pure function with tests), embed it apart, and subtract a weight times each candidate's cosine to it before the popularity prior; choose the weight on fixtures/search/meaning-tuning.tsv with `eval search --tune-meaning` (rule written into docs/eval-results.md first), then judge once with `eval search --report` (bar nDCG@10 > 0.75) and capture the grief query's results.
