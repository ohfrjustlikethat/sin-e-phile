# Phase 16 — Learning notes

**A person can set the app up once and watch something they chose, in a player and an interface that
feel finished.** This phase was inserted (ADR-0045) because the release build was unusable though every
test passed. Four sections per ADR-0016 (A4).

---

## 1. What we built

- **TV shows from folders** — `Show (Year)/Season N/…`, flat `S01E03` files, anime per season, one show
  split across drives merged; extras left out. 184-file layout corpus, no episode filed as a film.
- **A catalogue that keeps itself current** — hourly check from launch, IMDb refresh when the published
  files change, AniList airing every few hours, TMDB "showing now" for a profile with a key.
- **Home in the author's order** — continue, new episodes, your library, new releases, taste rails.
- **The controller in the player** — TV-standard buttons; the right stick scrolls whatever is under focus.
- **The hybrid player** — clean while watching, a full panel one button away, ~14 ms to respond.
- **Subtitles in the player** — live style per profile, find and choose from OpenSubtitles, 0.1 s sync,
  align again, load a file.
- **A first run that ends ready to watch**, and honest "no source yet" states.
- **SETUP.md**, and a layout check at three window sizes.
- **The improvement backlog's first seven** (SPEC.md Appendix F): Start over, mark watched (a film, an
  episode, a season, a show), a working Watchlist page, a "Finish setting up" card on Home, and three
  visual fixes. Also the picture's shape and zoom, and the bumpers moving by chapter in a film.
- **Three controller bugs the audit found once it looked**: the right stick could not move a rail
  (scroll snapping undid every step), focus was lost when a rail unmounted the focused card, and Down
  at the foot of a dialog left it.
- **A first run filled in by pad alone** (E7), which found four more: a pad could not change any
  select (A "clicked" it, and a click from a script does not open its list) or any slider (Left and
  Right moved off it); a button that disabled itself once pressed dropped focus onto the page; and a
  long step's last fields sat half under the sticky footer, where Down could not leave them.
- **The author's second library, read properly** (2026-11-16): `Ep. 01`, `Chapter 01`, year ranges,
  shelf-order prefixes and `Extra 1`; guesses kept off Home; a show's own files listed in order on its
  page; Films and TV Shows list what is on this PC. And in the player: click to pause, View for full
  screen, one volume readout; Search from any page.
- **How long a finished scan takes to show on Home** (E6): 38 ms for 200 films, measured in the real
  app on a copy of the catalogue.

## 2. Why

- **Passing tests are not a usable app.** Fixtures test what someone thought to test. The author's real
  TV folder, real display scale and real controller found faults no fixture had (Twin Peaks filed as
  films, a menu opening invisibly under the video, the masthead pushing the window buttons off screen).
- **The folder names the show.** People organise TV by folder; a file called `01.mkv` means nothing alone.
  Reading the nearest naming folder first means an episode is never mistaken for a film.
- **An audit only proves what it walks.** The pad flow passed for weeks while the right stick could not
  move a single rail: no step had pushed the stick on one. Each new check was made to fail first.
- **Measure before explaining.** The first theory for the dead pad in playback (the page loses focus
  behind the video) was wrong; a probe showed the page was focused and the faults were ours.
- **Background work must never be felt.** The refresh only downloads when the publisher's validator
  changed, so most launches cost one small request; it never blocks a command.
- **One list of intents.** Keyboard and pad both map onto `Intent`, so neither can offer what the other
  cannot.
- **Response time is what makes controls feel solid.** Under 100 ms reads as instant; a bar that moves in
  quarter-second steps reads as broken even when it is right, so it glides between reports.

## 3. New concepts

