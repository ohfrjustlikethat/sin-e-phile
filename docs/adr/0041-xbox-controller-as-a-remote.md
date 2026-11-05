# 0041 — An Xbox controller drives the whole app, like a remote

- **Status:** Accepted 2026-11-05 at the author's request: "i want my app to also be able to be controlled by an xbox controller … so people have a remote control in the shape of an xbox controller."
- **Date:** 2026-11-05
- **Phase:** 7 (recorded); built in Phase 8 and 18
- **Amends:** SPEC.md §9.4 (a new input requirement), Phase 8 and Phase 18 deliverables and exit criteria

## Context

Netflix on Xbox is driven entirely by the controller: the D-pad and left stick move focus,
A selects, B goes back, the triggers scrub. The author wants the same here. sin-e-phile
already has to be fully keyboard-navigable with visible focus (§9.4, Phase 2's
`tools/uiaudit`), so a controller is a second way to drive a focus system that exists.

An Xbox controller connects to Windows over **Bluetooth** (Xbox Series and recent One
controllers), the **Xbox Wireless Adapter**, or **USB**. However it connects, Windows
presents it the same way, so the app does not care which.

## Decision

**The W3C Gamepad API, in the WebView, is the input path.** WebView2 is Chromium and
exposes `navigator.getGamepads()` with the "standard" button mapping for Xbox controllers.
It needs no native code, no new crate, and no permission. Every press is mapped onto
the app's existing focus navigation, so anything reachable by keyboard is reachable by
controller, and the existing keyboard audit covers both.

| Control | Browsing | Player |
|---|---|---|
| D-pad / left stick | move focus (spatial) | — / seek ±10 s (left, right) |
| A | select | play / pause |
| B | back / close | exit player (with confirm if mid-film) |
| X | — | subtitles menu |
| Y | search | audio menu |
| LB / RB | previous / next tab | previous / next chapter or episode |
| LT / RT | page up / down | seek −30 s / +30 s, held to scrub |
| Menu (≡) | settings | pause overlay |
| View (⧉) | — | info overlay |
| Xbox button | owned by Windows (Game Bar); not used | |

Focus must be **visible from a sofa**: a controller user is ten feet away, so the focused
element gets the larger "TV focus" treatment whenever the last input came from a
controller, and the ordinary keyboard ring otherwise.

**The native fallback is held in reserve.** The Gamepad API only reports while the window
has focus, which is right for a media app, but if it proves unreliable in WebView2 (e.g.
missed input after a fullscreen switch under libmpv), the fallback is
`Windows.Gaming.Input` through the `windows` crate, which the project already depends on,
delivering events to the frontend over Tauri IPC. That would need no ADR beyond this one.

**Text entry** (search) uses an on-screen keyboard designed for the D-pad, because Windows'
touch keyboard does not respond to a controller. It arrives in Phase 18 with the 10-foot
audit; until then a controller user reaches search results through browsing and
recommendations, and Y opens search for whoever has a keyboard to hand.

## Consequences

- **Phase 8** gains: the gamepad layer, the mapping above for browsing and the player, the
  TV-focus treatment, and an exit criterion — *with only a controller, a user can browse
  to a film, start it, pause, seek and leave the player*. Tested in CI with a simulated
  gamepad (the Gamepad API is scriptable in headless Chrome), and confirmed once by the
  author with a real controller.
- **Phase 18** gains: the on-screen keyboard, and an exit criterion — *every screen is
  usable with a controller alone* — added to the design audit.
- Nothing to buy. A Bluetooth Xbox controller pairs through Windows Settings → Bluetooth.
