# Progress

> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by
> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two
> can never disagree). Edit the state file, then regenerate.

**Spec version 1.4.0** · 5 session(s) completed · last updated 2026-08-31

---

## Where we are right now

**Phase 2 — Design System and Visual Language** (`complete`, branch `phase/02-design-system`)

5 of 5 exit criteria met with evidence.

> Phase 2 builds the visual language BEFORE any product UI. Two constraints carry over from Phase 1: ADR-0020 removed the gradient scrim under player chrome, so the design system must produce a solid opaque panel; and ADR-0013 requires every artwork-bearing surface to have a designed artwork-free state, so PosterCard needs a typographic fallback rather than a grey rectangle.

### Subtasks — 10/10 complete

- [x] **2.0** THREE static HTML mockups of Home, film detail, and player-with-chrome, as three interpretations within the SPEC.md 9.0 brief, with REAL artwork and metadata. Author chooses before any token or component code. · `aebbd7f`
- [x] **2.1** All SPEC.md 9 tokens as CSS custom properties consumed by the Tailwind theme; fonts bundled locally, no network font requests · `aebbd7f`
- [x] **2.2** Component gallery route /design, dev-only, rendering every primitive in every state · `aebbd7f`
- [x] **2.3** Primitives: Button, IconButton, Input, Select, Toggle, Slider, Tabs, Tooltip, Popover, Dialog, Toast, Skeleton, Spinner, Badge, ProgressBar, Rating · `aebbd7f`
- [x] **2.4** Media primitives: PosterCard (with the ADR-0013 typographic artwork-free state), EpisodeCard, ChannelCard, Rail, HeroBanner, EmptyState · `aebbd7f`
- [x] **2.5** Rail: virtualised, momentum scroll, edge-bleed, keyboard-navigable, 60fps with 500 cards · `aebbd7f`
- [x] **2.6** Focus management with visible rings and correct tab order; full keyboard navigation; Ctrl+K command palette shell · `aebbd7f`
- [x] **2.7** prefers-reduced-motion support throughout (the global rule exists from Phase 1; verify per component) · `aebbd7f`
- [x] **2.8** Contrast audit script failing CI if any text token pair drops below WCAG AA; --ink-faint on --surface is the one to check · `aebbd7f`
- [x] **2.9** docs/specs/design-system.md documenting every token and component with usage rules · `aebbd7f`

### Exit criteria

- [x] **E1** Every primitive renders correctly in the gallery, in all states.
      - *Evidence:* Rendered headless at 1500px across all four gallery tabs with 0 page errors; screenshots in tools/uiaudit/out/. Every primitive shown in loading, disabled, error and empty states; PosterCard shown in both ADR-0013 states side by side.
- [x] **E2** The entire gallery is navigable by keyboard alone with visible focus at every step.
      - *Evidence:* node tools/uiaudit/run.mjs: 45 Tab stops walked, 0 without a visible focus ring, 0 off-screen. Rail exposes exactly 1 tab stop; ArrowRight/End reach card 499 of 500 (roving tabindex).
- [x] **E3** Contrast audit passes.
      - *Evidence:* python tools/contrast/audit.py -> "29 enforced pairs pass WCAG AA (3 decorative recorded, not enforced)"; runs in CI.
- [x] **E4** A rail of 500 poster cards scrolls at 60 fps with no dropped frames.
      - *Evidence:* node tools/uiaudit/run.mjs -> 200 sampled frames flicking 500 cards: median 16.7ms, p95 16.7ms, worst 16.8ms, 0 frames over 33.3ms, 14/500 cards mounted. Harness verified to fail on a reintroduced regression.
- [x] **E5** `docs/specs/design-system.md` documents every token and component with usage rules.
      - *Evidence:* docs/specs/design-system.md - tokens, layout invariants, all 21 components, Rail virtualisation + keyboard contract, motion, measured numbers, and rules for extending it.

---

## What's next

