# The author's brief, 2026-11-17

The author tested the installed release (fresh data folder) and asked for every concern to be fixed
"to an almost perfect degree" before moving on, with questions asked and the answers kept. This file is
the record: what they reported, what was found, and what they chose. Later sessions build to it.

## What they reported

1. Download gaps: season packs download whole (fix first); web links cannot be downloaded.
2. Installed add-ons are not visible anywhere ("disappears into thin air").
3. Streams must be listed per add-on and selectable, like Stremio, in sin-e-phile's design.
4. No shows or movies; search not working; catalogue not updated.
5. Leaving a page for another tab and coming back resets it to the tab's default.
6. Clearer navigation; a back arrow on every screen that can be left; arrows more prominent.
7. Remove the "stream or download" advisor; keep both actions.
8. In the player, clicking the picture still does not pause or play.
9. "Add to watchlist" and "Mark as watched" more prominent.
10. Buggy in full screen and when maximised.
11. Onboarding could be better; a profile picture in any image format, cropped automatically, shown next to
    the Downloads button.
12. The local library is still broken.
13. Many smaller bugs and experience issues.

Items that belong to a later phase go into `SPEC.md` for that phase; everything else is fixed now.

## What was found (2026-11-17, from the installed copy's log and database, read only)

- **A fresh install never builds the search index**: `search_index`, `search_trigram` and `search_indexed`
  hold 0 rows (2.7 million on the development copy, which was built by hand). Search finds nothing; library
  identification, which searches the catalogue, sent all 200 files to the review queue; Films and TV Shows,
  which list identified files, stayed empty. This one gap explains reports 4 and 12.
- The first library scan failed with "database is locked": it ran while the catalogue was being written.
- A fresh install also never fetches alternate titles (`titles` 2.9 M against 6.2 M), AniList's anime
  (0 against 7,445), or the search-by-meaning model.
- Nothing on screen showed the catalogue being built.
- Streaming through a Stremio add-on worked (first frame in 6.3 s) and downloads started.

## Answers

### Round 1: streams and the first run

- **Stream list: behind a "Choose stream" button.** Play plays the best stream at once; "Choose stream"
  slides a panel in from the right with the full list. (Not always on the page; not Stremio's
  choose-every-time.)
- **Grouping: by add-on, best first**, one section per add-on in the order set in Settings.
- **Each row shows all of**: resolution, HDR and codec as badges; size and seeders ("direct" for web links);
  the full release name in small grey text; audio and subtitle language badges when the stream says them.
- **First run: progress during onboarding.** The setup steps run while the catalogue builds, with a progress
  bar; when setup ends, Home opens only once search works; if it is still building, a "Getting ready… 72%"
  screen.

### Round 2: navigation and the title page

- **Tab memory: each tab remembers its place.** Every masthead tab (Home, Search, Films, TV Shows, Watchlist)
  keeps its own trail; coming back shows the page that was left, scrolled where it was. Pressing the tab
  already showing goes to its start.
- **Back arrow: top-left of the page, with a label**, just under the masthead, saying where it goes back to
  ("← Search results", "← Home"). On every screen that can be left: a title page, Settings, Downloads, the
  stream panel. Large.
- **Title page buttons: big round icon buttons with a word under each (TV style).** Play stays one wide
  button; under it, large circles: Download, Watchlist, Watched, Streams (Streams opens the "Choose stream"
  panel of round 1). Watchlist and Watched show their state (filled when on).
- **Downloading: both ways.** The round Download button downloads the best stream; in the stream panel each
  torrent row has its own download icon, so the exact stream chosen can be downloaded. The "stream or
  download" advisor sentence is removed.

### Round 3: player, window, profile, first run

- **Full screen and maximised: controls hidden or misplaced** (the bar, menus or panels cut off, under the
  video, or in the wrong spot). The picture itself, the app layout and flicker were not reported.
