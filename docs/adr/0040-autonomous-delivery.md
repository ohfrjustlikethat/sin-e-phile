# 0040 — Autonomous delivery: Claude decides, builds, verifies and ships; the author is asked only about what could sink the app

- **Status:** Accepted 2026-11-05 by the author, who asked for it in these words: "make correct and most decisions, only leave very hard and detrimental to the entire app decisions for me, take every other decision, involve me only when in doubt … update all of spec and project … to pivot to this style of work."
- **Date:** 2026-11-05
- **Phase:** 6 → 7
- **Amends:** SPEC.md §2.8, §10.2, §10.5, §10.6, §10.9, §10.10, §14 (session estimates), Appendix C; `CLAUDE.md`; the `/next` and `/closephase` skills
- **Keeps:** every quality rule — evidence (§10.8), seen-to-fail, the posture guard, the secret scan, the regression policy (§10.12), state after every subtask

## Context

The process so far was built for a slow, supervised pace: a plan presented and approved
before every unit of work, every spec amendment approved first, a stop-and-wait after
three failed attempts, and an understanding gate that waits for answers. Phases 0–6 took
twenty sessions. The author now wants the whole app built fast — "in the next 2 days" —
without lowering the standard, and has said what the app should be across the spec and
twenty sessions of history.

The rules that slowed things down were the ones that *waited for the author*. The rules
that made the work trustworthy were the ones that *checked it*. This ADR removes the
first kind and keeps every one of the second.

## Decision

### 1. Claude decides

Claude takes every decision except the reserved ones below, records each one where the
next session will find it (an ADR for anything architectural, a note in the phase doc or
`known_debt` for anything smaller), and keeps moving. Plans are still written — into the
phase doc and `PROJECT_STATE.json` — but **are not waited on**.

**Reserved for the author** — and only these:

1. Anything that would change the **legal posture** (§2.1): what sources ship, what the
   guard allows.
2. Anything **irreversible outside the repository**: publishing a release, deleting data
   the author holds, spending money, creating accounts, anything on the author's behalf
   with a third party.
3. **Reversing a locked technology decision** (CLAUDE.md's table) or the licence.
4. **Cutting or redefining a Tier B deliverable** so the app the spec describes would not
   exist.
5. A choice where Claude is **genuinely in doubt about what the author wants** and the
   wrong guess would waste real work or hurt the app.

For these, the decision protocol in CLAUDE.md still applies — the decision in one
sentence, costed options, a recommendation, the default — and the default is **taken
when the work cannot sensibly wait**, then raised at the next report.

### 2. The spec is amended by Claude, in the open

§2.8's process stands — ADR, edit in place, bump `spec_version`, log the amendment, resync
`CLAUDE.md` in the same commit — except step 5. The author's approval is needed only for
an amendment in the reserved list above. Every amendment is listed in the next report so
the author always knows what changed.

### 3. Stuck means choose, not stop

After three genuinely distinct attempts (§10.9), Claude records the blocker with what was
tried, picks the best of the costed options — usually carrying the criterion to a named
phase under ADR-0036, or working another phase meanwhile — and continues. It stops only
if every option is in the reserved list.

### 4. Ship continuously, never unverified

Each unit of work is started, finished, tested, committed and pushed by Claude. Before
**every** push: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`,
`npm run lint`, `npm test`, `npm run build`, the posture guard, and the secret scan —
including **gitleaks over the full history**, which CI runs and which caught a false
positive on 2026-11-05 that the regex scanner missed. A phase is merged with `--no-ff`
and tagged by Claude once its criteria are evidenced or carried, CI is green on the branch,
and the regression check (§10.12) shows no earlier number moved; then CI must be green on
`main`.

### 5. Learning never blocks

Learning notes and their questions are still written every phase. At a tier boundary the
questions are put to the author in the chat, and **development continues while they are
answered**. A struggling answer still triggers a rewritten note and, if needed, a
simplification — as a task, not a stop.

### 6. Pace

Session estimates in §14 are dropped as a planning device; phases run back to back.
Speed comes from not waiting, from doing independent work in parallel, and from scoping
each phase to its exit criteria — **never** from skipping a check, marking a criterion met
without evidence, or carrying one that could be met now.

## What stays exactly as it was

Evidence, not opinion. A check that has never failed is not evidence. No content sources,
ever, enforced over history. No secret committed. State current after every subtask.
`main` always builds. The test suites and eval harnesses only grow. Honest reporting: a
criterion that cannot be met is said to be unmet.

## Consequences

- The author is interrupted far less, and must read the reports, which now carry the
  decisions taken on their behalf. A report always lists decisions taken and spec
  amendments made.
- The risk this ADR accepts is a wrong decision that costs work. It is bounded by the
  reserved list and by recording every decision where it can be found and reversed.
- R9 (the author stops understanding the code) gets worse at this pace. The learning notes
  are the mitigation, and the gate at Phase 8 still asks.
- "Two days" for 21 remaining phases is the author's target. Some phases need things only
  the author can do or only real hardware can measure — a Tier 0 machine, disabling a
  network adapter mid-download, a qBittorrent install, an Xbox controller. Those criteria
  are built and tested as far as code allows, and the remaining manual step is asked for
  in one batch rather than blocking the phase.
