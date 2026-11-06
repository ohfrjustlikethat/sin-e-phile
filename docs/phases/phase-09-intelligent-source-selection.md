# Phase 9 — Intelligent Source Selection

**Status:** complete · **Depends on:** 8 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The user picks a language and a quality. The app does everything else, invisibly and well.

## Deliverables

A scoring function over `SourceCandidate`s combining: swarm health (seeders, peers, measured throughput), size-versus-quality plausibility (catching fakes and mislabels), release-group and encoding reputation heuristics, codec and HDR compatibility with the user's hardware, audio-track languages present, subtitle presence, and local availability (a local file always wins). Fake and mislabel detection: a 700 MB "4K" file is a lie; a 60-minute file for a 140-minute film is a lie. Language selection as a **first-class UI concept** — a Netflix-style language dropdown listing available audio languages, which resolves to whichever candidate actually carries that audio track, with dub/sub distinction for anime. Quality selection the same way, defaulting to the best the hardware and connection support. Automatic fallback: if the top candidate fails to start within a timeout, silently try the next, and log why. **The TV focus language** (ADR-0042), built here because every later screen inherits it and this phase's language and quality pickers are its first users: the white ring with its halo, grow, lift, stick tilt, sheen, press, the gliding ring, TV mode's 125% scale and safe margin, the button-hints footer, navigation sounds, rumble, and View for full screen, all in `src/design-system/focus/`. The hidden expert panel (`Ctrl+Shift+S` and a Settings toggle) showing the full ranked candidate list with every score component — this is both a debugging tool and a compelling demo. A "why this source" one-liner under the play button.

## Exit criteria

- [x] **E1** Scoring is a pure function with unit and property tests over a fixture set of candidate lists.
- [x] **E2** Selecting "Hindi" plays a source with a Hindi audio track, or clearly reports that none is available.
- [x] **E3** Selecting 4K plays genuine 4K, or explains why it can't.
- [x] **E4** A deliberately-failing top candidate falls back within 10 s with no user action.
- [x] **E5** The expert panel's score breakdown is legible and matches the code.
- [x] **E6** `docs/specs/source-scoring.md` documents every term in the scoring function and its weight, with justification.
- [x] **E7** The TV focus language (ADR-0042) is in the design system, and `tools/uiaudit` proves it in TV mode: the white ring is visible on every focus stop, the ring glides rather than jumps, the 125% scale and safe margin hold at 1920×1080, and under reduced motion no transform remains.

## Subtasks

- [x] **9.0** The TV focus language (ADR-0042) in src/design-system/focus/: the white ring with hairline and oxblood halo as one gliding element (critically damped spring ~200 ms), grow 1.06 and lift on tiles, left-stick tilt up to 6 degrees with a following sheen, A press-down 0.97x, TV mode with a pad (125% scale, 5% safe margin, button-hints footer, Web Audio ticks, ~20 ms rumble on select, View toggles full screen and picking up the pad enters it), settings for sounds, rumble and auto full screen, reduced motion honoured; extended tools/uiaudit/controller.mjs proves it.
- [x] **9.1** crates/selection: score(candidate, context) -> Score{total, terms[]} as a pure function over SourceCandidate plus a Context (title runtime, hardware tier and decode caps, preferred audio language, wanted quality, measured throughput). Terms: local availability, swarm health, size-versus-quality plausibility, encoding signals, codec/HDR compatibility, audio language, subtitles. Unit tests over a fixture set of candidate lists and property tests (ordering invariants).
- [x] **9.2** Fake and mislabel detection: bytes-per-minute against the claimed resolution and codec, and duration against the catalogue runtime; a 700 MB '4K' or a 60-minute file for a 140-minute film is flagged and sunk, with the reason kept. 'Release-group reputation' is implemented as structural signals only (naming consistency, encode tags, plausibility); no release-group or indexer names appear anywhere in the repo (SPEC.md 2.1).
- [x] **9.3** Language and quality as first-class choices: the detail page lists the audio languages the candidates actually carry and the qualities on offer (defaulting to the best the hardware and connection support), with dub/sub distinction for anime; choosing one re-plans to a candidate that carries it, or says plainly that none does.
- [x] **9.4** Automatic fallback: the plan returns the ranked list; if the chosen candidate has no first frame within a start timeout, the player silently opens the next one and logs why, within 10 s of Play in total for a failing first choice.
- [x] **9.5** The expert panel (Ctrl+Shift+S, and a Settings toggle): every candidate ranked with each score term and its weight, matching the code; and a one-line 'why this source' under Play.
- [x] **9.6** docs/specs/source-scoring.md: every term, its weight and its justification; learning note; evidence for E1-E6 including a deliberately failing top candidate in a test.