- **Clicking the picture: click pauses or plays; double-click toggles full screen**; moving the mouse shows
  the controls, which fade after a few seconds. (The 2026-11-16 fix did not work in the installed app.)
- **Profile picture: a rounded square, cropped automatically, then adjustable** in a small editor (drag and
  zoom) before saving. Any image format. Shown next to the Downloads button and in the profile picker.
- **First run should include**: profile first (name and picture, so it feels theirs from the first screen);
  a clear add-on step (Stremio add-ons explained in plain words, several can be pasted, the added ones
  listed, none ever suggested); more visual polish (big type, motion, imagery from their taste picks, a
  progress strip for the catalogue build). Not chosen: merging or cutting steps.

### Round 4: add-ons, Films and TV, search, look

- **Add-ons: Settings → Add-ons, as cards.** One card per installed add-on: logo, name, version, what it
  serves, on/off, move up/down (the order its streams are listed in), configure (opens the add-on's own
  page), remove. The paste box sits with them.
- **Films and TV Shows: the library on top, the catalogue below.** "On this PC" first, then rails from the
  whole catalogue (popular, new, by genre) so the tabs are never empty; any title opens its page with streams
  from the add-ons.
- **Search by meaning: downloaded during setup**, alongside the catalogue build, with its size shown.
- **Look: sin-e-phile's own** editorial style (oxblood accent, display type, quiet panels, the TV focus ring)
  for the stream panel and the new title page.

### Round 5: search data, best stream, library, other areas

- **Search-by-meaning data: publish it as a GitHub release** (`embeddings-v3`), as versions 1 and 2 were, so
  setup can download it on every PC. The author's decision on the reserved question (ADR-0040, ADR-0044,
  ADR-0014), taken knowing the cost: a public file shaped by IMDb's personal-use data.
- **"Best" stream: a combination of "best quality that streams smoothly" and "my preferred quality".** A
  preferred quality is set in Settings (e.g. 1080p); Play's automatic pick and the order inside each add-on
  take the healthiest stream at that quality (enough seeders to start fast; a web link counts as healthy),
  then the nearest quality above or below that is healthy; 4K only when its swarm is strong.
- **Library, all four reported broken**: shows not grouped as one; wrong titles or years; episodes missing or
  out of order; new files not noticed. (The empty search index explains the first three on a fresh install;
  the fourth needs its own look.)
- **Other areas reported buggy, no specifics yet**: Home and browsing; the Downloads page; Settings;
  controller and TV mode.

### Round 6: what was seen, area by area

- **Home and browsing**: empty or few rails (the empty search index and missing anime on a fresh install are
  part of it); posters missing, cards plain text. They want artwork.
- **Downloads page**: can't find finished files (no way to play, open or locate them from the page).
- **Settings**: all four: too long and hard to find things; confusing wording or options; things don't save
  or apply; looks unfinished. In their words: "adding local library feels like you did nothing, the app
  either doesnt save and refresh or doesnt tell me that it did it. either way after adding an addon or a key
  or connecting local library, the app should tell that something happened, and something ofcourse should
  happen, correctly ofcourse". So: every add, save and connect says plainly that it happened, and the
  result shows at once.
- **Controller and TV mode**: focus jumps or gets lost.

### Round 7: artwork, Settings, confirmations, focus

- **Artwork: both.** Keyless posters by default, opt-in (the poster images Stremio itself uses, looked up by
  IMDb id; one switch in onboarding, which says it sends IMDb ids to that service), and full artwork
  (posters, backdrops, cast) once the profile's own TMDB key is added, which onboarding now explains (free, a
  minute, a link, a live check). Without either, the designed text cards stay.
- **Settings: a side list of sections**: Profile, Library, Add-ons, Playback, Subtitles, Downloads, Network,
  Controller, About; one section at a time on the right, each short.
- **Confirmations: inline and a toast.** The new item appears at once in its list with "Added ✓" (a folder
  with a live "Scanning… 120 of 203"), plus a small toast ("Library folder added: 203 files found"). Errors
  say what to do.