Phase 2 is DONE: merged to main, tagged phase-02, CI green on main (run 33397473458 on the branch, and the main run after the merge - all 5 jobs pass, including the two new audit steps). Start Phase 3: read docs/phases/phase-03-*.md, then SPEC.md 15's Phase 3 entry, and plan before implementing. The understanding gate does NOT fire here - ADR-0016 moved it to tier boundaries, so the five questions in docs/learning/phase-02-notes.md accumulate and are asked together with Phases 1-8 at the end of Phase 8.

---

## Blockers

None.

---

## All 28 phases

Tiers are the legitimate stopping points from `SPEC.md` Appendix E. **Tier B is the definition of done** — complete it and the project has succeeded.

| | # | Phase | Tier | Depends on | Sessions | Criteria met |
|---|---|---|---|---|---|---|
| [x] | 0 | Bootstrap and Project Infrastructure | A | nothing | 1 | 8/8 |
| [x] | 1 | Application Shell and Capability Tiers | A | 0 | 1–2 | 7/7 |
| [x] | 2 | Design System and Visual Language | A | 1 | 1–2 | 5/5 |
| [ ] | 3 | Data Layer and Portable Storage | A | 1 | 1–2 | 0/5 |
| [ ] | 4 | Metadata Backbone | A | 3 | 2–3 | 0/7 |
| [ ] | 5 | Semantic Search Engine | A | 4 | 2 | 0/5 |
| [ ] | 6 | Source Resolver and Addon Protocol | A | 3 | 1–2 | 0/6 |
| [ ] | 7 | Torrent Engine and Streaming Server | A | 6 | 2–3 | 0/6 |
| [ ] | 8 | Player Core — MILESTONE: FIRST DEMOABLE BUILD 🏁 | A | 5, 7 | 2–3 | 0/6 |
| [ ] | 9 | Intelligent Source Selection | B | 8 | 1–2 | 0/6 |
| [ ] | 10 | Subtitle Pipeline | B | 8 | 2 | 0/6 |
| [ ] | 11 | Player Experience Layer | B | 8, 9, 10 | 2 | 0/5 |
| [ ] | 12 | Local Library Engine | B | 4 | 2–3 | 0/6 |
| [ ] | 13 | Download Manager and the Stream-vs-Download Advisor | B | 7, 9, 12 | 1–2 | 0/5 |
| [ ] | 14 | Profiles and First-Run Onboarding | B | 3, 5 | 2 | 0/6 |
| [ ] | 15 | Taste Model | B | 5, 14 | 2 | 0/5 |
| [ ] | 16 | Recommendation Engine | B | 15 | 2–3 | 0/6 |
| [ ] | 17 | Discovery Engine | B | 16 | 2 | 0/6 |
| [ ] | 18 | Browsing Surfaces 🏁 | B | 17 | 2–3 | 0/5 |
| [ ] | 19 | Watchlist and External Sync | C | 18 | 1–2 | 0/5 |
| [ ] | 20 | Windows Platform Integration | C | 12, 18 | 2–3 | 0/5 |
| [ ] | 21 | Performance Engineering and the Low-End Path | B | 18 (20 optional) | 2 | 0/4 |
| [ ] | 22 | Vision Layer (Tier 2) | D | 11 | 1–2 | 0/5 |
| [ ] | 23 | Binge Intelligence | C | 11, 12 | 1–2 | 0/4 |
| [ ] | 24 | Live Channels | D | 18 | 2 | 0/5 |
| [ ] | 25 | Manga and Comics | D | 3, 5, 16 | 2–3 | 0/5 |
| [ ] | 26 | Connected Playback | D | 11 | 2 | 0/5 |
| [ ] | 27 | Hardening, Packaging, and Portfolio Finalisation | B | everything | 2–3 | 0/5 |

Legend: `[x]` complete · `[~]` in progress · `[!]` blocked · `[?]` awaiting review · `[ ]` not started. 🏁 marks Phase 8 (first demoable build) and Phase 18 (complete product).

---

## Known debt

