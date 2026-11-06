# Phase 11 — Player Experience Layer

**Status:** in_progress · **Depends on:** 8, 9, 10 · **Sessions:** 2

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
- [ ] **E4** All player features are keyboard-accessible and documented in a shortcuts overlay (`?`).
- [ ] **E5** Manual test plan for playback passes on all three tiers.
- [ ] **E6** With only a controller, every text field in the app can be filled from the on-screen keyboard, and its suggestions, recent searches and word completion work (ADR-0043).

## Subtasks

- [x] **11.1** Series and airing awareness (ADR-0030): migration 0019 airing_schedule filled from AniList nextAiringEpisode during anime ingest/refresh; EpisodeRepository lists a series' seasons and episodes and its next airing; the detail page of a series shows seasons, episodes (playable) and 'Episode 7 airs Thursday' even with no episode row (a pure, tested wording function).
- [x] **11.2** The pause overlay: on pause, over the dimmed still, a panel with the cast (character names; headshots only with the user's TMDB key, else typographic), director and key crew, ratings that exist (IMDb from the catalogue; AniList for anime) and never Rotten Tomatoes, time remaining, the current chapter, 'more like this' from the vector index; a cast member opens their filmography. Data fetched when playback starts so the overlay paints in < 200 ms (measured in the real window). Trivia, goofs and soundtrack have no free legal source and are said to be absent, not scraped.
- [x] **11.3** Episodes in the player: previous/next episode (N/P keys, pad), a next-episode card with a countdown in the last 30 s or at a credits chapter, autoplay on by default and switchable in Settings -> Playback; a pure, tested function decides the next episode across season boundaries.
- [x] **11.4** The scrub bar: chapter marks from mpv's chapter list; hover thumbnails from FFmpeg sprites generated in a low-priority background process (one frame per 10 s, cached under data/cache/thumbs by file hash), proven not to stutter playback (mpv frame-drop-count unchanged while generating).
- [ ] **11.5** Player controls: picture-in-picture (small always-on-top window, Ctrl+P), audio delay (Ctrl+[ / Ctrl+]), brightness/contrast/saturation in a Picture menu, a stats-for-nerds overlay (I: decode path, dropped frames, bitrate, cache, swarm peers/speed), and a shortcuts overlay (?) generated from the same key map the player uses.
- [ ] **11.7** The TV keyboard (ADR-0043): the Netflix-TV grid (a-z/0-9 left, results live beside it, space and backspace below) on every text field, Xbox-standard buttons (A type, B close, X backspace, Y space, LT/RT cursor, LB/RB letters/symbols incl. case, Menu submit), title suggestions and word completion from the catalogue, recent searches; a voice key placeholder for Phase 20; the controller flow proves an API key and a search can be typed with the pad alone.
- [ ] **11.6** docs/MANUAL_TESTS.md playback plan run on all three tiers via the tier override (screenshots per tier), docs/specs/player-experience.md, learning note, evidence for E1-E5, regression check, close.

## Learning note

Why the overlay data is available but scene-level data isn't; background work without blocking playback; how thumbnail sprites work.

---

<!-- subtask log: appended during the phase -->

## Work log
- **11.1** Series and airing awareness (ADR-0030): migration 0019 airing_schedule filled from AniList nextA · `2794a42`
- **11.2** The pause overlay: on pause, over the dimmed still, a panel with the cast (character names; head · `5b55e4c`
- **11.3** Episodes in the player: previous/next episode (N/P keys, pad), a next-episode card with a countd · `1369b57`
- **11.4** The scrub bar: chapter marks from mpv's chapter list; hover thumbnails from FFmpeg sprites gener · `f2bf342`
