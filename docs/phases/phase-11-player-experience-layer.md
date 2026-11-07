# Phase 11 — Player Experience Layer

**Status:** complete · **Depends on:** 8, 9, 10 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The player becomes better than the commercial ones.

## Deliverables

**Airing awareness** (ADR-0030): a series page must be able to show "S03E07 airs Thursday" **without an episode row existing**, because AniList knows an episode is scheduled before IMDb lists it. Next-episode UI reads from the schedule as well as the catalogue. **The pause overlay** — the signature feature. On pause, a Prime-X-Ray-inspired panel slides in **from the right** over a dimmed, blurred frame, the frame staying visible on the left and the D-pad moving into the panel (ADR-0043): full cast with headshots, director and key crew, ratings (IMDb, TMDB, and AniList for anime — **not** Rotten Tomatoes, which has no free API and whose scraping would violate its terms), runtime remaining, the current chapter, trivia and goofs, soundtrack, and "more like this". Clicking a cast member opens their filmography. It must be beautiful — this is the screenshot people will remember. (Face recognition arrives in Phase 22; until then it shows the full cast list, well-designed.) Next-episode handling with a countdown and a preview card. **When a film ends**, the credits play in full, then a calm end card: what you watched, rate it, what next (ADR-0043); a series keeps the countdown. **An in-app mini player**: keep watching in a corner while browsing, separate from Windows picture-in-picture (ADR-0043). **The TV keyboard** (ADR-0043): on every text field in the app, a Netflix-TV grid (a–z / 0–9 left, results live beside it) with title suggestions, recent searches and word completion from the catalogue; A types, B closes, X backspaces, Y types a space, LT/RT move the cursor, LB/RB switch letters and symbols, Menu submits; a voice key arrives with Phase 20. Episode navigation within the player. Chapter markers on the scrub bar. Thumbnail previews on scrub-bar hover, generated in the background via FFmpeg and cached. Picture-in-picture. Audio delay control. Video filter controls (brightness, contrast, saturation) for badly-encoded sources. Statistics-for-nerds overlay showing decode path, dropped frames, bitrate, and swarm state.

## Exit criteria

- [x] **E1** The pause overlay is visually excellent and appears in < 200 ms.
- [x] **E2** Scrub thumbnails generate without stuttering playback.
- [x] **E3** Next-episode autoplay works and is disableable.
- [x] **E4** All player features are keyboard-accessible and documented in a shortcuts overlay (`?`).
- [x] **E5** Manual test plan for playback passes on all three tiers.
- [x] **E6** With only a controller, every text field in the app can be filled from the on-screen keyboard, and its suggestions, recent searches and word completion work (ADR-0043).

## Subtasks

