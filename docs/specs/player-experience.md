# The player experience layer (Phase 11)

What later phases build on: how the player takes input, what it shows when, and the
formats it keeps on disk. Decisions are in ADR-0041 (the pad), ADR-0042 (the focus
language) and ADR-0043 (keyboard, pause panel, end card, mini player).

## 1. Input: intents, not keys

Every way of driving the player becomes an **intent** (`src/features/player/intents.ts`):
`keyIntent` maps a key, `playerIntent` (`src/lib/gamepad.ts`) a pad button, and the player menu sends them by hand.
The player runs intents; it never reads a key itself.

- **`SHORTCUTS`** is the single table of what each key and button does. The `?` overlay is
  rendered from it, and a test asks `keyIntent` each row's sample key, so the overlay
  cannot describe a key that does something else.
- **Declining.** A listener on the intent bus (`intentBus.ts`) returns `false` to decline
  an intent; `sendPlayerIntent` reports whether anyone took it. The mini player declines
  everything except its own toggle, so the app underneath gets its keys back; an open
  panel declines the D-pad and A so they move focus inside it.
- **`claimButtons(fn)`** (`src/lib/gamepad.ts`) lets a component take pad buttons before
  navigation does, for as long as it is mounted. The TV keyboard uses it for X, Y, the
  triggers, the bumpers and Menu.

## 2. What appears when

| Moment | What shows | Rule |
|---|---|---|
| Pause | Title, time left, chapter, Resume; for a catalogue film a right-hand panel (cast, crew, ratings, more like this) | Data fetched when playback starts (`pause_details`, cached forever) so pausing only paints. A bare file gets no panel. |
| Last 30 s of an episode, or a credits chapter | Next-episode card with a 10 s countdown | `endCard.ts` `moment`; autoplay switchable in Settings → Playback |
| A film (or last episode) ends | The credits play out, then the end card: rate it, what next | Nothing interrupts the credits (ADR-0043) |
| I | Statistics: codec, size, decoder, bitrates, dropped frames, buffer, delays | Bitrates read in kbit/s below one megabit |
| Ctrl+M | The mini player: the same `PlayerScreen` in a 400×262 corner over the app | The player slot in `App.tsx` never unmounts, so shrinking does not reopen the film |
| Ctrl+P | Picture-in-picture: the window shrinks to 480×270 (logical), always on top | Restores the saved inner size, position and maximised state |

Trivia, goofs and soundtracks are absent: no free, legal source publishes them (§2.1).

## 3. Scrub thumbnails

`crates/player/src/thumbs.rs`. Local files only: sampling a stream would download it.

- One frame every 10 s, 160 px wide, tiled 10×10 to a JPEG sheet; `sheets.json` beside
  the sheets records `interval`, `cols`, `rows`, the tile size and `count`.
- Kept under `data/cache/thumbs/<file hash>/`, so a renamed file keeps its sheets.
- FFmpeg runs at below-normal priority on one thread and decodes keyframes only
  (`-skip_frame nokey`). Measured: 0 dropped frames while generating (E2).
- `tile_at(t)` (Rust) and `tileAt(t)` (TypeScript) are the same arithmetic.

## 4. The TV keyboard

`src/app/PadKeyboard.tsx`, text logic in `src/app/keyboardText.ts` (pure, tested).

- Shown only when the last input was a pad. Docked left, 400 px, so results stay visible.
- The search field opens it on focus; every other text field opens it on A.
- Pages abc / ABC / #+= (RB and LB), X deletes, Y types a space, LT/RT move the cursor,
  Menu is Done, B closes it before it leaves the screen.
- On search only: recent searches (the last eight, this machine only) before typing,
  then title suggestions and word completions from the catalogue.
- Voice search arrives in Phase 20; its key is not shown until then.

## 5. Testing it

- Unit: `PlayerScreen.test.tsx`, `intents.test.ts`, `PadKeyboard.test.tsx`, `endCard.test.ts`,
  `scrubPreview.test.ts`, `keyboardText.test.ts`, `crates/player` tests.
- Pad only, in CI: `tools/uiaudit/controller.mjs`.
- Real window, every tier: `docs/MANUAL_TESTS.md` "Playback, on every tier".
