# 0016 — Lean documentation and session profile

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 0
- **Amends:** `SPEC.md` §2.2, §10.2, §10.10, §11.1, §11.2, §13.2 → spec_version 1.2.0
- **Risk:** R9 (accepted, deliberately)

> Requested as ADR-0011; that number is taken by the fixture redaction policy and
> ADR-0001 forbids reuse, so this is 0016.

## Context

- The author reweighted the project's goals: **shipping is now the priority, the
  learning happens later.**
- The Phase 0 profile spent heavily on explanatory prose — full learning notes, a
  code tour, prose glossary entries, a prose system explanation.
- That prose is the one category of documentation that **can be regenerated from the
  code at any time**, and regenerated better later than it can be written now.
- Decisions, measured numbers and gotchas cannot. They exist only in the session
  that produced them.

## Decision

**Record what cannot be reconstructed; defer what can.** Six changes (A1–A6 in the
`SPEC.md` amendment log): reduced session reading, understanding gate at tier
boundaries only, 400-line explanations as note bullets rather than mid-session
pauses, four-section learning notes with no code tour, deferred case studies and
prose, and terse reporting.

**Never deferred:** every eval and performance number goes into
`docs/eval-results.md` the moment it is produced — metric, value, date, commit,
command.

**Explicitly not cut:** posture guard, §10.8 evidence strings, §10.9
three-attempts-then-stop, per-subtask state updates, `SESSION_LOG.md`, ADRs, eval
numbers. These are the resume and safety mechanisms; they are cheap, and losing one
costs far more than it saves.

## Consequences

- **Faster phases.** Less token spend per phase means more phases per unit of
  effort, which is the direct mitigation for **R7 (abandonment)** — the project's
  highest-likelihood, highest-impact risk.
- **R9 (author falls behind) rises**, knowingly. The mitigation moves from
  continuous to periodic: questions are still written every phase, then asked in a
  batch at a tier boundary. The failure mode is discovering a gap at Phase 18 rather
  than Phase 12.
- **A `file:line` pointer rots** when code moves. Accepted: a stale pointer is a
  cheap `grep`, whereas stale prose is misleading.
- **Numbers become the single non-negotiable documentation output.** If eval
  recording ever slips, §10.12's regression policy silently stops working — a
  quality regression would go unnoticed rather than blocking a merge.
- Reading `SPEC.md` selectively risks missing a constraint in an unread section.
  `CLAUDE.md` carries the standing rules, and cold resume keeps the full reading.

## Alternatives Considered

- **Keep the full profile.** Rejected: highest-quality documentation, but it is R7
  that kills this project, not R9, and the prose is recoverable.
- **Cut documentation across the board, including ADRs and evidence strings.**
  Rejected: those are what make a resume possible after a month's gap. Cutting them
  saves little and costs the ability to continue.
- **Defer the numbers too, reconstructing them at Phase 27 by re-running harnesses.**
  Rejected: re-running gives *today's* number, not the number at that commit, so
  regression detection is impossible retrospectively.
- **Drop learning notes entirely.** Rejected: the concept/`file:line` list is
  near-free to write and is exactly the part that cannot be reconstructed — nobody
  will later remember which file first used `Arc<Mutex<T>>`.
