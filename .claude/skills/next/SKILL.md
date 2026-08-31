---
name: next
description: Session start ritual, then plan and execute the next unit of work. The primary working command. Use when the user types /next or asks to continue, carry on, or start work.
---

# /next

The command that does the work. Orient, plan, get approval, execute, record.

## 1. Orient (do not skip, do not narrate)

- Read `PROJECT_STATE.json` and the current phase doc. **That is the reading list.**
  Not `SPEC.md` end to end. Read a `SPEC.md` section only if the work touches it.
- Run `python tools/statecheck/check.py`. If it fails, stop and run `/unstuck`
  instead — there is no point planning against a state you cannot trust.
- Run `python tools/doctor/doctor.py` if the last session did not, or if anything
  fails to build.
- **Verify against reality**: `git status`, `git log --oneline -5`, `cargo test
  --workspace`, `npm test`. The repository is right; the state file is a claim.
  Any disagreement gets corrected in the state file and recorded in `SESSION_LOG.md`.

If `last_updated` is more than 14 days old, run the cold-resume ritual
(`SPEC.md` §10.11) instead of this one.

## 2. Report — five lines, then plan

Same shape as `/status`. Then produce a plan for the next unit of work:

- The next incomplete subtask, or the next thing the phase doc's exit criteria need.
- What you will do, concretely. Files, approach, and what will prove it works.
- Anything you need decided — using the **decision protocol**: one-sentence decision,
  2–3 options with honest costs, your recommendation, **and the default you will take
  if the answer is "your call"**.

**Wait for approval before implementing.** That rule is not suspended by /next.

## 3. Execute

- Small commits, conventional messages, always-buildable `main`.
- Evidence, not opinion: a passing test name, a measured number and the command that
  produced it, a file path, a commit SHA, or `manual: <what was done and observed>`.
- **A check that has never been seen to fail is not evidence.** When you add a guard,
  audit or harness, break the thing it protects, confirm it fails, restore.
- Update `PROJECT_STATE.json` after **every** completed subtask, and run
  `python tools/phasedoc/generate.py --log <N>` to keep the phase doc current.
- Nothing blocks development, including the author's learning. Concepts go into the
  learning note as `concept + file:line`. Never pause the session to teach.
- Three genuinely distinct approaches failed → stop, record a blocker with
  `needs_user: true`, present costed options, recommend one, wait.

## 4. Before you stop

Run `/finish`, or say plainly what is left undone and why.
