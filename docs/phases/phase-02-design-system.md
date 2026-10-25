# Phase 2 — Design System and Visual Language

**Status:** complete · **Depends on:** 1 · **Sessions:** 1–2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

The complete visual language, built and documented, before any product UI exists.

## Deliverables

All Section 9 tokens as CSS custom properties, consumed by a Tailwind theme extension. Fonts bundled locally. A component gallery route (`/design`, dev-only) rendering every primitive in every state. Primitives: `Button` (primary/secondary/ghost/danger × sizes × loading/disabled), `IconButton`, `Input`, `Select`, `Toggle`, `Slider`, `Tabs`, `Tooltip`, `Popover`, `Dialog`, `Toast`, `Skeleton`, `Spinner`, `Badge`, `ProgressBar`, `Rating`. Media primitives: `PosterCard`, `EpisodeCard`, `ChannelCard`, `Rail` (virtualised, momentum scroll, edge-bleed, keyboard-navigable), `HeroBanner`, `EmptyState`. A focus-management system with visible rings and correct tab order. Full keyboard navigation infrastructure including a `Ctrl+K` command palette shell. `prefers-reduced-motion` support throughout. A contrast audit script that fails CI if any token pair used for text drops below AA.

## Exit criteria

- [x] **E1** Every primitive renders correctly in the gallery, in all states.
- [x] **E2** The entire gallery is navigable by keyboard alone with visible focus at every step.
- [x] **E3** Contrast audit passes.
- [x] **E4** A rail of 500 poster cards scrolls at 60 fps with no dropped frames.
- [x] **E5** `docs/specs/design-system.md` documents every token and component with usage rules.

## Subtasks

- [x] **2.0** THREE static HTML mockups of Home, film detail, and player-with-chrome, as three interpretations within the SPEC.md 9.0 brief, with REAL artwork and metadata. Author chooses before any token or component code.
- [x] **2.1** All SPEC.md 9 tokens as CSS custom properties consumed by the Tailwind theme; fonts bundled locally, no network font requests
- [x] **2.2** Component gallery route /design, dev-only, rendering every primitive in every state
- [x] **2.3** Primitives: Button, IconButton, Input, Select, Toggle, Slider, Tabs, Tooltip, Popover, Dialog, Toast, Skeleton, Spinner, Badge, ProgressBar, Rating
- [x] **2.4** Media primitives: PosterCard (with the ADR-0013 typographic artwork-free state), EpisodeCard, ChannelCard, Rail, HeroBanner, EmptyState
- [x] **2.5** Rail: virtualised, momentum scroll, edge-bleed, keyboard-navigable, 60fps with 500 cards
- [x] **2.6** Focus management with visible rings and correct tab order; full keyboard navigation; Ctrl+K command palette shell
- [x] **2.7** prefers-reduced-motion support throughout (the global rule exists from Phase 1; verify per component)
- [x] **2.8** Contrast audit script failing CI if any text token pair drops below WCAG AA; --ink-faint on --surface is the one to check
- [x] **2.9** docs/specs/design-system.md documenting every token and component with usage rules

## Learning note

Why a design system before features; what design tokens are; how CSS custom properties enable theming; virtualisation and why rendering 500 DOM nodes is a mistake.

---

<!-- subtask log: appended during the phase -->

## Work log
- **2.0** THREE static HTML mockups of Home, film detail, and player-with-chrome, as three interpretations · `aebbd7f`
- **2.1** All SPEC.md 9 tokens as CSS custom properties consumed by the Tailwind theme; fonts bundled loca · `aebbd7f`
- **2.2** Component gallery route /design, dev-only, rendering every primitive in every state · `aebbd7f`
- **2.3** Primitives: Button, IconButton, Input, Select, Toggle, Slider, Tabs, Tooltip, Popover, Dialog, T · `aebbd7f`
- **2.4** Media primitives: PosterCard (with the ADR-0013 typographic artwork-free state), EpisodeCard, Ch · `aebbd7f`
- **2.5** Rail: virtualised, momentum scroll, edge-bleed, keyboard-navigable, 60fps with 500 cards · `aebbd7f`
- **2.6** Focus management with visible rings and correct tab order; full keyboard navigation; Ctrl+K comm · `aebbd7f`
- **2.7** prefers-reduced-motion support throughout (the global rule exists from Phase 1; verify per compo · `aebbd7f`
- **2.8** Contrast audit script failing CI if any text token pair drops below WCAG AA; --ink-faint on --su · `aebbd7f`
- **2.9** docs/specs/design-system.md documenting every token and component with usage rules · `aebbd7f`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `3c2077f`.

### Evidence per criterion

**E1** — Every primitive renders correctly in the gallery, in all states.

> Rendered headless at 1500px across all four gallery tabs with 0 page errors; screenshots in tools/uiaudit/out/. Every primitive shown in loading, disabled, error and empty states; PosterCard shown in both ADR-0013 states side by side.

**E2** — The entire gallery is navigable by keyboard alone with visible focus at every step.

> node tools/uiaudit/run.mjs: 45 Tab stops walked, 0 without a visible focus ring, 0 off-screen. Rail exposes exactly 1 tab stop; ArrowRight/End reach card 499 of 500 (roving tabindex).

**E3** — Contrast audit passes.

> python tools/contrast/audit.py -> "29 enforced pairs pass WCAG AA (3 decorative recorded, not enforced)"; runs in CI.

**E4** — A rail of 500 poster cards scrolls at 60 fps with no dropped frames.

> node tools/uiaudit/run.mjs -> 200 sampled frames flicking 500 cards: median 16.7ms, p95 16.7ms, worst 16.8ms, 0 frames over 33.3ms, 14/500 cards mounted. Harness verified to fail on a reintroduced regression.

**E5** — `docs/specs/design-system.md` documents every token and component with usage rules.

> docs/specs/design-system.md - tokens, layout invariants, all 21 components, Rail virtualisation + keyboard contract, motion, measured numbers, and rules for extending it.

### Debt incurred

- **D9** Rail's roving tabindex sets tabIndex imperatively on the first focusable descendant of each mounted item rather than threading it through the render prop. Correct today because `render` returns caller-owned markup, but it silently does nothing if a card's first focusable element is not its main control. Revisit in Phase 9, when real screens use Rail with more complex cards.
- **D10** The UI audit (tools/uiaudit) drives the design gallery, not real product screens - those do not exist until Phase 9. Its budgets prove the Rail component holds 60fps, not that any real screen does. Revisit in Phase 9: point the audit at the Home screen too.

### Next phase starts by

Phase 4 subtask 4.1: build the resumable job runner in tools/ingest/ with checkpointing, so a killed ingestion resumes rather than restarts (exit criterion E2). Read docs/phases/phase-04-metadata-backbone.md first; R4 in docs/RISKS.md is this phase's named risk.
