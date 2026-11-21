# Manual test plan

The things automation cannot judge: playback quality, subtitle rendering fidelity,
UI responsiveness under load, and the feel of the discovery rails.

> **Status: Phase 11.** The playback checklist below is the first written out in full.
> Checklists are added by the phase that makes them testable.
>
> **Run this before merging any phase that touches playback or UI** (`SPEC.md`
> §12.3). Record the result as `manual: <what you did and observed>` evidence in
> `PROJECT_STATE.json` — that is an accepted evidence form under §10.8, and the only
> one for criteria a human must judge.

## Checklists, by the phase that adds them

| Checklist | Added in |
|---|---|
| Window behaviour, tier detection, crash handling | 1 |
| Design system: keyboard navigation, focus order, reduced motion | 2 |
| Playback quality, seeking, hardware decode, track switching | 8 |
| Subtitle rendering fidelity, especially ASS/SSA positioning | 10 |
| Pause overlay, scrub thumbnails, next-episode flow | 11 |
| Review queue ergonomics — 20 ambiguous items in under a minute | 12 |
| Onboarding with fresh eyes, on an actual non-technical person | 14 |
| Discovery rails: are the recommendations any good? (Phase 18 E5, below) | 17, 18 |
| Tier 0 experience on genuinely constrained hardware | 21 |

## Standing checks, every UI phase

- [ ] Every interactive element reachable by keyboard, with a visible focus ring
- [ ] `prefers-reduced-motion` respected
- [ ] Correct at 100%, 150% and 200% DPI
- [ ] No layout shift as images load
- [ ] Every empty state is designed, not blank
- [ ] Artwork-free state looks deliberate, not broken (ADR-0013)

## Playback, on every tier (Phase 11, E5)

Run in the real window, once per tier. A debug build takes `SINEPHILE_TIER=modest|standard|capable`
(the tier override from the first frame) and `SINEPHILE_AUTOPLAY=<path or address>` (plays it on
launch); both are compiled out of release builds. Rebuild the exe after `npm run build`: a debug
exe embeds the frontend it was compiled with.

Use a local file with a sidecar `.srt` that is offset by over a minute, and the same file served
over HTTP for the stream (`python -m http.server --bind 127.0.0.1`). Delete
`data/cache/subtitles/aligned` first so the aligner really runs.

- [ ] **Local file:** the first frame arrives; `player_state` shows hardware decode.
- [ ] **Subtitles:** on Standard and Capable the log says `aligned: … offset …`; on Modest
      (no speech alignment, §8) it says the file fits the length and is loaded as it is.
- [ ] **Pause:** Space shows the overlay in < 200 ms (`window.__pauseMs`). A file the
      catalogue knows gets the right-hand panel; a bare file gets no panel, not an empty one.
- [ ] **Scrub thumbnails:** `scrub_thumbs` returns sheets within 30 s.
- [ ] **Statistics (I):** codec, size, decoder, bitrates (kbit/s below one megabit), dropped
      frames, buffer, delays.
- [ ] **Shortcuts (?)** opens; Esc closes it.
- [ ] **Mini player (Ctrl+M):** the app returns with the film in a corner, its keys are the
      app's again, and Ctrl+M brings the film back.
- [ ] **Picture in picture (Ctrl+P):** a small always-on-top window; Ctrl+P restores the exact
      size and position.
- [ ] **Stream:** the same over HTTP: first frame, pause overlay, statistics.
- [ ] No `ERROR` or panic in the log.

**Last run: 2026-11-07, Phase 11 close.** All boxes, all three tiers. Screenshots in
`docs/evidence/phase-11/`; the numbers are in `PROJECT_STATE.json` under Phase 11 E5.

## Discovery on your own taste (Phase 18, E5: "the real acceptance test")

The author, on their own profile in the installed release, after at least ten films rated or finished (a blind spot is
said only after ten, and a country only once Wikidata has been asked about everything met: the hourly check does it).
**To rate ten quickly** (ADR-0056): Home's "Rate films" card while under ten, or Settings → your taste → Rate more
films; or the stars under any film's title, or X / right click on any card.

- [ ] Home shows "Where your taste points", and at least two discovery rails (a stretch, a bridge, the canon, deep cuts,
      or a blind spot), each card saying why. Write down any label that reads as false.
- [ ] **At least three films you had not heard of and want to watch**, from any discovery rail, a mood shelf or
      Surprise me. Name them, and the rail each came from. (E5 is met by these three, in your words.)
- [ ] X on a card (or a right click) rates it (stars; a show's "Rate seasons…"), and on a recommendation offers Not for
      me, Not right now, Already seen; the card becomes an Undo, and the title is gone from Home next time. Undo brings
      it back.
- [ ] Ratings (ADR-0056): a film's stars under its title, half stars by mouse, arrows or the pad; a show's season header
      has its stars and the show says "your average"; the end card's stars; Rate films asks one film after another.
- [ ] Continue watching: the × on a card (mouse), X → Remove, or the title page's link takes it out; playing it again
      brings it back. No line under the row's name.
- [ ] Comfort to Adventure (Settings, your taste): at Adventure Home has more discovery rails; at Comfort, fewer.
- [ ] Surprise me (the word left of Search, or RT on the pad) opens a film's page.
- [ ] Today's two mood shelves read as their moods.

**Not yet run.** Asked of the author in the Phase 19 batch.

