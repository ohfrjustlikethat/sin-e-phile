# Phase 11 — Player Experience Layer

**Status:** in_progress · **Depends on:** 8, 9, 10 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The player becomes better than the commercial ones.

## Deliverables

**Airing awareness** (ADR-0030): a series page must be able to show "S03E07 airs Thursday" **without an episode row existing**, because AniList knows an episode is scheduled before IMDb lists it. Next-episode UI reads from the schedule as well as the catalogue. **The pause overlay** — the signature feature. On pause, a Prime-X-Ray-inspired panel slides in over a dimmed, blurred frame: full cast with headshots, director and key crew, ratings (IMDb, TMDB, and AniList for anime — **not** Rotten Tomatoes, which has no free API and whose scraping would violate its terms), runtime remaining, the current chapter, trivia and goofs, soundtrack, and "more like this". Clicking a cast member opens their filmography. It must be beautiful — this is the screenshot people will remember. (Face recognition arrives in Phase 22; until then it shows the full cast list, well-designed.) Next-episode handling with a countdown and a preview card. Episode navigation within the player. Chapter markers on the scrub bar. Thumbnail previews on scrub-bar hover, generated in the background via FFmpeg and cached. Picture-in-picture. Audio delay control. Video filter controls (brightness, contrast, saturation) for badly-encoded sources. Statistics-for-nerds overlay showing decode path, dropped frames, bitrate, and swarm state.

## Exit criteria

- [ ] **E1** The pause overlay is visually excellent and appears in < 200 ms.
- [ ] **E2** Scrub thumbnails generate without stuttering playback.
- [ ] **E3** Next-episode autoplay works and is disableable.
- [ ] **E4** All player features are keyboard-accessible and documented in a shortcuts overlay (`?`).
- [ ] **E5** Manual test plan for playback passes on all three tiers.

## Subtasks

- [ ] **11.1** Series and airing awareness (ADR-0030): migration 0019 airing_schedule filled from AniList nextAiringEpisode during anime ingest/refresh; EpisodeRepository lists a series' seasons and episodes and its next airing; the detail page of a series shows seasons, episodes (playable) and 'Episode 7 airs Thursday' even with no episode row (a pure, tested wording function).
- [ ] **11.2** The pause overlay: on pause, over the dimmed still, a panel with the cast (character names; headshots only with the user's TMDB key, else typographic), director and key crew, ratings that exist (IMDb from the catalogue; AniList for anime) and never Rotten Tomatoes, time remaining, the current chapter, 'more like this' from the vector index; a cast member opens their filmography. Data fetched when playback starts so the overlay paints in < 200 ms (measured in the real window). Trivia, goofs and soundtrack have no free legal source and are said to be absent, not scraped.
- [ ] **11.3** Episodes in the player: previous/next episode (N/P keys, pad), a next-episode card with a countdown in the last 30 s or at a credits chapter, autoplay on by default and switchable in Settings -> Playback; a pure, tested function decides the next episode across season boundaries.
- [ ] **11.4** The scrub bar: chapter marks from mpv's chapter list; hover thumbnails from FFmpeg sprites generated in a low-priority background process (one frame per 10 s, cached under data/cache/thumbs by file hash), proven not to stutter playback (mpv frame-drop-count unchanged while generating).
- [ ] **11.5** Player controls: picture-in-picture (small always-on-top window, Ctrl+P), audio delay (Ctrl+[ / Ctrl+]), brightness/contrast/saturation in a Picture menu, a stats-for-nerds overlay (I: decode path, dropped frames, bitrate, cache, swarm peers/speed), and a shortcuts overlay (?) generated from the same key map the player uses.
- [ ] **11.6** docs/MANUAL_TESTS.md playback plan run on all three tiers via the tier override (screenshots per tier), docs/specs/player-experience.md, learning note, evidence for E1-E5, regression check, close.

## Learning note

Why the overlay data is available but scene-level data isn't; background work without blocking playback; how thumbnail sprites work.

---

<!-- subtask log: appended during the phase -->

## Work log

