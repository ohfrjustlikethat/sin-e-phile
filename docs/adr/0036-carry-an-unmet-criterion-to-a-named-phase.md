# 0036 — A built phase may carry an unmet criterion to a named later phase

- **Status:** Proposed. **Not in force until the author approves** (SPEC.md §2.8).
- **Date:** 2026-10-25
- **Phase:** 5
- **Resolves:** P14 (`docs/DECISIONS_PENDING.md`)
- **Amends:** SPEC.md Appendix B, "What 'done' means"

## Context

Appendix B says a phase is complete only when every exit criterion is met with evidence.
The state schema makes that literal: a `complete` phase must have `met: true` on every
criterion, and `tools/state/validate_state.py` rejects any unmet criterion behind
`current_phase`.

Phase 5 has three criteria that rule keeps open, for different reasons:

| criterion | measured | why it is unmet |
|---|---|---|
| E1 p95 < 80 ms | 15.2 ms | §2.3 enforces against Tier 0 hardware; this is a Tier 2 machine. P8 already schedules Tier 0 measurement for Phase 21. |
| E3 nDCG@10 > 0.75 | meaning 0.0472, filter 1.0000 | Five attempts. The last, a 3 h re-embed, lowered it (`docs/eval-results.md`, Session 19). No cheap lever is left. |
| E4 two named queries defensible | screenshots in `docs/case-study/search.md` | Neither page is defensible. Same cause as E3. |

Holding Phase 5 open until all three are met means holding it open indefinitely, with
every later phase stacked on an unmerged branch. R7, abandonment, is this project's
named top risk.

## Decision

A phase may be marked `complete` with a criterion **unmet**, if and only if that
criterion records:

- `carried_to_phase`: a later phase that exists and is not `skipped`;
- `carried_reason`: why it can't be met in this phase, at least 40 characters;
- `evidence`: the **measured** state at closing, as §10.8 requires of any evidence.
  "Deferred" is not evidence; "meaning 0.0472, `eval search --report`, 21d0b23" is.

The receiving phase inherits the criterion. When that phase closes, it must meet it, or
carry it again with a new reason.

§10.8 is unchanged. Every criterion, met or carried, still needs an artefact. What
changes is Appendix B: "every criterion is met" becomes "every criterion is met, or
carried with its measured state and a named destination".

## Consequences

- Phase 5 can close: E1 carried to Phase 21 (which already owns Tier 0 measurement), E3
  and E4 carried to Phase 16 (the recommender, the next phase that changes how the same
  vectors are ranked) or to a dedicated retrieval-quality phase if the author adds one.
- **The cost, stated plainly:** this is a permanent escape hatch. Nothing in the tooling
  can tell a genuine deferral from a tired one. The 40-character reason and the measured
  evidence make it visible, not impossible. Only the author polices it.
- Carrying a criterion repeatedly is the warning sign. `PROGRESS.md` lists carried
  criteria in their own section so they can't hide among met ones.
- A criterion can't be carried to a phase that's already complete, because nothing would
  ever check it again.

## What changes on approval

1. SPEC.md Appendix B: the completion sentence changes as above. `spec_version` gets a
   minor bump, and the change is logged in `## Amendments`.
2. CLAUDE.md "Definition of done": resynced in the same commit.
3. `docs/schemas/project-state.schema.json` and `tools/state/validate_state.py`: already
   drafted on branch `proposal/p14-carried-criteria`, with a test that a carried
   criterion lacking a reason is refused.

## Alternatives considered

- **Keep Phase 5 open.** Rejected: open-ended for E1 by construction, and for E3 with no
  known lever.
- **Carry only to a phase that already names the measurement.** Fits E1 alone; Phase 5
  stays open anyway, so it buys nothing.
- **Mark Phase 5 `skipped`.** It was built. That would be a different kind of untruth.

## Seen to fail (drafted tooling, 2026-10-25)

Against copies of the real state with Phase 3's E1 set unmet, the validator **refuses**:
unmet with no carry; a carry with no reason; a 5-character reason; a carry with no
evidence; a carry to phase 2 (not later); to phase 99 (not in the table); to a
`complete` phase; to a `skipped` phase; and a criterion that is both met and carried.
It **accepts** a carry to a later, unstarted phase with a 40+ character reason and
measured evidence. The real `PROJECT_STATE.json` still validates unchanged.

Found on the way: `anyOf` and `oneOf` were listed as known schema keywords but never
evaluated, so a rule written with either would have passed silently. Nothing used them
yet. Both are now evaluated.