- **D1** (raised in Phase 0) The hashed denylist matches exact tokens only — no substring, fuzzy, or homoglyph matching. Accepted in ADR-0009; the structural matcher covers the shapes that carry real risk. Revisit only if a near-miss is ever observed.
- **D2** (raised in Phase 0) A fresh clone is unprotected by the git hooks until tools/doctor runs once, because core.hooksPath is per-clone config. CI is the backstop. Accepted in ADR-0012.
- **D3** (raised in Phase 0) Bare-domain detection is disabled inside source files (attribute access is shaped identically). URLs are still checked everywhere, as is the denylist. Documented in tools/guard/README.md.
- **D4** (raised in Phase 0) tools/state/validate_state.py implements a subset of JSON Schema draft 2020-12 by hand, because ADR-0012 fixed these tools as stdlib-only. It rejects any schema construct it does not implement rather than passing silently, but it is not a conformant validator. Revisit only if the schema needs constructs it lacks.
- **D5** (raised in Phase 1) Player has TWO compositing paths - still-frame when paused, region cutouts when playing - and the chrome silhouette must stay in sync with the region or the uncovered part of the hole shows the desktop. Phase 11 owns this invariant. Retire if wry PR #1762 merges and DirectComposition replaces both.
- **D6** (raised in Phase 1) librqbit has no webseed (BEP-19) support, so Internet Archive torrents will not work through the torrent path. Phase 6's InternetArchiveBackend must resolve to direct HTTP instead. Not a defect, but it constrains how that backend is built.
- **D7** (raised in Phase 1) Integration tests that need a running Tauri app cannot run under cargo test on Windows (ADR-0022): a test binary linking Tauri fails to launch with STATUS_ENTRYPOINT_NOT_FOUND, and the targeted fix is nightly-only. Unit tests are unaffected because logic lives in crates/. Such tests belong in the SPEC.md 12.3 manual plan, or a WebDriver harness later.
- **D8** (raised in Phase 1) tiers.rs treats any non-software DXGI adapter as having hardware decode, and uses >= 2 GB dedicated VRAM as a proxy for 'discrete GPU or strong iGPU'. Both are coarse. Whether a SPECIFIC codec decodes in hardware is only knowable at play time from mpv's hwdec-current, so Phase 8 should feed that back and Phase 21 should revisit the VRAM threshold against real Tier 0/1 hardware.
- **D9** (raised in Phase 2) Rail's roving tabindex sets tabIndex imperatively on the first focusable descendant of each mounted item rather than threading it through the render prop. Correct today because `render` returns caller-owned markup, but it silently does nothing if a card's first focusable element is not its main control. Revisit in Phase 9, when real screens use Rail with more complex cards.
- **D10** (raised in Phase 2) The UI audit (tools/uiaudit) drives the design gallery, not real product screens - those do not exist until Phase 9. Its budgets prove the Rail component holds 60fps, not that any real screen does. Revisit in Phase 9: point the audit at the Home screen too.

---

## Decisions pending

- **P1** — decide by Phase 27 Source-only distribution versus Phase 27 packaging and the 2.3 installed-size budget. See docs/DECISIONS_PENDING.md.
- **P5** — decide by Phase 12 Where the Phase 12 review-queue confidence threshold sits, given >95% top-1 and <1% false-confident pull against each other. Measure, do not guess. See docs/DECISIONS_PENDING.md.
- **P6** — decide by Phase 27 Windows Sandbox pass on a genuinely bare machine. The E1 CI job proves a clean checkout builds, but windows-latest ships Rust, Node and MSVC preinstalled, so it does not prove SETUP.md is complete from nothing.
- **P8** — decide by Phase 21 Spike C measured query-embedding latency on the Tier 2 dev machine only. ADR-0015 also asked for a constrained VM approximating Tier 0. Unpadded headroom is ~18x so this does not block Phase 5, but the padded worst case with a 3-4x Tier 0 penalty lands at 24-33 ms, close to the 30 ms trigger. Measure before Phase 21 signs off the 80 ms search budget.
