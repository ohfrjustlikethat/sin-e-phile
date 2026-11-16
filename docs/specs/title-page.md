# The title page: Play, the round buttons, and every stream

*Phase 16, the author's brief item 7 (`docs/design/author-brief-2026-11-17.md`, rounds 1, 2 and 5).*

- **Play** is one wide button (the oxblood primary), "Resume from 20:00" when there is a place to resume,
  with "Start over" beside the line under it. Down from Play goes to the first round button
  (`data-pad-down`, honoured by `moveFocus` when the target is on screen), not the one under Play's middle.
- **The round buttons** (`RoundAction`): 64 px circles with a word under each. Download takes the best stream
  (`download_title` with no candidate) and then says how far it has got ("Downloading 42%"), and goes to
  Downloads when pressed again; Watchlist and Watched fill when on (and are `aria-pressed`); Streams opens the
  panel. The "stream or download" advice is gone (`DownloadAdvice` deleted).
- **Best** is the scoring spec's order (`docs/specs/source-scoring.md` §1): smooth to start, nearest the
  preferred quality from Settings → Playback ("Best for this PC", 4K, 1080p, 720p, SD), then points.
- **The streams panel** (`StreamsPanel`) slides in from the right, inside the app frame under the masthead
  (`#panel-slot` in App, so TV mode's scale and safe margin apply). Audio and Quality choices at its top; then
  one section per add-on in Settings' order; best first within each. A row: resolution, HDR and codec badges;
  size and seeders, or "direct"; "Play picks this" on the chosen one; the full release name in small grey type;
  audio and subtitle language badges; a warning line when it is weak or probably fake. Each row has Play
  (`play_candidate`, which resolves it and makes it the plan's choice, so the player's fallbacks are the honest
  copies below it) and, unless it is a file already here, Download (`download_title` with its index).
- **Leaving the panel**: its own "← Sunrise" button, or Escape, Backspace or B, which take the first step of
  the app's one Back (`panelBack` in the store). It keeps its list while a new audio or quality choice re-plans.

Evidence: `src/features/detail/DetailScreen.test.tsx` "the title page's round buttons and streams";
`crates/selection/tests/fixtures.rs`; the pad audit's title page steps (round Download, Watchlist, Mark
watched, Streams, the Audio row inside the panel, B closing it with the film's page kept).
