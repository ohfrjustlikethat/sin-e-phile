# Ready to Watch — the Phase 16 surfaces

Phase 16 (ADR-0045) exists because the release build was unusable though every test passed. This spec
covers what it changed in the player, Home and the first run. TV folder layouts are in
[`library.md`](library.md) ("TV layouts and extras"); the self-refreshing catalogue is in
[`catalogue-freshness.md`](catalogue-freshness.md). Measurements are in `docs/eval-results.md`, Phase 16.

## 1. The hybrid player

- **Clean while watching.** Nothing over the picture until input; the bar and the title fade in on input
  and out after idle. The full panel (subtitles, style, sync, audio, speed, picture) is one button away.
- **The video is a separate native window** (libmpv, `player-embedding.md`). The page shows through it
  only where the page cuts **holes** in the video window's region. Every overlay that must be seen above
  the picture carries `data-cutout`; holes are recomputed whenever one appears or goes. An overlay without
  the mark opens invisibly beneath the video — the bug 16.5 found.
- **Opening**: the video window stays hidden until mpv reports the first frame, so there is no black flash.
  **Closing**: the window is hidden before the page changes.
- **The position bar glides.** The backend reports position four times a second; between reports the bar
  runs on at the playback speed, at most `GLIDE_MAX_S` (0.5 s) past the last report and never past the end
  (`src/features/player/intents.ts` `glide`). A stall therefore still shows as a stall.
- **Budget**: input to visible response under 100 ms. Measured at ~14 ms in the real window.
- **Picture**: brightness, contrast and saturation (−100…100), and the **shape and zoom** the phase
  specifies: Shape cycles as filmed, 16:9, 4:3, 2.35:1 and "fill the screen"; Zoom runs 50–300%.
  `crates/player` `framing` turns them into mpv's `video-aspect-override` (a forced ratio, else `-1`,
  so nothing unexpected is ever passed through), `panscan` (1 for fill: crop rather than stretch) and
  `video-zoom` (a power of two, so a percentage becomes its base-2 logarithm).

**Clicks on the picture.** The picture is mpv's own window above the page, so the page never sees a
click on it. mpv is given one input section (`POINTER_SECTION`, `crates/player/src/player.rs`) that
reports a left click, a double click and mouse movement as script messages; its own bindings stay off.
The app layer forwards them as `player-pointer`: a click pauses or plays, a double click toggles full
screen, movement shows the controls. The bar's volume button says "Mute"; the level is said once, when
it changes.

## 2. Input: one intent list, two devices

