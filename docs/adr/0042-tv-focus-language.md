# 0042 — A TV focus language: a white ring, lift, and a living tile

- **Status:** Accepted 2026-11-06, decided by the author question by question: "you know how it works in the netflix xbox app? theres a white rectangle around the thing youre selecting, i want something exactly like that … i need to implement a tv feel in this pc app … Apply some apple gui features, the way it feels responsive and active."
- **Date:** 2026-11-06
- **Phase:** 9 (the focus language, in the design system); 18 (billboard and artwork backdrop)
- **Amends:** SPEC.md §9.0 (a scoped exception), §9.3, §9.4, Phase 9, Phase 18. Builds on [ADR-0041](0041-xbox-controller-as-a-remote.md).

## Context

ADR-0041 made an Xbox controller a remote, with a thicker oxblood ring as its "ten-foot
treatment". From a couch that is not enough: the author wants the TV feel of Netflix on
Xbox (a bright white rectangle around what is selected) and the tactility of Apple TV
(the focused tile lifts toward you, tilts under your thumb, and catches the light).

Research, recorded in the session log: Microsoft's Xbox guidance says the standard focus
rectangle is too hard to see at ten feet and apps should draw a larger one that is always
visible, and keep content inside a TV-safe margin. Apple's tvOS focus effect is a lift
(scale and shadow), a specular highlight, and parallax driven by the remote, with text
kept flat and everything disabled under Reduce Motion. Users of other media apps report
that a white focus border vanishes on light posters unless it has a dark edge.

Several of the author's choices collide with §9.0, which bans glow, shadow as a depth
cue, and gradient as decoration, and says to take none of Netflix's visual language. The
author owns §9.0, and chose these knowingly when shown them. The decision is how to admit
them without dissolving the rules that keep the rest of the app from looking generated.

## Decision

**One focus language, for keyboard and controller alike; mouse users see no ring.** It
is a single component in `src/design-system/focus/`, and **the only place** in the code
where shadow, glow, a gradient sheen and 3D transforms are allowed. The design lint
(Phase 18) enforces that boundary.

| Element | Specification |
|---|---|
| Ring | 3px `--focus-ring` (white), 2px gap, a 1px dark hairline outside it so it holds on light artwork, and a tight `--oxblood-bright` halo (18px, 35%). Radius 2px, the system's own. |
| Grow | Focused tiles scale to 1.06; neighbours do not move. Buttons and text controls do not scale. |
| Lift | A soft shadow deepens under the focused tile only. Depth everywhere else is still surface value and lines. |
| Tilt | Resting a thumb on the left stick, inside the navigation deadzone, tilts the focused tile up to 6° toward it (perspective 800px). Text stays flat. Past the deadzone, focus moves as before. |
| Sheen | A faint diagonal light crosses the tile once as it gains focus, then follows the tilt. |
| Press | Holding A presses the tile to 0.97× its focused size; it springs back on release. |
| Motion | The ring **glides** from target to target and rows scroll smoothly to keep the focused tile in view, on a critically damped spring of about 200 ms: alive, with no overshoot. The author asked for "a combination" of springy and crisp; this is that. |
| Couch scale | With a pad, the app renders at 125% inside a 5% TV-safe margin. |
| Hints | A quiet footer while a pad is in use: neutral button glyphs (no Xbox colours, §9.1) with the actions this screen offers. |
| Sound | Short synthesised ticks on move and select (Web Audio, no assets), very quiet by default; Settings can turn them off. |
| Rumble | About 20 ms of light rumble on select, by default; Settings can turn it off. Best effort: the Gamepad API's `vibrationActuator` where WebView2 exposes it. |
| Full screen | The View button toggles full screen; picking up the controller enters it, and a setting turns that off. |
| Reduce motion | Grow, lift, tilt, sheen, press and the glide all stop; the ring stays, cross-fading in 100 ms. |

**Phase 18 adds the surfaces:** a typographic billboard at the top of Home, and behind it
the focused film's **full artwork backdrop** when the user has a TMDB key, falling back to
a dim tint when there is none (ADR-0013: a default install has no artwork, so the
fallback is the primary state and is designed first).

**The masthead stays** (ADR-0037). Up from the first row reaches it; LB/RB change
destination. No left rail.

## Consequences

- §9.0's bans still hold everywhere outside the focus component; a shadow or gradient
  anywhere else is a defect, and the design lint will say so.
- The focus language is built in Phase 9, ahead of its own surfaces, because every
  later screen inherits it; the language and quality pickers that Phase 9 adds are its
  first users. Scoping it into Phase 9 is recorded there, not left silent.
- A moving ring is one more element to keep in sync with focus. It is driven by the
  same focus events the keyboard uses, so `tools/uiaudit` checks both.
- 125% in pad mode means fewer tiles per row on a desk monitor while a pad is in use;
  the mouse or keyboard returns it to 100%.
- Sounds and rumble on by default are a choice the author made; they are quiet, and one
  setting each turns them off.
