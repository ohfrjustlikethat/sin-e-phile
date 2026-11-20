# 0056 — Ratings for any title, and the author's interface answers of 2026-11-20

- **Status:** Accepted 2026-11-20. Decided by the author, question by question (the answers are theirs; this records
  them). The author: "the rating system should feed into the taste profile, i dont care if you have to rebuild the
  system, it needs to work well, because on that lies the recommendation algorithm."
- **Date:** 2026-11-20
- **Phase:** 18 (built before it closes, at the author's request)
- **Amends:** ADR-0043 (Surprise me's place; the end card's stars), Appendix F I42 (removed), Phase 15's signals (ratings
  in half stars, seasons rated). `SPEC.md` §15 Phase 18, Appendix F; spec_version 4.9.0.

## Context

Asked to rate ten films for Phase 18's acceptance test, the author found there was no way to: a title could be rated
only on the end card after it played, or in onboarding. They asked for a Letterboxd-style rating for any title, rated
season by season for a show, feeding the taste model; a way to remove anything from Continue watching; Surprise me in
the top bar; and the "the episode each show is on" line gone.

## Decision (the author's answers)

**Ratings**

- **Half stars**, 0.5 to 5 in half steps, like Letterboxd. No reviews.
- **A show is rated season by season.** The show itself shows the **average of the seasons rated**, marked "your
  average"; there is no separate show-level rating.
- **Stars and watching both count** in the taste model: a rating adds to what finishing or abandoning already says
  about a title (a finished film rated two stars is a mixed signal, not a clear "not this").
- **Rated from**: the title page, any card's menu (the X / right-click menu), the end card after watching, and a
  **rate-a-batch screen** of well-known films to rate one after another.
- **On a film's page**: a row of five stars on its own line under the title and year, before the buttons, with "Rate"
  beside it until rated. **On a show's page**: stars beside each season's header in the episode list, and "your
  average" under the title.
- **Filled stars are oxblood**, the app's accent.
- **The rate-a-batch screen** opens from the taste page ("Rate more films") and from a card on Home while a profile has
  fewer than ten ratings.

**Continue watching**

- **Remove from Continue watching** hides the title from the row and keeps its place; it comes back only if it is
  played again. Reached from the card's menu, an × on the card on hover, and the title's page.
- The small line under Continue watching ("the episode each show is on", "where you left each film") is removed on
  Films and TV Shows, and on Home.

**Surprise me**

- A text button in the top bar, just left of Search. The length choice ("I have 90 minutes", I42) is **removed**
  everywhere at the author's word, and the row it sat in on Home with it.

## Consequences

- A migration moves ratings to half stars (existing whole stars doubled) and adds season ratings; every reader of
  ratings (the taste model's signals, liked titles, the bandit's outcomes, imports, onboarding, the end card) reads half
  stars.
- The card menu becomes every title card's, not only recommended ones'.
- I42 leaves Appendix F as removed by the author, not as done or carried.
