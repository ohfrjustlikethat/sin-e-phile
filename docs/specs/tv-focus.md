# TV focus language

The build input for ADR-0042: what the focused thing looks like, how it moves, and what
"TV mode" changes. Numbers here are the contract; `tools/uiaudit/controller.mjs` checks
the ones that can be measured.

## 1. Where it lives

`src/design-system/focus/` is the only place shadow, glow, gradient and 3D transforms
are allowed (§9.0's scoped exception). Everything in it is driven by the document's own
focus, so the keyboard, the controller and `tools/uiaudit` all exercise the same path.

| Piece | Job |
|---|---|
| `FocusRing` | One absolutely positioned element above the page that follows `document.activeElement` when focus is visible (keyboard or pad, never mouse). |
| `spring.ts` | A critically damped spring (ζ = 1, settles in ~200 ms): pure, tested without a DOM. |
| `tile.css` + `data-focus-tile` | Grow, lift, sheen and press for elements marked as tiles. |
| `tilt.ts` | Left-stick tilt for the focused tile. |
| `tvMode.ts` | Pad mode: scale, safe margin, the hints footer, sounds, rumble, full screen. |
| `sound.ts` | Web Audio ticks, synthesised (no audio files). |

## 2. The ring

- 3px `--focus-ring` (#FFFFFF), 2px gap, then a 1px `rgba(0,0,0,0.6)` hairline outside so
  it holds on a white poster, then a halo `0 0 18px 2px` of `--oxblood-bright` at 35%.
- Radius 2px — the system's own.
- Mouse input hides it; keyboard or pad input shows it (`:focus-visible` semantics).
- **Glide**: on a focus change the ring moves from the old rectangle to the new one, x, y,
  width and height each on the spring. A move of more than 1.5 viewport heights (a page
  change) skips the glide and fades in, 100 ms.
- Reduced motion: no glide; the ring cross-fades in 100 ms at its new place.

## 3. Tiles (`data-focus-tile`)

Posters, cards, result rows, episode tiles. Buttons and text fields are not tiles: they
get the ring only.

| Effect | Value |
|---|---|
| Grow | scale 1.06 on focus, 200 ms spring; neighbours do not move (transform only) |
| Lift | shadow `0 18px 40px rgba(0,0,0,0.55)`, fading in with the grow |
| Tilt | left stick inside the navigation deadzone (0.12 < magnitude < 0.5) rotates up to 6° on X and Y, perspective 800px; springs back when released |
| Sheen | `linear-gradient(115deg, transparent 35%, rgba(255,255,255,0.12) 50%, transparent 65%)` sweeps once over 600 ms on focus, then sits offset by the tilt |
| Press | A held: scale 0.97 × the focused scale, 80 ms; spring back on release |

Text inside a tile is never tilted separately; it moves with the tile as one plane.

## 4. TV mode (a pad was the last input)

- **Scale**: the app renders at 125% (CSS `zoom` on the root) inside a 5% safe margin on
  every edge. Mouse or keyboard input returns it to 100%.
- **Hints footer**: bottom right, `--void` background, neutral round glyphs with the
  letter (no Xbox colours, §9.1), `label` type: the actions this screen offers, e.g.
  `Ⓐ Select  Ⓑ Back  Ⓨ Search`. Screens declare their hints; the player hides the footer
  with its chrome.
- **Sound**: a 4 ms filtered click on move, a softer two-tone on select, a low tick at a
  row's end. Default volume 15%. Settings → Controller: on/off and volume.
- **Rumble**: `vibrationActuator.playEffect("dual-rumble", {duration: 20, weakMagnitude:
  0.25, strongMagnitude: 0})` on select. Settings: on/off. Silently absent where the
  WebView does not expose the actuator.
- **Full screen**: the View button toggles it. The first pad input after launch enters
  it unless Settings → Controller turns that off. Leaving TV mode does not leave full
  screen; View or Escape does.

## 5. Reduced motion

`prefers-reduced-motion: reduce` removes grow, lift, tilt, sheen, press and the glide.
The ring remains, cross-fading in 100 ms. The audit checks that no `transform` other
than `none` is computed on any focused element under reduced motion.

## 6. Phase 18 additions

The Home billboard and the focused film's full artwork backdrop (with a TMDB key; a dim
tint without), cross-fading in under 300 ms as focus moves.
