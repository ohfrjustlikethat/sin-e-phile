# 0024 — The design system is Take B, with Take A's hero

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 2
- **Implements:** `SPEC.md` §9 (ADR-0023)

## Context

- Three mockups were built against the §9.0 brief with real artwork and metadata:
  **A Editorial** (Instrument Serif, bets on emptiness), **B Catalogue**
  (Bricolage Grotesque, spine numbers and an index column), **C Signal**
  (Archivo 800, heavy display against quiet metadata).
- The author chose **B, with A's hero size** — 74vh rather than 56vh.

## Decision

**Take B is the design system.** Bricolage Grotesque 800 for display, Instrument
Serif reserved for film titles, Inter for UI and metadata only, JetBrains Mono for
technical panels. Radius 0 and 2px. Spine numbers, hairline rules, and a persistent
index column as the idiosyncratic details §9.0 requires.

**The hero is 74vh**, taken from Take A.

## Consequences

- The hero change is not cosmetic. At 74vh the first rail sits below the fold, so
  Home leads with a single curated statement rather than a browsing grid — B's
  catalogue ordering, opening with A's restraint.
- The index column is now a **layout invariant**, not decoration: content sits in a
  `96px | 1fr` grid on every surface, and Phase 18's browsing screens must honour it
  or the vertical rule breaks.
- Two display faces (grotesque + serif) means a discipline to hold: the serif is for
  **film titles only**. Using it for UI headings would collapse the distinction that
  makes it read as editorial.
- Bricolage Grotesque is a variable font with real quirks at weight 800; it must be
  subset and self-hosted, and it is the identity of the app.

## Alternatives Considered

- **Take A unchanged.** The most distinctive and the most restrained, but at 200+
  titles the emptiness stops reading as curation and starts costing navigation.
- **Take C unchanged.** Best for browsing density; weakest on the §9.0 test, since
  heavy uppercase grotesque on near-black is the closest of the three to a generic
  dark dashboard.
- **B at its original 56vh hero.** Rejected by the author. The taller hero is what
  keeps B from reading as a dense catalogue *only*.
