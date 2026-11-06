# Phase 9 — Learning notes

**Intelligent source selection, and the TV focus language that every later screen
inherits.** Four sections per ADR-0016 (A4). Written as the phase goes; 9.0 first.

---

## 1. What we built

- **The TV focus language** (9.0, ADR-0042) — `src/design-system/focus/`: one white ring
  that glides between targets, tiles that grow, lift, tilt under the left stick, catch a
  sheen and press in; TV mode with a pad (125% scale, safe margin, button hints, quiet
  sounds and rumble, View for full screen); Settings → Controller.
- **Proof in a real browser** — `tools/uiaudit/controller.mjs` now checks the ring's
  colour and position, the glide frame by frame, the scale and margin, the tile effects,
  and that reduced motion removes them.
- **Source scoring** (9.1–9.2) — `crates/selection/`: every candidate gets nine scored terms
  and a sentence for each, and the list is ordered: sunk fakes last, copies that miss the
  viewer's choice next, a file on this machine first, then by points. Play now uses it.
- **Fake and mislabel detection** — `crates/selection/src/plausibility.rs`: too few bytes a
  minute for the claimed resolution, too short for the film's runtime, a name that
  contradicts its listing, or a file that is not a video at all.
- **Language and quality choices** (9.3) — `src/features/detail/ChoiceRow.tsx`: Audio and
  Quality rows under Play, offering only what honest copies carry, labelled original or
  dub, remembered across films, and saying plainly when no copy has the choice.
- **Automatic fallback** (9.4) — `crates/playback/src/fallback.rs`: a copy that errors is left
  at once, one that never loads after 8 s, one that loads is given 30 s; the next copy is
  resolved on demand; a newer Play stops an old loop.

## 2. Why

- **One ring element instead of an outline per element**, so it can *move*: an outline
  can only appear and disappear, and a glide is what makes focus feel alive.
- **A critically damped spring** is the fastest motion that never overshoots: the ring
  arrives without bouncing past and coming back.
- **CSS for the tiles, JavaScript for the ring.** A tile's effects are states of one
  element, which CSS transitions do well; the ring has to follow a target that may be
  moving (scrolling, growing), so it is recomputed every frame.
- **Rules before points.** "A local file always wins" and "a fake is never played" are
  rules, so the order is lexicographic: no weight can be big enough to buy past a rule,
  and none has to be tuned to pretend to be one.
- **A pure function** so a ranking can be reproduced exactly in a test and shown term by
  term in the expert panel; nothing it depends on is hidden.
- **Structure, not names, for reputation**: a list of group names would be a catalogue of
  where to find things (SPEC.md §2.1), and it would go stale.
- **The exception is confined to one folder** so the design rules can still be enforced
  everywhere else by a lint, rather than by memory.

## 3. New concepts

- **Critically damped spring** — `src/design-system/focus/spring.ts:24`: the exact solution
  x(t) = (x0 + (v0 + ωx0)t)e^(−ωt); exact rather than stepped, so a slow frame cannot
  overshoot.
- **`requestAnimationFrame` loop that reads layout** — `FocusLayer.tsx` `FocusRing`: read
  `getBoundingClientRect`, step the spring, write a transform; one read and one write per
  frame.
- **`useSyncExternalStore` over a `MutationObserver`** — `FocusLayer.tsx` `usePadMode`: React
  re-renders when an attribute on `<html>` changes, without polling.
- **CSS custom properties as animation inputs** — `focus.css` `--tilt-x`, `--tilt-y`,
  `--tile-scale`: JavaScript sets one variable, the transform and its transition do the
  rest.
- **CSS `zoom` scales lengths, not percentages** — `focus.css` TV mode: dividing the width
  by the zoom left the app in 80% of the window; the audit now measures that it fills it.
- **Web Audio synthesis** — `feedback.ts`: an oscillator and a gain envelope make a tick,
  with no audio files to ship or license.
- **`vibrationActuator`** — `feedback.ts` `rumble`: the Gamepad API's rumble, used if present.
- **Tauri capabilities** — `src-tauri/capabilities/default.json`: a window call the app is
  not granted fails silently; the player's full-screen button had never worked.

- **Lexicographic ordering with `Ordering::then`** — `crates/selection/src/lib.rs` `compare`:
  each `.then` only matters when everything before it is equal.
- **`f64::total_cmp`** — `crates/selection/src/lib.rs` `compare`: a total order on floats,
  so `sort_by` never sees "neither less nor greater" (NaN).
- **Property tests from a seeded generator** — `crates/selection/tests/properties.rs` `Rng`:
  thousands of random lists, the same ones every run, checking what must hold for all.
- **Mutation as proof** — swapping one `.then` in `compare` fails
  `an_honest_local_file_that_meets_the_choice_is_first`; a property nobody has seen fail
  proves nothing.
- **`#[serde(default)]` for a wire field** — `crates/source-protocol/src/types.rs`
  `duration_seconds`: old addons that never send it still parse.

- **`Intl.DisplayNames`** — `src/lib/playbackChoice.ts` `languageName`: the platform turns `hi`
  into "Hindi", so the app ships no language table.
- **`role="radiogroup"` with buttons** — `src/features/detail/ChoiceRow.tsx`: one choice of
  several, announced correctly, and every option a focus stop for the pad.
- **`COALESCE(?, column)` in an UPDATE** — `crates/catalogue/src/anime.rs`: write a value
  only when there is one, never erase a known one with NULL.

- **A trait as a seam** — `crates/playback/src/fallback.rs` `Screen`: the loop is tested with a
  scripted fake player and with the real one, and the app adds `superseded`.
- **`spawn_blocking`** — `fallback.rs` `try_one`: the player's waits block a thread, so they
  run on tokio's blocking pool, never on the async workers.
- **A generation counter (`AtomicU64`)** — `src-tauri/src/commands/player.rs` `OPENS`: each
  Play counts up; an older loop sees the count move and stops.

## 4. The questions

Written, not asked — Phase 9 is not a tier boundary (§10.10).

1. Why does the ring use a spring with damping ratio exactly 1, and what would a user see
   at 0.5 and at 2?
2. The ring is drawn outside `#root`, but the elements it follows are inside a zoomed
   `#root`. Why does its position still come out right?
3. A focused tile grows to 1.06 using `transform`, not `width`. Why does that keep its
   neighbours still?
4. Why does leaving TV mode need 32px of mouse movement, when entering it needs one
   press?
5. Under reduced motion the ring still moves to the new target. What exactly is removed,
   and why is the ring itself kept?

Source scoring (9.1–9.2):

6. Why is "a local file always wins" a step in the order rather than a large weight?
7. A copy states no audio languages; another states English. The viewer chose Hindi.
   Which ranks higher, and why is that the honest choice?
8. Why are `quality` and `plausibility` given the same weight?
9. Why does the bytes-per-minute floor use 60% for an unknown codec rather than 100%?
10. Why is a camera recording scored −1 but not sunk, while a ".exe" is sunk?
