# Manual test plan

The things automation cannot judge: playback quality, subtitle rendering fidelity,
UI responsiveness under load, and the feel of the discovery rails.

> **Status: Phase 0.** Nothing to test manually yet. Checklists are added by the
> phase that makes them testable.
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
| Discovery rails: are the recommendations any good? | 17 |
| Tier 0 experience on genuinely constrained hardware | 21 |

## Standing checks, every UI phase

- [ ] Every interactive element reachable by keyboard, with a visible focus ring
- [ ] `prefers-reduced-motion` respected
- [ ] Correct at 100%, 150% and 200% DPI
- [ ] No layout shift as images load
- [ ] Every empty state is designed, not blank
- [ ] Artwork-free state looks deliberate, not broken (ADR-0013)
