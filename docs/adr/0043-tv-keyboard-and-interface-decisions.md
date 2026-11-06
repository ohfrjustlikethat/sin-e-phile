# 0043 — The TV keyboard, and the interface decisions that lock the app

- **Status:** Accepted 2026-11-06, decided by the author question by question: "on the tv mode, the app needs to also have a keyboard so that the user can type anything … ask me further questions on the gui and ui of this app, so we can crystal clearly lock the app … this app needs to be ready in its entirety within the week, with me involved only when absolutely necessary."
- **Date:** 2026-11-06
- **Phase:** 11 (keyboard, pause panel, end card, mini player); 14 (profile picker); 17 (Surprise me, mood shelves); 18 (ambient screen, launch); 19 (watch stats); 20 (voice search)
- **Amends:** SPEC.md §15 (preamble, Phases 11, 14, 17, 18, 19, 20). Builds on [ADR-0041](0041-xbox-controller-as-a-remote.md) and [ADR-0042](0042-tv-focus-language.md).

## Context

Phase 8 gave the controller an on-screen keyboard: an alphabetical grid that opens for
the search field. With TV mode (ADR-0042) the app is meant to be used from a couch with
nothing but a pad, so every text field (API keys, languages, links) must be fillable
without a physical keyboard, and typing on a grid must be quick.

The author also asked to settle the remaining interface questions now, so that nothing
about how the app looks or behaves is left to a later guess, and set the schedule: every
phase, finished, within about two weeks.

## Decision

The author chose each of these from presented options.

**The keyboard.**
- **Layout:** the Netflix TV grid, a compact a–z / 0–9 block on the left with results
  updating live beside it; space and backspace keys below.
- **Coverage:** every text field in the app, not only search.
- **Typing aids:** all four. A row of title suggestions from the catalogue as you type,
  recent searches before you type, word completion from catalogue titles and names, and
  voice search.
- **Buttons:** Xbox standard. A types, B closes, X backspaces, Y types a space, LT/RT move
  the cursor, LB/RB switch between letters and symbols (case and punctuation, for API
  keys), Menu submits.
- **Voice:** Windows' own offline speech recognition on every tier; Whisper instead on
  Tier 2, for accuracy. Nothing leaves the PC (§2.7).
- **Opening it** (decided in building it, 11.7): the search field opens the keyboard as soon
  as it is focused, as on Netflix. Any other field opens it on A, because a keyboard popping
  up at every field the D-pad passes made walking Settings impossible (found by the
  controller flow). Done from search moves down to the results; from any other field focus
  stays on it.

**The interface.**
- **Pause overlay:** a right-side panel (Prime X-Ray style). The dimmed frame stays visible
  on the left; the D-pad moves into the panel.
- **When a film ends:** the credits play in full, then a calm end card: what you watched,
  rate it, what next. A series keeps Phase 11's next-episode countdown.
- **Idle in TV mode:** after 5 minutes, an ambient screen. It is a slow slideshow of artwork
  (typographic title cards without artwork) with the clock, and any button returns.
- **Profiles:** the "Who's watching?" picker at launch only when there are two or more
  profiles; otherwise straight to Home.
- **Launch:** straight to Home. No intro, no wordmark animation, no sound.
- **New features:**
  - **Surprise me:** one button and a pad shortcut. It picks a film you will probably like
    from your taste and your sources, and opens it ready to play.
  - **In-app mini player:** keep watching in a corner while browsing the app. This is
    separate from Windows picture-in-picture.
  - **Mood shelves:** Home rows by mood ("Slow and beautiful", "Tense, under 100 minutes")
    built from the semantic index rather than genres.
  - **Watch stats:** a private page of hours watched, the directors and decades you return
    to, and your year in film. Nothing leaves the PC.

**The schedule.** Every phase, 0–27, finished at full depth. No tier is cut; Tiers C and D
get the same rigour as Tier B. The target is about two weeks from 2026-11-06. The author is
involved only when absolutely necessary (ADR-0040's reserved decisions).

## Placement

| What | Phase |
|---|---|
| The TV keyboard (layout, every field, buttons, suggestions, recents, completion) | 11 |
| Right-side pause panel; end card after credits; in-app mini player | 11 |
| Profile picker only with 2+ profiles | 14 |
| Surprise me; mood shelves | 17 |
| Ambient idle screen; no launch intro | 18 |
| Watch stats | 19 |
| Voice search (Windows speech; Whisper on Tier 2) | 20 |

Voice goes to Phase 20 because Windows' speech recognition is a platform integration, the
subject of that phase. The keyboard leaves a place for its key, hidden until then.

## Consequences

- Every text input is reachable by pad, which Phase 18's controller-only audit then
  enforces across all screens.
- Phase 11 grows by a keyboard subtask, and three phases gain a feature each. No exit
  criterion is weakened. Phase 11 gains one criterion for the keyboard.
- Abandonment (R7) is mitigated differently: the author has chosen breadth at full depth.
  Tier boundaries remain natural checkpoints for the understanding gate (§10.10), but the
  work continues through them.