- **Focus jumps or gets lost: everywhere listed**: Home rails; the title page; Settings; and after closing
  something (a menu, panel, keyboard or page), focus is not where it was.

## The plan (decided 2026-11-17, ADR-0040; built in this order)

Everything below is fixed now, in Phase 16, before any later phase starts: each is a bug, or blocks using
the app. Where a later phase already owns a feature, the version built now is the usable core and the later
phase deepens it (recorded in `SPEC.md` for that phase).

1. Downloads: a torrent's chosen file only (season packs) — first, as asked.
2. Downloads: direct web links (the Internet Archive, add-ons that answer with links).
3. A fresh install that works: the search index built at first run (and alternate titles, AniList's anime,
   the search model and the published `embeddings-v3`), the library scan waiting for it, and onboarding
   showing the build's progress; Home opens only when search works.
4. The player: a click pauses or plays, a double click toggles full screen, in the real window.
5. Full screen and maximised: controls never hidden or misplaced.
6. Navigation: each tab keeps its trail and scroll; a large labelled back arrow top-left on every screen that
   can be left.
7. The title page: a wide Play; round Download, Watchlist, Watched and Streams buttons; the advisor removed;
   the stream panel (by add-on, rows with every detail, play and download per row); a preferred quality in
   Settings and the "best" rule above.
8. Settings: the side list; Add-ons as cards; inline and toast confirmations everywhere something is added,
   saved or connected.
9. Films and TV Shows: "On this PC" on top, catalogue rails below.
10. Artwork: keyless posters (opt-in) and TMDB, both offered in onboarding.
11. Profile picture: any image, a rounded square, cropped automatically, adjustable; next to Downloads and in
    the profile picker.
12. Onboarding: profile first; the add-on step; visual polish; the build's progress.
13. Library: new and removed files noticed while the app runs; grouping and order checked on the author's
    folders after item 3.
14. Downloads page: finished files can be played, opened in their folder, and found in the library.
15. Controller focus: Home rails, the title page, Settings, and after anything closes.

## Item 3 in detail: the first run's catalogue (decided 2026-11-17)

The development copy was built by hand with `ingest imdb, credits, akas, normalise, anime, episodes,
movielens, search-index, embed`; a fresh install ran only the hourly refresh (titles, ratings, episodes),
so its search index was empty. The first run becomes a pipeline in two stages, its steps moved out of
`tools/ingest/src/main.rs` into `crates/catalogue` (a `bootstrap` module) so the app and the CLI share them
(ADR-0022):

- **Stage A, what search and identification need** (onboarding shows its progress; Home waits for it):
  titles and ratings, episodes, normalised titles, alternate titles (akas), the FTS5 index and the core
  trigram index. The library scan waits for stage A, then every waiting file is identified again.
- **Stage B, enrichment, in the background after Home opens**: credits (directors on cards), AniList's anime,
  MovieLens (taste), and search by meaning (the published `embeddings-v3`, the model, the vector index).
- Each step is a resumable job (they already are); a restart carries on. Progress goes to the page as an
  event, and Settings shows what is done and what is still coming.

## Status, 2026-11-17: all fifteen done

