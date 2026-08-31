# 0028 — The navigation system: phase docs, commands, and statecheck

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** between 3 and 4
- **Amends:** `SPEC.md` §10.2 and §13.2 (spec_version 1.6.0)
- **Partially reverses:** ADR-0016 A5 · **Relates to:** ADR-0016 (all), §10.11

## Context

The author's instruction, verbatim in intent:

> *"I don't want to write prompts like this one any more. I want to type a word and
> have you know exactly where you are, what's done, what's next, and how to recover
> if you get lost."*

That is a real cost being paid every session. It is also a real risk: `SPEC.md`
Appendix D rates **R7 — the project is abandoned partway** as High likelihood and
Severe impact, the most likely failure mode of the project and not a technical one.
Friction at the start of every session is exactly what makes R7 fire.

Three things had gone wrong and stayed wrong:

1. **The stale-pointer bug.** ADR-0016 A5 removed per-phase documents, but the
   session-start ritual kept telling every session to read
   `docs/phases/phase-NN-<slug>.md`. Those files stopped being generated after Phase
   0. For two phases, every session followed a pointer to nothing. Nothing detected
   it; it was found by reading.
2. **Orientation was a hunt.** With no phase doc, orienting meant `PROJECT_STATE.json`
   plus the last session-log entry plus `SPEC.md` §15 plus whatever the last session
   happened to remember — four sources, one of which is prose.
3. **Recording was voluntary.** Phase 2's record sat at `not_started` with no evidence
   for a day after Phase 2 was merged and tagged. Nothing complained.

## Decision

### 1. Per-phase documents come back, with a narrower job

`docs/phases/phase-NN-<slug>.md` is **the single file a session reads to know what it
is doing**. It is a working file, not a document: terse, bullets, no prose.

Generated and maintained by `tools/phasedoc/generate.py`, in three stages:

- **open** — at phase start, from `SPEC.md` §15: goal, deliverables, exit criteria as
  a live checklist, dependencies, risks.
- **log** — during the phase: checklists refreshed from `PROJECT_STATE.json`, one line
  per completed subtask with its commit.
- **close** — at phase end: outcome, evidence per criterion, debt incurred, and an
  unambiguous **"next phase starts by"**.

**This does not reverse ADR-0016 A5's actual point.** Phase *retrospectives* still
fold into the `SESSION_LOG.md` entry. What comes back is a generated working file,
not a hand-written narrative — the thing A5 was right to cut.

Generation is what makes this affordable. A5's cost was writing prose; there is none
here.

### 2. Six commands, as skills

`/status` `/next` `/finish` `/closephase` `/unstuck` `/decide`, in
`.claude/skills/<name>/SKILL.md`, with `docs/COMMANDS.md` as the cheat sheet.

These encode rituals that already existed in `CLAUDE.md` and `SPEC.md` §10. The
change is that invoking one costs a word instead of a paragraph, and that the ritual
cannot be half-remembered.

### 3. The session reading list is two files

`PROJECT_STATE.json` and the current phase doc. `SPEC.md` sections are read only when
the work touches them. This narrows §10.2 further in the same direction A1 already
took it.

### 4. Being lost is mechanically detected

`tools/statecheck/check.py`, in CI and on pre-push. Five checks, each one written
because that exact thing went wrong here:

1. the current phase's doc exists and matches `current_phase`;
2. `PROGRESS.md` is in sync with `PROJECT_STATE.json`;
3. every phase behind the current one is closed;
4. `next_action` is an instruction naming a file, not a direction;
5. **the most recent commit touching `src-tauri/`, `src/` or `crates/` is recorded in
   `PROJECT_STATE.json`** — and if not, some commit since it is.

Check 5 is the one that matters. The author's framing: it makes *"finished work but
forgot to record it"* mechanically impossible rather than something to remember.

It is deliberately satisfied by a *later* commit as well as the code commit itself.
Requiring the same commit would force state updates to be bundled into code commits,
which standing rule 4 (small commits) argues against, and would be routed around
within a week.

### 5. Decisions are put to the author in a fixed shape

A fifth protocol in `CLAUDE.md`: the decision in one sentence, two or three options
with honest costs, a recommendation, **and the default taken if the answer is "your
call"**. Where a reasonable default exists and nothing is blocked, the default is
taken and recorded rather than the session waiting on a reply.

## Consequences

- One session's work, against 24 remaining phases. The arithmetic is not close.
- `tools/phasedoc` reads `SPEC.md` §15 by regex. If §15's heading format changes, it
  fails loudly with the heading it expected rather than silently producing an empty
  document.
- `statecheck` runs on pre-push. `--no-verify` remains available for a deliberate
  work-in-progress push, and the hook says so.
- The phase doc's checklists are **regenerated** from `PROJECT_STATE.json` on every
  `--log`, so they cannot drift from it. Hand-editing a checkbox will be overwritten;
  edit the state file.
- Each check was verified to fire before being trusted, and `--selftest` keeps that
  true — including the case that must *stay quiet* (a `skipped` phase is legal, since
  Tier C and D are optional).
