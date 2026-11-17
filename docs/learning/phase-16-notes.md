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
- **Stremio add-ons work** (2026-11-17, ADR-0047): paste a Stremio add-on's address or `stremio://` link; it is
  asked by IMDb id, and its torrents stream through the built-in engine and can be downloaded.
- **The author's second library, read properly** (2026-11-16): `Ep. 01`, `Chapter 01`, year ranges,
  shelf-order prefixes and `Extra 1`; guesses kept off Home; a show's own files listed in order on its
  page; Films and TV Shows list what is on this PC. And in the player: click to pause, View for full
  screen, one volume readout; Search from any page.
- **How long a finished scan takes to show on Home** (E6): 38 ms for 200 films, measured in the real
  app on a copy of the catalogue.
- **Web addresses download directly** (2026-11-17): the Internet Archive and add-ons that answer with
  links, beside the two torrent backends; a pause or a restart carries on from the bytes on disk.
- **A fresh install builds its own search** (2026-11-17): after the first catalogue load, every title is
  normalised and indexed (stage A) while onboarding shows the progress and Home says "Getting ready… N%";
  alternate titles follow in the background; and every refresh's new titles are now made searchable,
  which they never were. The meaning index (`embeddings-v3`) is published for the app to download.
- **Click to pause, for real** (2026-11-17): a click on the film pauses it, a click on the paused picture
  plays it, and a double click goes full screen. Checked with real clicks in the real window; the
  earlier test had only told mpv a key was pressed, and the click never reached mpv at all.
- **Each tab keeps its place** (2026-11-17): the page it was left on and where it was scrolled; pressing the tab
  again goes to its start; a large arrow top-left says where Back goes ("← Search results"); Escape, Backspace
  and B take that step. Specified in `docs/specs/navigation.md`.
- **The title page** (2026-11-17): a wide Play; round Download, Watchlist, Watched and Streams; every stream in a
  panel from the right, by add-on, each with its quality, size, seeders, release name and languages, to play or
  download exactly that one; Play picks the healthiest stream at the preferred quality (a new Settings choice).
  Specified in `docs/specs/title-page.md`.
- **Settings, rebuilt** (2026-11-17): a side list of nine sections, one shown at a time; add-ons as cards with
  their version, what they serve, on/off, order, their own settings page and remove; every add, save and remove
  said on the spot and in a toast, with what actually happened. Specified in `docs/specs/settings.md`.
- **Films and TV Shows are never empty** (2026-11-17): what is on this PC first, then the catalogue: most watched,
  new this year, and the main genres.
- **Posters** (2026-11-17, ADR-0048): with the profile's TMDB key, or with "Show posters" switched on (Stremio's
  poster images by IMDb id, said plainly); fetched once, kept as WebP, shown as titles come into view.
- **A profile picture** (2026-11-17): any picture file, squared automatically (a little high in a portrait), adjusted
  by drag and zoom, kept as a 256 px WebP; shown next to Downloads, in the profile picker and in Settings.
- **The first run, polished** (2026-11-17): a bar of seven segments, larger titles, each step rising in (not under
  reduced motion), the films picked shown back as you go, and the add-on step said in plain words.
- **The library follows its folders, and finished downloads can be found** (2026-11-17): a folder is watched even
  after a failed scan (a new file is in within two seconds); each finished download has Play, Show in folder, and its
  title's page.
- **Round 8** (2026-11-17, after the author tried the installer): Streams no longer crashes once Settings has been
  opened; Manage profiles opens at Profile; the player's bars and pointer hide after 3 s in a window and in full
  screen; Continue watching is one card per show, also atop Films and TV Shows; a show's page resumes its episode;
  the pause screen names the show, the episode and what is next. Search by meaning: a new install fetches it in
  setup (ADR-0050), an existing one is offered it at the top of Search and in Settings; and it could never have
  loaded after a download (the tokenizer's file name), now one name in one place.
- **Focus where it was** (2026-11-17): back from a title page, focus is on the card that opened it; Settings opens on
  its side list (now sticky) without losing its place.
- **Full screen from a maximised window** (2026-11-17): it kept the maximised height, so the player's bar
  floated 80 px up with black beneath; the window is now un-maximised first and maximised again after.
- **Search as one poster grid** (round 9): Home's cards, about eight across, films first among equally good matches,
  each other kind marked; a title's page has a faded still behind its top and the poster on the right (ADR-0052).
- **Search by meaning inside the installer** (round 9, ADR-0053): the author's download was corrupt mid-file (a resume
  answered from the wrong byte), kept, and judged by its header, so the build failed forever without a log line. The
  installer now carries the files; downloads check where a resume starts, delete what fails, and are read in full.
