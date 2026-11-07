# Phase 15 — Learning notes

**The app learns what each person likes from what they do: what they finish, what they walk out of, what
they rate, and when. It says it back in plain words, and they can correct it.** Four sections per
ADR-0016 (A4).

---

## 1. What we built

- **Signals** — the player records whether a film was finished or abandoned, and where (migration 0026).
- **Taste modes** — the liked films grouped into 3 to 8 separate tastes, each recommended from in
  proportion to its weight, with a pull towards films many people loved. It now beats "show the most
  popular" (40.63% against 35.64% of held-out favourites).
- **Axes** — decades, lengths, genres, directors and what you walk out of, with your stated preferences
  as a starting point.
- **Decay** — old likes fade (half-life 180 days, 720 for strong ones), so a taste that moves is followed.
- **The taste profile screen** — each taste in a sentence, its share, the leanings, and corrections.
- **Freshness** — the model rebuilds in the background within about 130 ms of a watch.

## 2. Why

- **Several tastes, not one average.** Someone who loves horror and documentaries is not "half horror":
  the average of the two sits in the middle and recommends neither. Clusters keep them apart (96.6%
  purity on a deliberately split profile; 59.3% when the vectors are shuffled, which proves the test).
- **Walking out early says more than walking out late.** Five minutes in is a verdict; eighty minutes in
  is a phone call. The weight scales with how much was watched.
- **Popularity is a strong baseline.** A personal model that ignores it loses to it. A small prior, tuned
  on users who are not the ones it is judged on, makes it win honestly.
- **Show the model so it can be trusted.** A sentence a person can disagree with ("Not me") is worth more
  than a hidden vector.
- **A correction must do something.** Mode corrections change Home; axes only describe, so they are
  corrected through what you said, not through a toggle that would change nothing.
- **Never make the UI wait for learning.** The rebuild runs after the command returns.

## 3. New concepts

- **Exponential decay by half-life** — `crates/taste/src/modes.rs:16` `HALF_LIFE`, applied in `:60` `weigh`.
- **Weighting by how far in** — `modes.rs:38` `abandon_weight`; outcome rules in `crates/taste/src/signals.rs:25` `outcome`.
- **Weighted k-means with farthest-point seeding** — `modes.rs:108` `kmeans`.
- **Choosing k by silhouette** — `modes.rs:158` `silhouette`, used in `:188` `modes`.
- **Interleaving lists by weight** — `modes.rs:254` `interleave_by_weight`.
- **A popularity prior (log votes) blended into a similarity score** — `crates/taste/src/model.rs:154` `POPULARITY_PRIOR`, `:160` `rank`.
- **Tuning on one set, judging on another** — `tools/eval/src/taste.rs` `--tune`.
- **A negative control that must fail** — `tools/eval/src/modes.rs` `--prove` (shuffled vectors).
- **A cache keyed by a fingerprint of the data** — `model.rs:94` `ModelCache::get`; the fingerprint, `crates/persistence/src/repositories/profiles.rs:313`.
- **Background work after a command returns** — `src-tauri/src/commands/home.rs:144` `refresh_model` (`tauri::async_runtime::spawn`).
- **Days from a date without a date library** — `model.rs:14` `day_of` (the civil-from-days algorithm).
- **Weighted shares and floors** — `crates/taste/src/axes.rs:33` `shares`; **priors as pseudo-counts** — `axes.rs:64` `PRIOR`.
- **Natural-language generation from data** — `crates/taste/src/describe.rs:102` `mode_sentence`, `:80` `decade_words`.
- **Corrections as multiplicative weights, renormalised** — `describe.rs:170` `apply`.
- **Landing focus deliberately on open** — `src/features/taste/TasteProfile.tsx` (`landed` ref).

## 4. Five questions

1. A profile loves twenty horror films and twenty documentaries. Why would recommending from the average
   of all forty vectors be bad, and how do the modes avoid it?
2. Someone walks out of one film at 5 minutes and another at 80. What weight does each get, and why is it
   not the same?
3. The popularity prior is 0.5. Why was it chosen on users 120, 166 and 171 and not on the three users
   `eval taste` judges on?
4. What does `ModelCache` compare to decide whether to rebuild, and what would go wrong if it never
   rebuilt? (Hint: what did `eval update` print when that was broken on purpose?)
5. Why can a mode be corrected with "Not me" on the taste screen but a leaning cannot, and where do you
   correct a leaning instead?
