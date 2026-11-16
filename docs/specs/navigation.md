# Navigation: trails, the way back, scroll memory

*Phase 16, the author's brief item 6 (`docs/design/author-brief-2026-11-17.md`, round 2). The author: "going back to
an earlier page takes me back to the default of that page", "add clearer navigation, add arrows to any screen can be
exited".*

## The model (`src/lib/store.ts`)

- **Tabs.** The five destinations (§3.1, unchanged) and search. Search stays a mode reached from the masthead's
  labelled button (ADR-0037), but keeps a trail and its words like a tab.
- **Trails.** Each tab has its own list of title pages opened from it, oldest first. `detail` is the end of the
  current tab's trail, so every reader of `detail` is unchanged.
- **Going to a tab** (`goTab`, the masthead, LB/RB) shows the page it was left on. Pressing the tab already showing
  goes to its start: its trail is emptied and its scroll positions forgotten.
- **Search shortcuts** (`searchNow`: "/", Ctrl+F, the pad's Y) mean "I want to type": the search box, with its words,
  at the start of search's trail. The masthead's Search button is a tab.
- **Back** (`back`) takes one step: the review queue or taste profile to Settings; Settings or Downloads to the page
  under them; a title page to the page before it; search to the destination. It returns `false` when there is
  nowhere to go.

## The way back on screen (`src/app/BackBar.tsx`, `src/app/backKey.ts`)

- A large arrow top-left, just under the masthead, saying where it goes: "← Search results", "← Home", "← Heat"
  (the previous title, from the query cache), "← Settings". Shown wherever `back` would do something.
- Escape, Backspace outside a field, and the pad's B (sent as Escape) take the same step, once, in App: only when
  nothing else used the key (menus, dialogs, the full player and the review queue mark Escape handled; Popover now
  does too), and not under the first run, the profile picker or the palette. Escape in a field leaves the field
  first, except the search box, whose Escape leaves search.
- The screens' own "← Back" buttons and Escape handlers (title page, Downloads, taste profile) are gone: one way back.
  Settings keeps its "Done" until item 8 redesigns it.

## Scroll memory (`src/lib/scrollMemory.ts`)

- One scroll area (`<main>`) holds every page. Positions are recorded per page key (`tab|trail` or the overlay's
  name) as the person scrolls, and put back when that page returns, retried for up to 1.5 s while a page fills in
  (Home's rails), and abandoned the moment the person scrolls.
- The hook takes the scroll area through a callback ref: `<main>` is a new element after the full-screen player,
  and a listener bound once stayed on the old one, so nothing was recorded after a film had played. The pad audit
  found this in real layout ("Settings opens again where it was left (1126 -> 0)"); it now reads 2366 -> 2366.

## Evidence

- `src/lib/store.test.ts` (trails, overlays, search), `src/app/backKey.test.ts`, `src/app/BackBar.test.tsx` (labels,
  scroll, a replaced scroll area); each seen to fail.
- `tools/uiaudit/controller.mjs`, by pad: "its page has a back arrow that says Watchlist", "RB then LB comes back to
  the film's page: the Watchlist tab kept it" (seen to fail with tabs that forget), "B goes back from the film's page
  to the Watchlist", "Settings opens again where it was left".