- **A way back from Who's watching?** (round 10): opened from the avatar it says where back goes, as every page does;
  launch's question has none. The arrow and its label are one design-system piece, `BackLink`.
- **Load a file, in the real window** (round 10): the picker offers subtitle files first and a loaded track is named for
  its file. On the way, a first launch's race that could leave no profile the default, and with it break everything kept
  per profile on every later launch, was found and fixed; an install already in that state mends itself.

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
- **Two protocols behind one type, chosen by the data's shape** — `crates/source-protocol/src/addon.rs` `AddonManifest::parse`.
- **Reading an untrusted JSON answer field by field, keeping what is safe and dropping the rest** — `crates/source-protocol/src/stremio.rs` `parse_streams`, `candidate`.
- **A key that carries its own payload, so there is no second request** — `crates/source-protocol/src/stremio.rs` `Key`, `stream_of`.
- **Resuming a download with an HTTP `Range` request, and a `.part` file renamed only when complete** — `crates/downloads/src/direct.rs:140` `fetch_inner`, `:86` `part_of`.
- **One instance shared by everything that is rebuilt around it** (`OnceLock`) — `src-tauri/src/commands/downloads.rs` `DIRECT`.
- **"Done" decided by finished jobs, not by data being present** (a stopped load leaves rows behind) — `crates/catalogue/src/bootstrap.rs:83` `loaded`.
- **Making an incremental index take a row again by removing its "already indexed" mark** — `crates/catalogue/src/bootstrap.rs:240`.
- **Knowing what is owed from the order jobs finished in** — `crates/catalogue/src/bootstrap.rs:116` `catch_up_due`.
- **Work held back by a flag and released exactly once** (`swap(false)`) — `src-tauri/src/commands/catalogue.rs:103` `release_waiting_scans`.
- **Polling only while something is in progress** (`refetchInterval` as a function) — `src/features/onboarding/CatalogueBuild.tsx:38`.
- **Failing open: a status that cannot be read counts as ready** — `src/features/onboarding/CatalogueBuild.tsx:48`.
- **Slow, synchronous work kept off the async runtime** (`tokio::task::spawn_blocking`, and the `??` that unwraps the join and then the work) — `crates/catalogue/src/bootstrap.rs` `fetched`, `credits`.
- **Finding which window a click really lands on** (`WindowFromPoint` under a real `mouse_event`), and answering it in that window's procedure — `crates/player/src/host.rs:101` `proc`, `:105`.
- **Working around a platform quirk in one place, with the measurement that found it** (a maximised window's web view kept its height in full screen) — `src/lib/fullscreen.ts` `setWindowFullscreen`.
- **A trail per tab, and one derived value kept so its readers need not change** (`detail` is the end of the current trail) — `src/lib/store.ts` `goTab`, `back`, `openDetail`.
- **One handler for a key many screens used to handle, acting only on what no one else claimed** (`event.defaultPrevented`) — `src/app/backKey.ts` `handleBack`.
- **A callback ref, because the element itself can be replaced** (a listener bound once stayed on the old `<main>`) — `src/lib/scrollMemory.ts` `useScrollMemory`.
- **A rule added to a lexicographic order, not a weight** ("starts smoothly" before points) — `crates/selection/src/lib.rs` `compare`, `weakness`, `off_preferred`.
- **Rendering into a slot elsewhere in the page** (`createPortal`), so a panel lives inside the app's frame — `src/features/detail/StreamsPanel.tsx`, `src/app/App.tsx` `#panel-slot`.
- **A synthetic key event must be `cancelable` for `preventDefault` to mean anything** — `src/app/App.tsx` the pad's `back`.
- **One small store for app-wide notices, called from anywhere** (`toast(...)`) — `src/lib/toast.ts`.
- **Opening only what the app vouches for** (an add-on's settings page by its id, not an address; `rundll32 url.dll`) — `src-tauri/src/commands/sources.rs` `addon_configure`, `src-tauri/src/shell.rs`.
- **Choosing a query's shape by measuring it on real data** (walk the votes index and check the genre per row; drop the genres that walk too far) — `crates/persistence/src/repositories/media.rs` `most_voted_of`, `crates/taste/src/shelf.rs`.
- **Fetching in the backend so the page's security policy can stay strict** (remote images become `data:` URLs; only `image/*` under 4 MB; misses remembered) — `crates/artwork/src/posters.rs`, `src-tauri/src/commands/artwork.rs`.
- **Loading only what comes into view** (`IntersectionObserver` with a margin) — `src/features/home/TitleCard.tsx` `useNear`.
- **A crop as fractions of the picture, so the page's preview and the original agree** (and two pure functions convert, testable without a browser) — `crates/artwork/src/avatar.rs` `Crop`, `src/features/profiles/PictureEditor.tsx` `cropOf`, `viewOf`.
- **Windows reports a still pointer as moving** when the window under it changes shape; filter by where it is, not
  by the message — `crates/player/src/host.rs` `MoveFilter`.
- **A Tauri command that is not `async` runs on the UI thread**: never take a lock there that a thread waiting on the
  UI thread may hold (it froze the app) — `src-tauri/src/commands/player.rs` `player_cursor`.
- **One cache key, one shape**: two screens storing different data under `["sources"]` crashed the second to read
  it; share the hook instead (`useSeries`) — `src/features/detail/SeriesSection.tsx`.
- **One name, in one place, for a file two programs touch**: the download and the loader each spelled the tokenizer's
  file name, differently — `crates/catalogue/src/assets.rs` `TOKENIZER_FILE`, `tokenizer_path`.
- **Tailwind v4 class names can mean two things**: `text-base` was a colour here, not a size — checked by
  `src/design-system/ambiguousClasses.test.ts`.
- **A test can read the app's own sources through the bundler** (`import.meta.glob` with `?raw`), no Node APIs —
  `src/design-system/ambiguousClasses.test.ts`.
- **What to continue is derived, not stored**: the latest position per show, then the next episode —
  `crates/taste/src/continuing.rs` `resolve`.
- **Remembering focus by meaning, not by element** (`data-focus-key="title-<id>"`: the element is gone when the page comes back, the title is not) — `src/lib/scrollMemory.ts` `refocus`.
- **A double click recognised from two clicks that arrive from different places** (the native window, then the page's still frame) — `src/features/player/PlayerScreen.tsx:531` `clickPicture`.
- **Ordering by tiers with a stable sort**: compare the match tier, then the kind; `Array.prototype.sort` keeps rank
  within a tie — `src/features/search/SearchScreen.tsx` `filmsFirst`.
- **A named D-pad target** (`data-pad-down="<id>"`) when the nearest box is the wrong one: Down from the wide search
  field goes to the first result — `src/features/search/SearchScreen.tsx` `FIRST`, `src/lib/gamepad.ts` `moveFocus`.
- **One function choosing a field by a function pointer**: `fn(Posted) -> Option<String>` picks poster or backdrop
  from one TMDB answer — `crates/metadata-api/src/tmdb.rs` `image_url`.
- **A CSS mask to fade an image's edge** without painting over it — `src/features/detail/DetailScreen.tsx` `Backdrop`.
- **HTTP range requests and `Content-Range`**: a `206` says where its bytes start; trust that, not the request —
  `crates/catalogue/src/download.rs` `range_start`.
- **A header check versus a full check**: cheap enough for a screen, blind to a corrupt middle —
  `crates/catalogue/src/assets.rs` `state_of`, `verified_state`.
- **Tauri resources and a merged release config** (`--config`): files beside the exe, only for the installer —
  `src-tauri/tauri.release.conf.json`, `crates/catalogue/src/assets.rs` `MeaningFiles`.
- **A test HTTP server on a plain thread** (`std::net::TcpListener`), honest or not on purpose —
  `crates/catalogue/tests/download.rs` `serve`.
- **Check-then-act is a race**: two callers both read "none" and both act. One statement (`INSERT … SELECT … WHERE NOT
  EXISTS`) or one transaction makes it a single step — `crates/persistence/src/repositories/profiles.rs`
  `ensure_default`, `create`.
- **A race test must be able to lose**: it passed before the fix until the pool was warmed to eight open connections, so
  the asks really ran side by side — `crates/persistence/tests/profiles.rs`
  `many_first_asks_at_once_leave_one_default_profile`.
- **Focus back to the opener** when a full-screen layer closes, captured before the layer moves focus —
  `src/features/profiles/ProfilePicker.tsx`.
- **A Windows file-type filter** (`COMDLG_FILTERSPEC`), its strings kept alive across the call —
  `src-tauri/src/commands/library.rs` `picker::file`.

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
