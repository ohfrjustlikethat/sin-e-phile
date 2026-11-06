# Phase 11 — Learning notes

**The player as a place to spend two hours: series, the pause panel, what comes next,
the scrub bar, the controls a film lover expects, and a keyboard for the couch.** Four
sections per ADR-0016 (A4).

---

## 1. What we built

- **Series** — a series page with seasons, playable episodes and "Episode 7 airs
  Thursday" from AniList's airing schedule (migration `0019_airing`).
- **The pause panel** — right-hand, over the dimmed frame: cast (a name opens their
  films), crew, ratings that exist, more like this. Paints in 2–22 ms.
- **What comes next** — a next-episode card with a 10 s countdown in the last 30 s or at a
  credits chapter; for a film, the credits in full, then an end card where you rate it.
- **The scrub bar** — chapter marks and hover thumbnails from FFmpeg sprite sheets, made in
  the background with 0 dropped frames.
- **Controls** — picture-in-picture, a mini player in the corner while you browse, sound
  timing, picture adjustments, statistics, and a shortcuts list generated from the key map.
- **The TV keyboard** — on every text field with a pad: letters, capitals, symbols,
  recent searches, suggestions and word completion.
- **Checked on every tier** in the real window, which found and fixed four defects.

## 2. Why

- **Fetch on play, paint on pause.** A pause panel that asks the database when you press
  Space feels slow; fetched as playback starts, pausing only draws.
- **Intents, not keys.** Keyboard, pad and menu all become the same intent, so a feature
  added once works from all three, and the shortcuts list cannot lie about a key.
- **Declining an intent.** The mini player must let the app have its keys back; returning
  `false` from a listener says "not mine" without the player knowing what the app does.
- **One player slot that never moves.** Shrinking to the corner re-parents nothing, so the
  film does not reopen and lose its place.
- **Keyframes only, at low priority.** Decoding every frame for thumbnails would compete
  with playback; keyframes are a small fraction of the work, and Windows schedules the rest
  around the film.
- **Credits are part of the film.** ADR-0043: a film's credits are never cut off by a card.
- **Check it in the real window.** Unit tests passed for all four defects the tier run
  found; only the real window shows that `setSize` means the inner size, or that a debug
  exe carries the frontend it was compiled with.

## 3. New concepts

- **An intent bus with a "taken" answer** — `src/features/player/intentBus.ts:19`
  `sendPlayerIntent`: true if any listener took it.
- **Claiming buttons ahead of navigation** — `src/lib/gamepad.ts:278` `claimButtons`.
- **A table that tests itself** — `src/features/player/intents.ts:158` `SHORTCUTS`, each
  row with a sample key checked against `keyIntent`.
- **Measuring paint, not render** — `src/features/player/PauseOverlay.tsx:40`:
  `useLayoutEffect` then `requestAnimationFrame`, the moment the frame reaches the screen.
- **Stale-forever queries** — `src/features/player/PlayerScreen.tsx:79` `staleTime:
  Infinity`: a film's cast does not change mid-film.
- **Sprite sheets** — `crates/player/src/thumbs.rs:36` `tile_at`: 100 frames in one JPEG,
  shown by offsetting it.
- **Process priority from Rust** — `crates/player/src/thumbs.rs:84`
  `BELOW_NORMAL_PRIORITY_CLASS` through `creation_flags`.
- **A pure "what now" function** — `src/features/player/endCard.ts:30` `moment`.
- **Text with a cursor** — `src/app/keyboardText.ts:22` `edit`: the keyboard edits a value
  and a position, never the DOM directly.
- **Typing into a React input from outside** — `src/app/PadKeyboard.tsx:30` `write`: the
  native value setter, then an `input` event, so `onChange` fires.
- **Inner and outer window size** — `src/features/player/PlayerScreen.tsx:325`: Tauri's
  `setSize` sets the inner size, so the inner size is what to save.
- **Debug-only hooks** — `src-tauri/src/state.rs` `dev_tier` and
  `src-tauri/src/commands/player.rs` `dev_autoplay`: `#[cfg(debug_assertions)]` compiles
  them out of a release.
- **The next episode across seasons** — `crates/persistence/src/repositories/episodes.rs:270`
  `next_after`.

## 4. The questions

Written, not asked — Phase 11 is not a tier boundary (§10.10).

1. Why does the pause panel fetch its data when playback starts, and what would a user
   notice if it fetched on Space instead?
2. The mini player returns `false` for every intent but one. What would go wrong if it
   simply unsubscribed from the bus instead?
3. Why do thumbnails decode keyframes only, and why are they never made for a stream?
4. A test asks `keyIntent` every row of `SHORTCUTS`. What mistake does that catch that
   reading the table would not?
5. Picture-in-picture saved the outer size and restored it with `setSize`. What happens to
   the window after five round trips, and why did no unit test notice?
