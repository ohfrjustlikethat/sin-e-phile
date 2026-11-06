# Phase 9 — Intelligent Source Selection

**Status:** in_progress · **Depends on:** 8 · **Sessions:** 1–2

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
- [ ] **E5** The expert panel's score breakdown is legible and matches the code.
- [ ] **E6** `docs/specs/source-scoring.md` documents every term in the scoring function and its weight, with justification.
- [ ] **E7** The TV focus language (ADR-0042) is in the design system, and `tools/uiaudit` proves it in TV mode: the white ring is visible on every focus stop, the ring glides rather than jumps, the 125% scale and safe margin hold at 1920×1080, and under reduced motion no transform remains.

## Subtasks

- [x] **9.0** The TV focus language (ADR-0042) in src/design-system/focus/: the white ring with hairline and oxblood halo as one gliding element (critically damped spring ~200 ms), grow 1.06 and lift on tiles, left-stick tilt up to 6 degrees with a following sheen, A press-down 0.97x, TV mode with a pad (125% scale, 5% safe margin, button-hints footer, Web Audio ticks, ~20 ms rumble on select, View toggles full screen and picking up the pad enters it), settings for sounds, rumble and auto full screen, reduced motion honoured; extended tools/uiaudit/controller.mjs proves it.
- [x] **9.1** crates/selection: score(candidate, context) -> Score{total, terms[]} as a pure function over SourceCandidate plus a Context (title runtime, hardware tier and decode caps, preferred audio language, wanted quality, measured throughput). Terms: local availability, swarm health, size-versus-quality plausibility, encoding signals, codec/HDR compatibility, audio language, subtitles. Unit tests over a fixture set of candidate lists and property tests (ordering invariants).
- [x] **9.2** Fake and mislabel detection: bytes-per-minute against the claimed resolution and codec, and duration against the catalogue runtime; a 700 MB '4K' or a 60-minute file for a 140-minute film is flagged and sunk, with the reason kept. 'Release-group reputation' is implemented as structural signals only (naming consistency, encode tags, plausibility); no release-group or indexer names appear anywhere in the repo (SPEC.md 2.1).
- [x] **9.3** Language and quality as first-class choices: the detail page lists the audio languages the candidates actually carry and the qualities on offer (defaulting to the best the hardware and connection support), with dub/sub distinction for anime; choosing one re-plans to a candidate that carries it, or says plainly that none does.
- [x] **9.4** Automatic fallback: the plan returns the ranked list; if the chosen candidate has no first frame within a start timeout, the player silently opens the next one and logs why, within 10 s of Play in total for a failing first choice.
- [ ] **9.5** The expert panel (Ctrl+Shift+S, and a Settings toggle): every candidate ranked with each score term and its weight, matching the code; and a one-line 'why this source' under Play.
- [ ] **9.6** docs/specs/source-scoring.md: every term, its weight and its justification; learning note; evidence for E1-E6 including a deliberately failing top candidate in a test.

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
