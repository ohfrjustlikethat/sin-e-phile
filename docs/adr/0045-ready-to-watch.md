# 0045 — Ready to Watch: a fix-first phase, inserted as Phase 16

- **Status:** Accepted 2026-11-08, decided by the author question by question after using the
  release build at the end of Phase 15: "glitches and weird ui/gui are still part of the app",
  "there's no way to download right now", "the local library doesn't sync well with entire tv
  show folders", "during playback, in tv mode the controller cannot change subtitles or volume",
  "the whole playback in general feels extremely choppy", "the problem of having latest tv shows
  and their latest episodes, as well as latest movies still persists", "now more than ever we
  need clarity".
- **Date:** 2026-11-08
- **Phase:** 16 (new); every phase from the old 16 onward moves up by one.

## Context

Phases 0–15 met their exit criteria with evidence, and the app was still not usable by its
author. The reasons were not in any harness:

- **No source, no explanation.** The library had no add-ons installed, so no film had anything to
  stream or download from, and onboarding never said so. Downloads could not be tested at all.
- **The player was shallow and felt choppy.** No subtitle style, download or sync; a controller
  could not change volume or subtitles; controls lagged, and opening and closing jumped.
- **The right stick did nothing.**
- **TV-show folders were misread.** A `Twin Peaks (1990)` folder with season subfolders was read
  file by file, and episodes were looked up as films.
- **The catalogue never refreshed itself.** ADR-0030's layers existed as tools, not as something
  the app does on its own, so new films and episodes never appeared.
- **Home did not show the local library straight after a scan**, and first run, Home and Settings
  had visible layout, focus and loading glitches.

## Decision

A new phase, **Phase 16 — Ready to Watch**, before the recommendation engine. Its choices, each
the author's:

| Question | Choice |
|---|---|
| Player direction | **Hybrid**: clean while watching; a full VLC-depth panel one button away |
| Subtitles in the player | Style editor, find & download, sync adjust, load a file: all four |
| Catalogue freshness | **Daily, in the background**; AniList airing every few hours; TMDB with a key |
| Home, top to bottom | Continue watching, New episodes of your shows, Your library, New releases, then taste |
| Controller in the player | **TV-standard**: A play/pause, left/right seek, up/down volume, LB/RB episode, Y subtitles, X audio, Menu panel, B back |
| Right stick | **Scrolls everything**: page, panel, rail, paused details |
| First run | Library folders, add-ons by address, keys, download backend: all four |
| TV layouts | Season folders, flat, anime absolute, split across drives: all four |
| What feels choppy | Opening/closing, jumpy motion, controls lag |
| Glitches | Cards and text, flicker/loading, focus jumps, layout/spacing |
| Scheduling | **Fix-first phase now**, rather than folding fixes into later phases |
| Taste screen | Films as cards, clearer hierarchy, live preview, fewer words |

**The author also asked for a list of "plugins … with links to download … literally everything
ever, work kind of exactly how it works on stremio".** That request is answered with `SETUP.md`:
every prerequisite (WebView2, FFmpeg, the model files, the datasets, the keys, an optional
qBittorrent, an optional VPN) with its official link and how to add it, and how to add **any**
add-on by its address. It **names no content source and links to none** (§2.1, rule 5); the app
suggests none, and the onboarding step checks whatever address the user brings. This is not
reopened by this ADR.

## Renumbering

Inserting a phase moves every later one up by one: the old 16–27 are now 17–28, and there are 29
phases, 0–28. The living text of `SPEC.md` (everything above `## Amendments`), `CLAUDE.md` and
`PROJECT_STATE.json` use the new numbers. **Documents written before 2026-11-08 keep the old
numbers**: the amendment entries before 4.0.0, ADRs 0001–0044, the session log, learning notes and
phase documents up to Phase 15. In those, read "Phase 18" as today's Phase 19, and so on for any
number from 16 up.

The tiers move with it: Tier B is now Phases 9–19, then 22, then 28; Tier C is 20, 21, 24; Tier D
is 23, 25, 26, 27. The understanding gates fire at the end of Phase 8 and the end of Phase 19.

## Consequences

- The recommendation engine waits for one phase; the carried author checks (Phase 7 E7, Phase 8
  E5, Phase 14 E1/E4/E6, Phase 15 E3) are received by Phase 16, which is where download and
  first-run work happens anyway.
- The author's answers: Phase 15 E3 met (the taste screen works) with polish asked for here;
  Phase 14 E1 met (first run under two minutes); Phase 8 E5 met (controller and TV mode good);
  Phase 14 E4/E6 not met (first run and Home need polish, here); Phase 7 E7 and the download
  checks could not be done without a source, and are carried here.
- **Testing changes**: each feature is also exercised in the real release build the way a person
  would use it (no add-ons installed, a real TV folder), not only in fixtures.
