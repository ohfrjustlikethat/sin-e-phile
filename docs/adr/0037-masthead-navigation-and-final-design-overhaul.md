# 0037 — Navigation moves to a top masthead, and Phase 18 ends with a design overhaul

- **Status:** Accepted
- **Date:** 2026-10-25
- **Phase:** 5 (decided), 18 (built)
- **Amends:** SPEC.md §3.1, §9.4, Phase 13, Phase 18 → spec_version 2.0.0
- **Supersedes:** the left navigation rail (SPEC.md Phase 1, §9.4) and Take B's sidebar
  (ADR-0024). ADR-0024 otherwise stands: its type, its spine numbers, its 96 px index
  column and its 74vh hero are all kept.

## Context

The author, 2026-10-25, after seeing the redesigned search screen: *"the design still
isn't amazing, i want the home, film, tv show etc tabs to be on top, i want a search
button too."*

They're right, and it's worth saying precisely why, because "make it look better" can't
be built or checked. Five things are wrong with the app as it stands, in order of how
much each one costs:

1. **The shell is a generic dashboard silhouette.** A 240 px left sidebar of icons and
   Inter labels, with a separate title strip above the content, is the layout of every
   admin panel. §9.0's test asks whether a screenshot would pass as MUBI or Criterion.
   Both of those put navigation in a single horizontal masthead and give the whole
   width to the work. The sidebar fails the test before any content has rendered.
2. **Search is invisible.** It's reached by `/`, `Ctrl+F` or the command palette, and
   none of these appear anywhere on screen. A search engine is this project's centre,
   and a first-time user can't find it.
3. **Most of the app is placeholder.** Home, Films, TV Shows, Watchlist and Live show a
   centred rounded pill ("PHASE 18"), a heading and a sentence. The pill alone breaks
   the two-radius rule, and the pages carry no design at all.
4. **There is no artwork, and the design assumed there would be.** ADR-0013 makes TMDB
   optional, so a default install has no posters or stills. Take B was chosen from
   mockups full of film stills. The state almost every user will actually see, with no
   artwork, has never been designed as the primary state, only as a fallback.
5. **Nothing enforces the banned list.** §9.0 lists shadows, uniform rounding,
   gradients, blur and Tailwind's default palette, and no tool checks for any of
   them. The search screen's result rows used colour classes that resolve to no
   token (`bg-surface-raised`, `ring-accent`), so keyboard focus was invisible, and
   nobody noticed until a screenshot.

## Decision

### The masthead

**One bar, at the top, holds the window's title bar and the app's navigation.** There is
no separate title strip and no sidebar.

- **Height 56 px**, `--void`, a 1 px `--line-subtle` rule beneath it. Solid at all times:
  no transparency over the hero and no blur (banned).
- **Left:** the wordmark *sin·e·phile* in Bricolage Grotesque, linking Home.
- **Then the five destinations as words, not icons:** Home, Films, TV Shows, Watchlist,
  Live. Inter, 13 px, `--ink-muted`. The active one is `--ink`, with a 2 px
  `--oxblood-bright` underline that sits on the bar's bottom rule. Oxblood marks intent
  and position, which is what §9.1 reserves it for.
- **Right:** a **Search button** labelled "Search", showing its `/` key hint. Then
  Downloads (a glyph that shows live progress while anything is downloading, and the
  count of active items), Settings, the profile avatar once Phase 14 exists, and the
  window controls.
- **Empty space in the bar is the drag region.** Double-click maximises, as a title bar
  should.
- **It fits at the 1,024 × 640 minimum window** with nothing collapsed. Five short words
  and three buttons need about 900 px. If a later destination breaks that, the fix is a
  design decision recorded here, not a silent overflow menu.

### What stays from Take B

- **The 96 px index column** on every content surface. With the sidebar gone it does more
  work: it's the page's left edge, its spine numbers and its hairline rule, and it's
  the thing that makes the app read as a kept catalogue.
- Bricolage for display, Instrument Serif for film titles only, Inter for metadata, mono
  for numbers. Radius 0 and 2 px. Depth from value and lines. The 74vh Home hero.

### Phase 18 ends with a final design overhaul

Phase 18 builds the browsing surfaces. It now also redesigns **every** surface to one
standard before it can close. The overhaul has a process, not just a goal:

1. **Mockups before code, again** (ADR-0023's rule). Static HTML, real catalogue data,
   two interpretations of the masthead and of Home, Films and a detail page. Each is shown
   **with no artwork first** and with artwork second. The author chooses.
2. **The artwork-free state is designed as the primary state.** The typographic card
   (§9.4) has to hold a whole page. Instrument Serif title, director, year and spine
   number on `--surface` with a 1 px line, with card sizes varied by editorial weight.
   Pages are judged in this state first.
3. **No placeholder survives.** Every destination, Search, Settings, the detail page and
   every empty and error state is designed. An honest empty state is still a designed
   one.
4. **A design lint.** It runs in CI and fails on what §9.0 bans and code can detect:
   shadow utilities, radii other than 0 and 2 px, `backdrop-blur`, decorative gradient
   utilities, Tailwind's default palette names, and Tailwind colour classes that resolve
   to no token (the search rows' bug). Each rule is seen to fail before it counts.
5. **Motion as a finish.** Page changes at 180 ms, the active-tab underline moves
   rather than jumps, and `prefers-reduced-motion` reduces both to an opacity fade.
6. **A screenshot review set.** `tools/shots` captures every surface, at 1,024 × 640
   and 1,920 × 1,080, with and without artwork. The author reviews the set against
   §9.0's test, and Phase 18 closes only on their sign-off.

### When the masthead is built

The masthead changes only the shell, not any surface beneath it, so it **may be built
ahead of Phase 18** as a standalone task whenever the author asks. Phase 18 then finishes
the overhaul under it. Either way, Phase 18's exit criteria are where it's checked.

## Consequences

- Phase 18 grows from 2–3 sessions to 4–5. That's the honest price: half of the new
  work is the mockup round and the review set, and those are what make "amazing"
  something the author decides rather than something asserted.
- The left rail's collapse/expand state, persisted in the UI store since Phase 1, has no
  home and is removed with the rail.
- Downloads loses the full-height surface Phase 13 gave it "in the sidebar". It becomes a
  page reached from the masthead button, which is where a browser puts it too.
- `tools/uiaudit`'s keyboard-reach check must cover the masthead's tab order: wordmark,
  destinations, Search, Downloads, Settings, then content.
- Tab labels are words, not icons. That costs horizontal room, and the 1,024 px fit
  check is what keeps it honest.

## Alternatives considered

- **Keep the sidebar and restyle it.** Rejected. The silhouette is the problem, not the
  styling, and the author asked for tabs on top.
- **A transparent masthead over the Home hero, solidifying on scroll** (MUBI's own
  behaviour). Tempting and more cinematic. Rejected for now: without blur or a
  decorative gradient, ink on a bright still is unreadable on some frames, and a
  legibility scrim would be the first gradient in the app. The mockup round may
  reopen this with evidence.
- **Icons on the tabs.** Rejected. Words are the MUBI and Criterion convention, the
  current glyphs are placeholders, and an icon row is the dashboard tell this ADR
  removes.
- **Hide search behind an icon only.** Rejected. The author asked for a search button,
  and a magnifier with no label is exactly how search became invisible in the first
  place.
