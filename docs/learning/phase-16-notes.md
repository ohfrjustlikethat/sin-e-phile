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

## 2. Why

- **Passing tests are not a usable app.** Fixtures test what someone thought to test. The author's real
  TV folder, real display scale and real controller found faults no fixture had (Twin Peaks filed as
  films, a menu opening invisibly under the video, the masthead pushing the window buttons off screen).
- **The folder names the show.** People organise TV by folder; a file called `01.mkv` means nothing alone.
  Reading the nearest naming folder first means an episode is never mistaken for a film.
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
- **Focus following scrolled content** — `src/lib/gamepad.ts:63` `nearestBox`, `:353` `scrollByStick`.
- **Key repeat: delay, then rate** (hold to scrub) — `src/lib/gamepad.ts:272` `shouldRepeat`.
- **Letting an open menu claim the buttons** — `src/lib/gamepad.ts:423` `claimButtons`.
- **One mapping table per device onto shared intents** — `src/lib/gamepad.ts:115` `playerIntent`.
- **Reading meaning from the folder path** — `crates/library/src/folders.rs:96` `hint`, `:15` `season_of`, `:81` `is_extra`.
- **HTTP validators (ETag / Last-Modified) to skip work** — `crates/catalogue/src/freshness.rs:74` `check`, `:254` `current_validator`.
- **A background loop started once the app is ready** — `src-tauri/src/commands/catalogue.rs:98` `keep_current`.
- **Forcing the query planner's index** (`INDEXED BY`) — `crates/persistence/src/repositories/media.rs:378`; migration `crates/persistence/migrations/0027_new_releases_index.up.sql:5`.
- **Refetching on a backend event instead of polling** — `src/features/home/HomeScreen.tsx:43`.
- **Measuring whether something fits, instead of a breakpoint** — `src/app/Masthead.tsx:68` (ResizeObserver at `:80`).
- **A layout check with a control that must fail** — `src/lib/layoutCheck.ts:7`.

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