| # | Item | Commits |
|---|---|---|
| 1 | Season packs: only the chosen episode | 8621e13, 248baa9 |
| 2 | Direct web downloads, resumable | 15781a5 |
| 3 | A fresh install builds its search (0 → 199 of the author's 200 files matched); enrichment after; the meaning index offered in setup; `embeddings-v3` published | 24b545d, a4df39a |
| 4 | Click to pause, double click for full screen, in the real window | 6c8d015 |
| 5 | Full screen from a maximised window | 08b6b90 |
| 6 | Each tab keeps its place; labelled Back; one way back | 5058717 |
| 7 | The title page: wide Play, round buttons, every stream in a panel; "best" by health and preference | 0ad50c7, 56215ed |
| 8 | Settings: side list, add-on cards, every change said | aa398c3 |
| 9 | Films and TV: the library, then the catalogue | 0fc2922 |
| 10 | Posters: TMDB, or the keyless service opted into (ADR-0048) | 08a59f5 |
| 11 | A picture for each profile | f3a6e43 |
| 12 | Onboarding: plainer add-ons, the picks shown back, motion | 4cc59d2 |
| 13 | The library: every file checked on a copy; folders watched even after a failed scan | 7ae1608 |
| 14 | Downloads: a finished file plays, shows in Explorer, opens its page | 46f868c |
| 15 | Controller focus: back on the card opened; Settings keeps its place | 5a160aa |

What later phases deepen is in `SPEC.md` Appendix F, I79–I86 (ADR-0049).

**For the author to try** (what cannot be checked from here): the new installer, on a fresh install (the first run
builds its search in about four minutes after the catalogue's ~23-minute download; onboarding shows it); clicking and
double-clicking the film; choosing a profile picture from your Pictures folder (Windows' own dialog); Show in folder on
a finished download; switching posters on; the whole app with a real Xbox controller.

## Round 8 (2026-11-17, after testing the new installer)

**Reported:** "Manage profiles" opens Settings at About, not Profile (uploading a picture works). The interface broke
("something in the interface broke") every time Streams was opened, on every title; the other time it appeared was
also opening Streams. Continue watching shows a show's single episode ("Episode #1.1" for Twin Peaks) and opens that
episode, not the show. The pause screen does not say which show or episode is playing. In full screen the bars never
hide. Continue watching should also be on Films and TV Shows, each filtered to its own kind. Search by meaning does not
work (found: the install never downloaded it; setup was finished before it was offered). Downloading works.

**Answers:**
- **Continue watching, a show:** one card per show, titled with the show ("Twin Peaks"), with the episode to watch
  under it ("S1 E2 · next", or the unfinished one) and a thin bar for how far into that episode; pressing it opens the
  show's page with that episode ready to resume.
- **Pause screen, an episode:** the show's name large; "Season 1 · Episode 2 — Traces to Nowhere" and the time left;
  the side panel shows the show's cast; a line "Next: S1 E3".
- **The bars:** hide everywhere (window and full screen) after 3 seconds without mouse movement or a controller press,
  the mouse cursor with them; back at once on either; while paused they stay.
- **Search by meaning:** downloaded automatically on new installs, during setup; an existing install (like the author's)
  gets a clear offer instead (the search page and Settings), with its size.
- **Continue watching on Films and TV Shows:** a rail at the very top of each, films only on Films, shows only on TV
  Shows, above "On this PC"; hidden when there is nothing to continue.

### Round 8 status (2026-11-17)

| Report | Fix | Commit |
|---|---|---|
| Streams: "something broke in the interface", every title | One cache key held two shapes; the panel reads Settings' shape. Interface errors now reach the log. | b34ee3f |
| Manage profiles opens at About | Opens at Profile | bb4ecb6 |
| Bars never hide in full screen | A still pointer is not a movement; bars and pointer hide after 3 s, window and full screen | d5d6689 |
| Continue watching shows "Episode #1.1" | One card per show, the episode and a bar; also atop Films and TV Shows; the show's page resumes it | a021a35 |
| Paused, no show or episode | Show, "Season 1 · Episode 2 — name", time left, next, the show's cast | a021a35 |
| Search by meaning does not work | Never worked on any install: the tokenizer's name, and vectors addressed by the producer's row ids. Keyed by IMDb id (ADR-0051); setup fetches it on a new install, an existing one is offered it (ADR-0050) | e140264 |

**For the author to try:** the new installer on your install, then Search: "Download 347 MB" at the top, and a
description such as "a heist where the plan goes wrong" once it is done (the index builds in about 30 s).