Keys and pad buttons both map onto `Intent` (`src/features/player/intents.ts`), so they cannot offer
different things. The pad map (`src/lib/gamepad.ts` `playerIntent`, ADR-0045's TV-standard):

| Button | Does |
|---|---|
| A | play / pause |
| D-pad ← / → | seek 10 s (held: repeats, i.e. scrubs) |
| D-pad ↑ / ↓ | volume ±5 |
| LT / RT | seek 30 s |
| LB / RB | previous / next episode; in a film, previous / next chapter |
| Y | subtitles quick menu |
| X | sound quick menu |
| Menu | the full player panel |
| View | full screen (the player's own while it plays, so it knows; listed in the bar's hints) |
| B | back (closes a menu first, then leaves the player) |

While a quick menu is open, the D-pad moves inside it and does not reach the player (Down never turns the
volume down under an open menu). **A dialog holds the D-pad**: from a control inside a `role="dialog"`,
focus only moves to another control in the same dialog (`moveFocus`); before, Down at the foot of the
player's panel left it for the bar beneath.

**Selects and sliders by pad** (ADR-0041). The app's selects are native `<select>`s; a pad's A clicks
the focused element, and a click from a script does not open a select's list, so no select could be
changed by pad. With a pad, A on a select opens `PadPicker` (`src/app/PadPicker.tsx`): its options as a
list in a dialog, focus on the current one, the D-pad moves, A chooses, B leaves unchanged; the select is
then set as a person choosing would set it (`setControlValue`), so its own `onChange` runs, and focus
returns to it. Left and Right on a focused slider step its value (`adjustValue`), as the arrow keys do;
at either end focus stays and nothing changes. Up and Down still move between controls.

**Chapters** (`intents.ts` `chapterTarget`): with no episode either side, RB goes to the next chapter's
start and LB to the start of the one playing, or to the one before when it began under
`CHAPTER_BACK_S` (3 s) ago, as every player's "previous" does. Each jump is said on screen
("Chapter: The city"); with nowhere to go it says so instead of doing nothing. `SHORTCUTS` is the single table shown by `?`; a test checks each row's
sample key produces the row's intent.

## 3. The right stick scrolls everything

`scrollByStick` (`src/lib/gamepad.ts`) scrolls the scrollable element under focus — a page, a panel, a long
rail, the paused film's details — falling back to the nearest scrollable ancestor or the dialog.

- **Speed by deflection**: nothing inside `SCROLL_FROM` (0.2); then speed rises with the **square** of the
  push past the dead zone, up to `SCROLL_MAX` (2,400 px/s). A gentle push reads a line at a time; a full
  push crosses a page in about a third of a second.
- **Focus follows**: if the focused element scrolls out of view, focus moves to the nearest focusable
  that is really on screen (`onScreen`: inside the window and inside every scrolling box around it),
  within the same dialog if it was in one, so the next D-pad press starts from what the viewer can see.
- **Snapping is suspended while the stick drives**: a rail snaps to its cards, and the stick's few
  pixels a frame each snapped back to where they started, so the stick could not move a rail at all.
  The element's snap is switched off while the stick moves it and restored at rest, when the rail
  settles on the nearest card.
- **Focus that falls to the page** (`rescueFocus`): it falls when the element holding it goes — a
  virtualised rail drops the focused card a frame or two after the stick scrolled it away; a button
  disables itself once pressed ("Add", its field cleared); a form gives way to what it saved ("Save
  key" becomes "Your key is saved"). With the pad driving, focus is put back on the control on screen
  nearest where it was (within its dialog, if it was in one), because the D-pad has nowhere to start
  from the page itself. With the mouse it is left alone (a click put it there), except within
  `FOLLOW_GRACE_MS` (600 ms) of the stick moving something. Never while a film plays (the player owns
  the buttons), and never when the whole screen went (`wentWithItsScreen`: nothing the element sat in
  survives below `main` or the masthead), since the next screen decides where focus starts.

## 4. Subtitles in the player

`src/features/player/SubtitleTools.tsx`; backend `src-tauri/src/commands/subtitles.rs` over
`crates/subtitles`.

- **Style** (size, font, colour, outline, background box, position) is applied to mpv live and saved per
  profile. Every control is a button, so the D-pad reaches all of it.
- **Find**: OpenSubtitles is searched with the profile's own key; offers come back best first (preferred
  language, then those hash-matched to this file, "timed for this file"). Searching spends no download
  budget; only choosing an offer does, and a refused download returns its reservation.
- **Sync**: 0.1 s steps, remembered; "align again" re-runs the aligner (`subtitle-alignment.md`) on an
  external track and says why it cannot for an embedded one.
- **Load a file**: Windows' file picker.
- **By pad**, in the panel: Find, a found subtitle, "Subtitles earlier/later" (0.1 s) and every style
  button. Choosing a found subtitle closes the list it was in; focus is put back within the panel
  (`rescueFocus` acts during playback only inside a dialog still open over the film).

## 5. Home, in the author's order

`crates/taste/src/home.rs`: Continue watching → New episodes of your shows → Your library (newest added
first) → New releases, by taste (this year, with a vote floor) → the taste rails → Watchlist → Widely
loved. Empty rails are left out; "widely loved" needs only the offline catalogue, so Home is never empty.

- A finished library scan emits an event; Home refetches on it, so the library rail appears without a
  reload.
- New releases use `idx_media_items_year_votes` (migration 0027), named in the query: without it the
  planner walked the votes index and the profile switch went over its 100 ms budget.

## 6. A first run that ends ready to watch

`src/features/onboarding/Onboarding.tsx`, seven steps, each saying what it is for, each skippable, none a
dead end: who it is for → what you like → your films on this PC (the scan starts at once) → where films
come from (add-ons **by address**, checked live; the app suggests none, `SPEC.md` §2.1) → subtitles
(OpenSubtitles key, link and live test) → what's showing now (TMDB key, link and live test) → downloads
(built-in engine or the user's own qBittorrent, with the VPN adapter check). Pad: D-pad moves, A chooses,
B goes back a step. Every field is filled by the TV keyboard and the adapter chosen from `PadPicker`.
The footer stays on screen as a step scrolls, so the wizard's scroller carries `scroll-padding-bottom`
(`scroll-pb-32`): `scrollIntoView` then keeps the focused control clear of it. Without it a long step's
last fields sat half under the footer, and Down from them found nothing below. Wherever play or download would be and no source is configured, the app says so in
plain words instead of offering a button that does nothing.

## 7. Watched, saved, and what is left to set up (Appendix F, ADR-0046)

- **Start over** (I01) beside "Resume from …", only when there is a place to resume from.
- **Mark watched** (I21): a film, an episode, a season or a whole show
  (`ProfileRepository::set_watched`, scopes `WatchScope::{Title, Season, Series}`). Watched records one
  completed viewing of each title not already finished, as an import does, so the taste model reads it
  the same way; marking twice records nothing more. Not watched removes every viewing; a rating stays.
  Either way the resume position goes, so the title leaves Continue watching and the film page's
  "Resume from" is dropped at once. Episode rows show "Watched"; the series says "Seen N of M".
- **Watchlist** (I39): "Add to watchlist" on a film's or a show's page; the Watchlist tab is a grid of
  what was saved, newest first, with a designed empty state. Phase 20 deepens it.
- **Finish setting up** (I38): a card at the top of Home while anything is left (library folders, an
  add-on the viewer chose, an OpenSubtitles key, a VPN adapter for downloads); each line opens its
  Settings section with focus on its first control (`src/lib/openSettings.ts`). A built-in source does
  not count as one the viewer chose. The rules are pure (`src/features/home/setup.ts`); hidden per
  profile once dismissed; it waits for every fact before showing, so it never flashes "0 of 4".

## 8. Layout at three sizes

`src/lib/layoutCheck.ts` (and the same rules in the pad-flow audit): the page never scrolls sideways, no
text is cut off in its box, nothing sits off the right edge outside a deliberate scroller. Checked at
1280, 1600 and 1920 wide, with and without TV mode's 125% zoom. The masthead measures whether it fits and
goes compact when it does not — a media query cannot, because TV mode zooms without changing what a media
query sees.

## 9. Testing it

- `node tools/uiaudit/controller.mjs` — the pad flow against the built app with a fake backend: first run
  filled in by pad (a folder path, an add-on address, both keys typed on the TV keyboard; the VPN's adapter
  from its list; each step saying it took), then Home with nothing left to set up, search, film page (mark watched, watchlist), player (Y and X quick menus, the
  bumpers by chapter, seek and hold-to-scrub, every panel control by D-pad), the right stick on a long
  rail, the panel, the paused details and Settings, the Watchlist tab, layout. The fake backend can
  send events (`__emit`), so the player's reports arrive as the real backend's do.
- `npm run audit:ui` — frame budgets, focus rings, reduced motion.
- The real window: debug build with `SINEPHILE_AUTOPLAY` and `SINEPHILE_PROBE` (timed intents as the pad
  sends them), video-window region read from Windows (`GetWindowRgn`). PrintWindow cannot see through the
  holes, so the region is the evidence. With any probe script the page also times a finished library scan
  to the first card of "Your library" on Home (E6); run it with `SINEPHILE_DATA_DIR` on a copy of the
  database, never the original.
- Not automatable (`docs/MANUAL_TESTS.md`): a real controller in hand, Windows' file picker, a real
  download and stream from a source the author chose.