## Learning note

Designing a scoring function; why weights need justification and testing; the fake-detection heuristics; how to build an abstraction that hides complexity without hiding failure.

---

<!-- subtask log: appended during the phase -->

## Work log
- **9.0** The TV focus language (ADR-0042) in src/design-system/focus/: the white ring with hairline and o · `1be4948`
- **9.1** crates/selection: score(candidate, context) -> Score{total, terms[]} as a pure function over Sou · `0db6420`
- **9.2** Fake and mislabel detection: bytes-per-minute against the claimed resolution and codec, and dura · `0db6420`
- **9.3** Language and quality as first-class choices: the detail page lists the audio languages the candi · `92d4d71`
- **9.4** Automatic fallback: the plan returns the ranked list; if the chosen candidate has no first frame · `0f18990`
- **9.5** The expert panel (Ctrl+Shift+S, and a Settings toggle): every candidate ranked with each score t · `5b50da9`
- **9.6** docs/specs/source-scoring.md: every term, its weight and its justification; learning note; evide · `91595da`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `63d00d9`.

### Evidence per criterion

**E1** — Scoring is a pure function with unit and property tests over a fixture set of candidate lists.

> 0db6420: crates/selection score() is pure (no clock, disk or network; depends only on source-protocol). cargo test -p sinephile-selection: tests/fixtures.rs 18 fixture lists (a_700_mb_4k_file_is_a_lie, a_60_minute_file_for_a_140_minute_film_is_a_lie, your_own_file_beats_a_better_stream, choosing_hindi_plays_hindi_even_at_lower_quality, ...) and tests/properties.rs 7 properties over 1,000-2,000 seeded random lists each (ranking_is_a_permutation, the_order_does_not_depend_on_the_order_given, a_sunk_copy_is_never_ahead_of_an_honest_one, an_honest_local_file_that_meets_the_choice_is_first, the_total_is_the_sum_of_the_terms_and_every_value_is_bounded, more_seeders_never_lower_a_score, carrying_the_chosen_language_never_hurts). Seen to fail: reversing the local, sunk and total rules each failed its property; plan test the_scorer_chooses_not_the_order_sources_answer_in fails with the duration floor at 0.1.

**E2** — Selecting "Hindi" plays a source with a Hindi audio track, or clearly reports that none is available.

> 92d4d71: crates/playback/tests/plan.rs choosing_hindi_opens_the_copy_with_hindi_audio (plan_with opens the Hindi copy over a higher-resolution English one; with no Hindi copy, score.unmet = "carries en, not hi"); src/features/detail/DetailScreen.test.tsx "says plainly when no copy has the chosen language" ("None of your sources has Hindi audio.", Play still enabled, Hindi still selected); both seen to fail with the unmet text removed. Real window 2026-11-06: Nosferatu (1922) detail page offers Quality Best/720p/SD from the Internet Archive and re-plans on choice (CDP + PrintWindow).

**E3** — Selecting 4K plays genuine 4K, or explains why it can't.

> 92d4d71: crates/playback/tests/plan.rs choosing_4k_opens_the_4k_copy_or_says_there_is_none (4K chosen opens the UHD copy; Tier 0 with nothing chosen gets 1080p; with no 4K copy, unmet = "1080p, not 4K", shown as "None of your sources has this in 4K."); "genuine" by crates/selection/tests/fixtures.rs a_700_mb_4k_file_is_a_lie (a fake 4K is sunk and never offered: the_choices_offered_are_what_honest_copies_carry).

**E4** — A deliberately-failing top candidate falls back within 10 s with no user action.

