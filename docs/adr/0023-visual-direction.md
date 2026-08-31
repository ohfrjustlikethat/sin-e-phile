# 0023 — Visual direction: editorial, near-black, and not AI-generated

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 2
- **Amends:** `SPEC.md` §9 → spec_version 1.4.0 · **Resolves:** P7

## Context

- `SPEC.md` §9 specified tokens and a name ("Charcoal & Oxblood") but **no
  direction** — nothing that says what the app should feel like, and nothing a
  design could be judged against.
- The author supplied one: MUBI's editorial restraint, Spotify's typographic
  hierarchy, Criterion's sense of curation, Netflix's browsing mechanics only.
- They also named a failure mode explicitly: **looking AI-generated**. That is not
  vagueness — it is a specific and recognisable set of choices (purple, glass,
  gradients, uniform rounding, uniform sizing, a neutral sans used as display).
- The original tokens were judged **flat**. They are: the value steps are nearly
  linear, and every grey is blue-shifted while the ink is warm cream.

## Decision

**§9.0 is added** carrying the reference hierarchy, an explicit banned list, an
explicit required list, and a single test the design is audited against:

> Would this screenshot pass as a MUBI or Criterion screen? If it would pass as a
> generic dark SaaS dashboard, it has failed.

**The palette is revised**, not merely re-toned. Two structural changes:

- **Warm greys.** R ≥ G ≥ B throughout, so the ground agrees with the warm cream ink
  instead of fighting it. Cool grey plus warm ink is a large part of why the original
  read as generic.
- **Non-linear value steps.** A large jump from `--base` to `--surface`, then small
  ones. Elevation becomes rare and meaningful rather than a ramp everything sits on.

**Oxblood stays.** It is not a technology colour; it reads as cinema — velvet,
Criterion's spine red — and it is the one thing stopping a near-black neutral palette
from being anonymous. Its chroma is raised slightly so it separates from the warmer
ground. It remains reserved for *intent* and is never a large fill.

**Process: mockups before code.** Three static HTML mockups of three screens (Home,
film detail, player with chrome), as three interpretations *within* this brief, with
real artwork and real metadata. The author chooses; tokens and components are built
from the chosen one. **No token or component code before that answer.**

## Consequences

- The design system gains a **falsifiable** acceptance test. "Looks clean" stops being
  a defence.
- Real artwork and real metadata in the mockups is a hard requirement: grey
  placeholder boxes hide exactly the problems that need seeing — how the chrome
  behaves against a bright still, whether the type holds at real lengths.
- **Constrains later phases too.** Phase 11's pause overlay, Phase 18's browsing
  surfaces and Phase 14's onboarding are all audited against §9.0.
- The banned list rules out some genuinely easy wins. Uniform rounding and uniform
  card sizes are faster to build and easier to keep consistent; deliberate
  irregularity costs real effort and needs judgement per surface.
- **No shadows as depth** means every elevation must be earned with surface value and
  1px lines, which is harder to get right and much better when it is.
- Fonts must be free and self-hostable, so the MUBI and Spotify faces (Söhne-like,
  Circular) are unavailable and equivalents must be found and argued for.

## Alternatives Considered

- **Keep §9 as it was and let taste do the rest.** Rejected: the original tokens have
  no direction behind them, and "taste" is not reviewable. A written brief with a
  test is.
- **Drop oxblood for a neutral or monochrome scheme.** Genuinely tempting — MUBI is
  nearly accentless. Rejected because a monochrome dark app with no accent is exactly
  the generic-SaaS silhouette §9.0 bans, and the accent is what marks intent.
- **Pick a direction and build straight into components.** Rejected by the author,
  correctly: three interpretations rendered with real content cost far less than
  discovering in Phase 18 that the direction was wrong.
- **Adopt Tailwind's dark palette and restyle later.** Rejected outright — §9.0 bans
  it by name, and "restyle later" is how the tell survives to Phase 27.
