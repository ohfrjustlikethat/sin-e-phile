# Phase 16 — Ready to Watch

**Status:** in_progress · **Depends on:** 15 · **Sessions:** 3–4

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A person can install the app, set it up once, and watch anything they choose (from their own library, a stream, or a download) in a player and an interface that feel finished. This phase exists because the author used the release build after Phase 15 and found it was not usable yet, though every test passed (ADR-0045). It is judged by using the app, not only by fixtures.

## Deliverables

**The hybrid player** (ADR-0045): clean while watching, like Netflix or Apple TV; a full panel one button away with VLC's depth: subtitles, subtitle style, subtitle sync, audio, speed, picture (aspect, zoom, brightness). Controls answer in under 100 ms. Opening, closing, the bar and the panels move without jumps or flicker. **Subtitles in the player**: a style editor (size, font, colour, outline, background box, position) previewed live over the picture and kept per profile; find and download from OpenSubtitles without leaving playback (the profile's own key); sync in 0.1 s steps with "align again"; load a file from disk. **The controller in the player** (TV-standard): A play/pause; left/right seek 10 s, hold to scrub; up/down volume; LB/RB previous/next episode or chapter; Y a subtitles quick menu; X an audio quick menu; Menu the full panel; B back. **The right stick scrolls everything**: the page or panel under focus, a long rail, the paused film's details, at a speed set by how far it is pushed, with focus following what scrolls into view. **TV shows from folders**: `Show (Year)/Season N/episodes`, `Show (Year)/` with flat `S01E03` files, anime absolute numbering mapped per season where the data allows (never cumulated, ADR-0031), and one show split across folders or drives merged into one; the folder names the show, so no episode is ever filed as a film. **The catalogue keeps itself current** (ADR-0030 made automatic): once a day, in the background and while idle, new titles and episodes from IMDb's updated files; AniList airing schedules every few hours; TMDB now-playing and airing for a profile with a key; Settings says when it last ran. **Home, in this order**: Continue watching; New episodes of your shows; Your library (shown the moment a scan finishes); New releases, filtered by taste; then the taste rails. **A first run that leaves the viewer ready to watch**: library folders (the scan starts at once); add-ons by address, checked live (does it answer, what does it offer); the app suggests none (§2.1); optional TMDB and OpenSubtitles keys, each with a link to get one and a live test; where downloads go (built-in engine or the user's own qBittorrent) with the VPN check. When no stream source is configured, the app says so plainly wherever play or download would be. **`SETUP.md`**: every prerequisite with its official download link and how to add it to the app (WebView2, FFmpeg, the model files, the IMDb and MovieLens datasets, the keys, an optional qBittorrent, an optional VPN) and how to add any add-on by its address. It names no content source. **Polish**: Settings section headings made clear and larger in the display face; the taste screen with its example films as cards, a clearer hierarchy, a live preview of Home after a correction, fewer words; first run and Home audited for layout, clipping, focus jumps and content flashing, at three window sizes.

## Exit criteria

- [ ] **E1** The player is hybrid as specified, every panel option is reachable by pad, controls respond in under 100 ms of input, and opening and closing show no flicker, all measured in the UI audit.
- [ ] **E2** Subtitles: the style editor applies live and persists per profile; find and download, sync in 0.1 s steps, and load from disk each work in the player, by pad.
- [ ] **E3** The controller works in the player as specified, and the right stick scrolls pages, rails, panels and the paused details, each covered by the pad-flow audit.
- [x] **E4** A TV-layout corpus (season folders, flat, anime absolute, split across drives, including a real-world case like the author's) is identified with at least 98% of episodes right and no episode filed as a film.
- [x] **E5** The catalogue refreshes daily in the background: titles and episodes released since the last refresh appear without the user doing anything, and a run on the real data adds the recent releases it lacked.
- [ ] **E6** Home's rails are in the specified order, and a finished library scan appears on Home within 2 s.
- [ ] **E7** A first run done only by pad leaves library folders, an add-on the user chose, keys and a download backend configured and tested; with no source, the app says so wherever play or download would be.
- [ ] **E8** `SETUP.md` lists every prerequisite with its official link and how to add it, and passes the posture guard.
- [ ] **E9** The author signs off a screenshot set of the player, first run, Home, Settings and the taste screen, and confirms a download and a stream work with a source of their choosing.

## Subtasks

- [x] **16.1** TV shows from folders (crates/library): the show from its folder (`Show (Year)`), the season from a `Season N` subfolder, the episode from the file; flat `S01E03` files under a show folder; anime absolute numbers mapped per season where the data allows (never cumulated, ADR-0031); one show across folders or drives merged; no episode ever filed as a film. A TV-layout corpus in fixtures/ including a Twin Peaks-like case, judged by eval identify (E4); then the author's real folder rescanned on the release build.
- [x] **16.2** The catalogue refreshes itself (ADR-0030 made automatic): a background job once a day while idle fetches IMDb's updated basics, ratings and episode files and adds titles and episodes past what is stored; AniList airing every few hours; TMDB now-playing and airing for a profile with a key; Settings says when it last ran and offers 'refresh now' (E5).
- [x] **16.3** Home in the specified order: Continue watching; New episodes of your shows; Your library (newest added first, shown the moment a scan finishes, by event); New releases filtered by taste; then the taste rails (E6).
- [x] **16.4** The controller in the player (TV-standard: A, left/right seek with hold to scrub, up/down volume, LB/RB episode or chapter, Y subtitles quick menu, X audio quick menu, Menu panel, B back) and the right stick scrolling everything: page, panel, rail, the paused details, speed by deflection, focus following (E3).
- [x] **16.5** The hybrid player: clean while watching; the full panel (subtitles, style, sync, audio, speed, picture); controls answering in under 100 ms; opening, closing, bar and panels without jumps or flicker, measured in the UI audit (E1).
- [x] **16.6** Subtitles in the player: a live style editor kept per profile; find and download from OpenSubtitles in playback; sync in 0.1 s steps with 'align again'; load a file from disk; all by pad (E2).
- [ ] **16.7** A first run that leaves the viewer ready to watch: library folders (scan starts at once), add-ons by address with a live check (the app suggests none, §2.1), TMDB and OpenSubtitles keys with links and live tests, the download backend with the VPN check; honest no-source states wherever play or download would be (E7).
- [ ] **16.8** SETUP.md: every prerequisite with its official link and how to add it to the app, and how to add any add-on by its address; names no content source; passes the guard (E8).
- [ ] **16.9** Polish: Settings section headings clear and larger in the display face; the taste screen (films as cards, clearer hierarchy, live Home preview, fewer words); first run and Home audited for layout, clipping, focus jumps and content flashes at three window sizes, with automated checks added to the UI audit.
- [ ] **16.10** The author batch: sign-off on a screenshot set (player, first run, Home, Settings, taste screen; Phase 14 E4/E6 with it), a download and a stream with a source of their choosing (Phase 7 E7's VPN cut with it); docs/specs, learning note, README and HOW_IT_WORKS, regression, close (E9).

## Learning note

Input latency and how to measure it; spatial scrolling with an analogue stick; how a TV library is laid out and why the folder names the show; scheduling background work without hurting the foreground; why tests passing is not the same as the app being usable.

---

<!-- subtask log: appended during the phase -->

## Work log
- **16.1** TV shows from folders (crates/library): the show from its folder (`Show (Year)`), the season fro · `cabf03d`
- **16.2** The catalogue refreshes itself (ADR-0030 made automatic): a background job once a day while idle · `345e392`
- **16.3** Home in the specified order: Continue watching; New episodes of your shows; Your library (newest · `394b11e`
- **16.4** The controller in the player (TV-standard: A, left/right seek with hold to scrub, up/down volume · `0367901`
- **16.5** The hybrid player: clean while watching; the full panel (subtitles, style, sync, audio, speed, p · `989c481`
- **16.6** Subtitles in the player: a live style editor kept per profile; find and download from OpenSubtit · `2416ab6`