> 0f18990: crates/playback/tests/fallback.rs a_dead_first_choice_falls_back_within_ten_seconds — headless libmpv with the app's Patience::default(), top choice a loopback server that accepts and never answers, next a local file: Play -> fallback's first frame 8.11 s (< 10 s); seen to fail at 11.17 s with the load patience at 11 s. Rules: a_failing_first_choice_falls_through_to_the_next, slow_is_not_broken, an_unresolvable_fallback_is_skipped_and_recorded, when_nothing_starts_every_reason_is_kept, a_newer_play_stops_the_old_fallback, only_honest_copies_below_the_chosen_one_are_fallbacks. Real window: normal Play through the new path, first frame 5615 ms, skipped=0 (docs/eval-results.md).

**E5** — The expert panel's score breakdown is legible and matches the code.

> 5b50da9: src/features/detail/DetailScreen.test.tsx 'Ctrl+Shift+S shows every source with the arithmetic of its score (E5)' — points render as value x weight (+4.00, +1.50) and the shown total (+5.50) is the backend's Score.total; seen to fail when the panel printed value instead of points. The panel shows the backend's Score.terms verbatim (CandidateView/TermView in src-tauri/src/commands/playback.rs). Legible: real-window screenshot 2026-11-06 of Nosferatu (1922), six Internet Archive candidates ranked with source, quality, codec, transport, total and nine term rows each. 'Why this source' line: crates/selection/tests/explained.rs (4 tests).

**E6** — `docs/specs/source-scoring.md` documents every term in the scoring function and its weight, with justification.

> 5b50da9: docs/specs/source-scoring.md documents all nine terms (§2 table: weight, +1/0/-1 meanings), each weight's justification (§2 'Why these weights'), fakes (§3), choices (§5), fallback (§6) and explanation (§7). crates/selection/tests/explained.rs the_specs_weights_are_the_codes_weights parses the table and fails on any mismatch with selection::weight (seen to fail with health documented as 3).

**E7** — The TV focus language (ADR-0042) is in the design system, and `tools/uiaudit` proves it in TV mode: the white ring is visible on every focus stop, the ring glides rather than jumps, the 125% scale and safe margin hold at 1920×1080, and under reduced motion no transform remains.

> 1be4948 (9.0) + 91595da: tools/uiaudit/controller.mjs in CI ('Controller flow (simulated Xbox pad)') at --window-size=1920,1080 with a simulated pad: 'the focus ring is white, 3px and showing'; 'the ring surrounds every focus stop on the film page (19 stops; missed: none)' (seen to fail naming six stops when the choice rows opted out); 'the ring glides to the next key: 9 frames in between, overshoot 0.00px' (seen to fail with the spring removed, 9.0); 'TV mode renders the app at 125%' and 'the scaled app fills the window, with a 5% safe margin' (seen to fail when the zoom shrank the app, 9.0); 'under reduced motion a focused tile has no transform or shadow' and 'the ring moves without gliding (0 frames in between)'. 19/19 locally 2026-11-06.

### Debt incurred

- **D67** sinephile_playback::machine assumes no AV1 hardware decode and no HDR display (neither is probed), and Context.throughput is None until the torrent engine or HTTP path reports a measured speed. Probe both via DXGI/D3D11 video decoder profiles in crates/tiers and feed a measured speed in 9.4.
- **D68** original_language is filled only for anime matched to AniList, and only on an ingest or refresh after 92d4d71; IMDb films have none, so they show no original/dub labels. Fill it from TMDB (original_language) when a key is present (Phase 11 enrichment), and re-run the AniList pass on existing catalogues.
- **D69** A swarm abandoned by fallback (9.4) stays in the torrent engine; remove or pause it when fallback moves past a torrent candidate (crates/playback/src/fallback.rs, sinephile_torrent::Engine).

### Next phase starts by

Phase 10 subtask 10.1 (docs/phases/phase-10-subtitle-pipeline.md): finish crates/subtitle-align on branch phase/10-subtitle-pipeline (work in progress is in git stash "phase 10 wip"): replace the z-score confidence with distinctness (best peak over the best peak at least 3 s away), use it for split windows, add negative controls to tools/eval/src/subs.rs, and reach > 90% on cargo run --release -p eval -- subs.