- **Input latency, measured end to end** (intent logged → Windows region changed) — `docs/eval-results.md` "The player, measured in its real window"; region code `crates/player/src/host.rs:22`.
- **Window regions with holes** (`CombineRgn` with `RGN_DIFF`, `SetWindowRgn`) — `crates/player/src/host.rs:22`; holes from `data-cutout` — `src/features/player/PlayerScreen.tsx:261`.
- **Dead-reckoning between reports, capped** — `src/features/player/intents.ts:199` `glide`, cap `:191`.
- **A dead zone and a squared response curve** — `src/lib/gamepad.ts:54` `scrollSpeed`.
- **Focus following scrolled content** — `src/lib/gamepad.ts:63` `nearestBox`, `:413` `scrollByStick`.
- **Key repeat: delay, then rate** (hold to scrub) — `src/lib/gamepad.ts:272` `shouldRepeat`.
- **Letting an open menu claim the buttons** — `src/lib/gamepad.ts:574` `claimButtons`.
- **One mapping table per device onto shared intents** — `src/lib/gamepad.ts:115` `playerIntent`.
- **Reading meaning from the folder path** — `crates/library/src/folders.rs:96` `hint`, `:15` `season_of`, `:81` `is_extra`.
- **HTTP validators (ETag / Last-Modified) to skip work** — `crates/catalogue/src/freshness.rs:74` `check`, `:254` `current_validator`.
- **A background loop started once the app is ready** — `src-tauri/src/commands/catalogue.rs:98` `keep_current`.
- **Forcing the query planner's index** (`INDEXED BY`) — `crates/persistence/src/repositories/media.rs:378`; migration `crates/persistence/migrations/0027_new_releases_index.up.sql:5`.
- **Refetching on a backend event instead of polling** — `src/features/home/HomeScreen.tsx:43`.
- **Measuring whether something fits, instead of a breakpoint** — `src/app/Masthead.tsx:68` (ResizeObserver at `:80`).
- **A layout check with a control that must fail** — `src/lib/layoutCheck.ts:7`.
- **mpv's picture properties: aspect override, panscan, zoom as a power of two** — `crates/player/src/player.rs:676` `framing`.
- **Chapter navigation with a "restart this one first" threshold** — `src/features/player/intents.ts:217` `chapterTarget`.
- **Focus trapped in a dialog, preferred in a region** — `src/lib/gamepad.ts:297` `moveFocus` (`trapped`, `:308`); the follow too, `:508`.
- **CSS scroll snapping fighting a programmatic scroll, and suspending it** — `src/lib/gamepad.ts:395` `suspendSnap`, `:403` `releaseSnap`.
- **Focus that falls to the page, put back near where it was** (a card unmounted, a button disabled, a form replaced) — `src/lib/gamepad.ts:529` `rescueFocus`, `:448` `noteFocus`, `:472` `FOLLOW_GRACE_MS`.
- **Telling a control that went from a screen that went** (the nearest surviving ancestor) — `src/lib/gamepad.ts:465` `wentWithItsScreen`.
- **"Visible" meaning clipped by every scrolling ancestor, not just inside the window** — `src/lib/gamepad.ts:486` `onScreen`.
- **Setting a React-controlled input from outside React** (the prototype's value setter, then the event) — `src/lib/gamepad.ts:331` `setControlValue`.
- **A native select a pad cannot open, and a list that stands in for it** — `src/app/PadPicker.tsx:36` `PadPicker`, opened from `src/app/App.tsx:130`.
- **A slider stepped by Left and Right instead of being left** — `src/lib/gamepad.ts:344` `adjustValue`, `src/app/App.tsx:120`.
- **`scroll-padding` keeping focus clear of a sticky footer** (`scrollIntoView` honours it) — `src/features/onboarding/Onboarding.tsx:111`.
- **Timing across an event in the page's own clock** (scan done to the rail drawn) — `src/features/player/useDropToPlay.ts` `probe`.
- **One SQL statement per scope, chosen by an enum** — `crates/persistence/src/repositories/profiles.rs:523` `set_watched`, `:62` `WatchScope`.
- **A transaction around a write and its cleanup** — `crates/persistence/src/repositories/profiles.rs:544` (`begin` / `commit`).
- **Invalidating several cached queries after one change** — `src/features/detail/Watched.tsx:20` `useSetWatched`.
- **Pure rules behind a component, tested without a page** — `src/features/home/setup.ts:35` `setupLines`, `setup.test.ts`.
- **A fake backend that can send events** — `tools/uiaudit/controller.mjs:180` `__emit`, `:189` `transformCallback`.
- **Testing focus without layout: placing elements by hand in jsdom** — `src/lib/gamepad.test.ts` `place`, `scroller`.
- **Asking a native window to report input it would otherwise swallow** (an mpv input section of script messages, read as client-message events) — `crates/player/src/player.rs:159` `POINTER_SECTION`, `:665` `pointer_message`, `:239` `on_pointer`.
- **One threshold, kept beside every query that must honour it** — `crates/persistence/src/lib.rs:23` `MATCH_ACCEPTED_AT`.
- **Sorting with "no number" last** (`None` as `u32::MAX` in the key) — `crates/library/src/shelf.rs:22` `in_order`.
- **A pattern that means two things, told apart by context** ("Chapter 2" with a year is a title) — `crates/filename-parser/src/lib.rs` `parse`; a year range, `crates/filename-parser/src/tags.rs:597`.
- **A prefix recognised by its punctuation, not its digits** — `crates/library/src/folders.rs:105` `without_shelf_order`.

## 4. Five questions

1. The backend reports the playback position four times a second. Why does the bar not simply draw the
   last report, and why is `glide` capped at half a second past it?
2. The right stick's speed rises with the *square* of how far it is pushed. What would a linear curve
   feel like at a gentle push, and what is the dead zone for?
3. Under `D:\TV\Twin Peaks (1990)\` there is `Season 2\Episode S02E08.mkv` and
   `Featurettes\Behind the Scenes\Frank Byers.mkv`. Walk through what the library makes of each, and why
   neither is ever searched for as a film.
4. The video is a separate native window above the page. How does a menu on the page become visible
   above the picture, and what went wrong when the subtitles menu lacked `data-cutout`?
5. Most launches check whether the catalogue is out of date without downloading it. What does the app
   ask the server, and why is "could not tell" treated as "do nothing" rather than "refresh"?