- [x] **11.1** Series and airing awareness (ADR-0030): migration 0019 airing_schedule filled from AniList nextAiringEpisode during anime ingest/refresh; EpisodeRepository lists a series' seasons and episodes and its next airing; the detail page of a series shows seasons, episodes (playable) and 'Episode 7 airs Thursday' even with no episode row (a pure, tested wording function).
- [x] **11.2** The pause overlay: on pause, over the dimmed still, a panel with the cast (character names; headshots only with the user's TMDB key, else typographic), director and key crew, ratings that exist (IMDb from the catalogue; AniList for anime) and never Rotten Tomatoes, time remaining, the current chapter, 'more like this' from the vector index; a cast member opens their filmography. Data fetched when playback starts so the overlay paints in < 200 ms (measured in the real window). Trivia, goofs and soundtrack have no free legal source and are said to be absent, not scraped.
- [x] **11.3** Episodes in the player: previous/next episode (N/P keys, pad), a next-episode card with a countdown in the last 30 s or at a credits chapter, autoplay on by default and switchable in Settings -> Playback; a pure, tested function decides the next episode across season boundaries.
- [x] **11.4** The scrub bar: chapter marks from mpv's chapter list; hover thumbnails from FFmpeg sprites generated in a low-priority background process (one frame per 10 s, cached under data/cache/thumbs by file hash), proven not to stutter playback (mpv frame-drop-count unchanged while generating).
- [x] **11.5** Player controls: picture-in-picture (small always-on-top window, Ctrl+P), audio delay (Ctrl+[ / Ctrl+]), brightness/contrast/saturation in a Picture menu, a stats-for-nerds overlay (I: decode path, dropped frames, bitrate, cache, swarm peers/speed), and a shortcuts overlay (?) generated from the same key map the player uses.
- [x] **11.7** The TV keyboard (ADR-0043): the Netflix-TV grid (a-z/0-9 left, results live beside it, space and backspace below) on every text field, Xbox-standard buttons (A type, B close, X backspace, Y space, LT/RT cursor, LB/RB letters/symbols incl. case, Menu submit), title suggestions and word completion from the catalogue, recent searches; a voice key placeholder for Phase 20; the controller flow proves an API key and a search can be typed with the pad alone.
- [x] **11.6** docs/MANUAL_TESTS.md playback plan run on all three tiers via the tier override (screenshots per tier), docs/specs/player-experience.md, learning note, evidence for E1-E5, regression check, close.

## Learning note

Why the overlay data is available but scene-level data isn't; background work without blocking playback; how thumbnail sprites work.

---

<!-- subtask log: appended during the phase -->

## Work log
- **11.1** Series and airing awareness (ADR-0030): migration 0019 airing_schedule filled from AniList nextA · `2794a42`
- **11.2** The pause overlay: on pause, over the dimmed still, a panel with the cast (character names; head · `5b55e4c`
- **11.3** Episodes in the player: previous/next episode (N/P keys, pad), a next-episode card with a countd · `1369b57`
- **11.4** The scrub bar: chapter marks from mpv's chapter list; hover thumbnails from FFmpeg sprites gener · `f2bf342`
- **11.5** Player controls: picture-in-picture (small always-on-top window, Ctrl+P), audio delay (Ctrl+[ /  · `656441e`
- **11.7** The TV keyboard (ADR-0043): the Netflix-TV grid (a-z/0-9 left, results live beside it, space and · `6fed80f`
- **11.6** docs/MANUAL_TESTS.md playback plan run on all three tiers via the tier override (screenshots per · `acf5a85`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `cc3ca3c`.

### Evidence per criterion

**E1** — The pause overlay is visually excellent and appears in < 200 ms.

> 5b55e4c: real window 2026-11-06, Nosferatu (1922) streaming from the Internet Archive, Space pressed four times: pause to the overlay's first painted frame (window.__pauseMs, measured from the intent to requestAnimationFrame after layout) = 22, 4, 4, 5 ms (target < 200 ms). Before the change the overlay waited for mpv's still capture (up to 500 ms); the video window is now hidden first. Visually: screenshot shows the right-side panel (ADR-0043) between the player's bars with cast and parts, crew by role, IMDb 7.8 / 125,211 votes, and more like this (Nosferatu the Vampyre, Shadow of the Vampire, The Last Laugh). Tests: PlayerScreen.test.tsx 'the pause panel shows who is in it and who made it, before the still arrives' and 'a cast member opens their films, and a film leaves the player for its page'.

**E2** — Scrub thumbnails generate without stuttering playback.

> f2bf342: real window 2026-11-07, a 20-minute 1080p H.264 file on d3d11va: player_state.dropped_frames over 30 s of playback was 0 -> 0 with thumbnail generation running and 0 -> 0 with the sheets cached; generation (crates/player/src/thumbs.rs: FFmpeg below-normal priority, -threads 1, -skip_frame nokey) made both sheets in 970 ms. docs/eval-results.md Phase 11 E2. Tests: crates/player/tests/playback.rs thumbnail_sheets_are_made_from_a_file; thumbs::tests a_time_finds_its_tile; src/features/player/scrubPreview.test.ts. Screenshot: the 7:25 tile shown above the bar in its own cut-out.

**E3** — Next-episode autoplay works and is disableable.

> 1369b57: src/features/player/PlayerScreen.test.tsx 'autoplay plays the next episode when the countdown ends' (the card shows 'Play in 10' at 30 s left; after ten seconds prepare_play is asked for the next episode) and 'offers the next episode in the last 30 s; B says not now, A plays it' (with autoplay switched off the card offers 'Play' with no countdown, and nothing plays by itself); the switch is Settings -> Playback 'Play the next episode automatically' (src/features/settings/PlaybackSection.tsx). When to offer it: src/features/player/endCard.test.ts (last 30 s, a credits chapter, the end). Next/previous across seasons: crates/persistence/tests/repository_surface.rs episode_repository_surface next_after/previous_before.

**E4** — All player features are keyboard-accessible and documented in a shortcuts overlay (`?`).

> 656441e: every player feature has a key: src/features/player/intents.ts SHORTCUTS (19 rows: play/pause, seeks, volume, mute, full screen, frame step, speed, subtitles, subtitle timing, sound timing, episodes, statistics, mini player, picture-in-picture, shortcuts, back, and the pad's player menu); intents.test.ts 'says what each key really does' turns each row's real key event through keyIntent and must get the row's intent; the '?' overlay renders that same table (PlayerScreen.test.tsx '? lists the shortcuts, I shows statistics, Ctrl+[ moves the sound'). The pad reaches the rest through Menu (PlayerScreen.test.tsx 'Menu opens the player menu; inside it the D-pad moves focus instead of seeking'). Real window 2026-11-07: Ctrl+M mini player and Ctrl+P picture-in-picture (480x270, restored to 1455x909) screenshotted.

**E5** — Manual test plan for playback passes on all three tiers.

> 450cca6: docs/MANUAL_TESTS.md 'Playback, on every tier (Phase 11, E5)' run in the real window 2026-11-07 under each tier (debug SINEPHILE_TIER override; get_hardware_profile reported effective_tier modest/standard/capable, overridden true), with Garden.mkv (8:00, H.264 720p) and a sidecar .srt offset by 82 s, aligned cache cleared before each run. Local file, every tier: first frame, d3d11va; pause overlay painted in 2.2 / 2.3 / 2.4 ms (window.__pauseMs); scrub_thumbs returned sheets; statistics, shortcuts (?), mini player (Ctrl+M out and back), picture-in-picture (742x418 always on top, Ctrl+P restored the exact 2204x1376 inner size at 200,125); 0 ERROR lines. Subtitles: standard and capable logged 'aligned: scale 1.00000, offset -82180 ms, confidence 1.91'; modest logged 'fits the film's length; loaded as it is' (no speech alignment on Tier 0, by design). Stream (the same file over local HTTP, 3 GETs served): first frame 359 / 275 / 272 ms, pause overlay 2.3 / 2.5 / 3.7 ms, statistics shown, 0 ERROR lines. The run found four defects, fixed and tested in 450cca6 before this evidence was taken: Ctrl+M could not leave the mini player, PiP restore used the outer size, a bare file paused into an empty panel, a 5 kbit/s stream read '0.0 Mbit/s'. Screenshots: docs/evidence/phase-11/{modest-paused,standard-stats,capable-mini,capable-pip}.png.

**E6** — With only a controller, every text field in the app can be filled from the on-screen keyboard, and its suggestions, recent searches and word completion work (ADR-0043).

> 6fed80f: tools/uiaudit/controller.mjs (CI step 'Controller flow (simulated Xbox pad)'), pad only: Y opens search and the keyboard, 'sunrise' typed with D-pad and A; Menu opens Settings, the D-pad walks to the OpenSubtitles API key field 'without a keyboard popping up on the way', A opens the keyboard, RB/LB switch case and 'Ab9' is typed, X deletes, Menu closes with focus kept on the field. Typing aids: src/app/PadKeyboard.test.tsx 'on search: recent searches first, then suggestions and word completion' and 'types upper case and symbols, for API keys and addresses'; src/app/keyboardText.test.ts (cursor editing, completion, recents).

### Next phase starts by

Phase 12 subtask 12.1 (docs/phases/phase-12-local-library-engine.md): crates/filename-parser/src/lib.rs parse(name) -> Parsed; tokenise on separators, take bracketed group and tags first, then SxxEyy / 1x02 / ranges / " - NNN" absolute, year as the last plausible 19xx/20xx before the tags, title as the text before the first tag; fixtures/filenames/names.tsv hand-labelled, tests/corpus.rs per-field accuracy.
