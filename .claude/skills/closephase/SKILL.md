---
name: closephase
description: Verify every exit criterion has real evidence, merge, tag, generate the next phase doc, set next_action. Refuses if any criterion lacks evidence. Use when the user types /closephase or says a phase is finished.
---

# /closephase

Close a phase properly, or refuse. There is no third outcome.

## 1. Refuse first

Read every exit criterion in `current_phase`. For each, ask: **is there an artefact?**

An artefact is a passing test name, a measured number **with the command that
produced it**, a file path, a commit SHA, or an explicit
`manual: <what the author did and observed>`.

It is not "implemented and working". It is not "tested manually". It is not
"looks good".

**If any criterion lacks one, stop.** Say which, say what would produce the evidence,
and do not proceed. Marking a criterion met without evidence destroys every future
session's ability to trust the state file — that is a correctness bug, not a
formality.

`python tools/phasedoc/generate.py --close <N>` refuses on the same grounds. Let it.

## 2. Verify, then merge

```bash
cargo test --workspace && npm test          # everything, not the new thing
python tools/guard/guard.py --history
python tools/statecheck/check.py
```

Compare every eval and performance number against the previous phase's recorded
values. **A regression in an earlier phase's metric blocks the merge exactly as a
failing test does.**

Then: CI green on the branch → merge with `--no-ff` → CI green on `main` → tag
`phase-NN`.

## 3. Close the record

- `phases[N]`: status `complete`, `completion_commit` = **the merge commit**, and the
  evidence on every criterion. Do this *before* `current_phase` advances — the schema
  enforces it, and it caught a real one-day drift already.
- `python tools/phasedoc/generate.py --close <N>`
- Phase retrospective folded into the `SESSION_LOG.md` entry (ADR-0016 A5).
- `README.md` and `docs/HOW_IT_WORKS.md` updated to say what now exists — honestly,
  never over-claiming.

## 4. Open the next one

```bash
python tools/phasedoc/generate.py --open <N+1>
```

Set `current_phase` to N+1 and write a `next_action` that names a file and an
approach. Read `SPEC.md` §15's entry for N+1 to author its subtasks.

## 5. The understanding gate

Fires at **tier boundaries only** — end of Phase 8, end of Phase 18, the end of
whatever tier the author stops at, or on request (ADR-0016 A2). At a gate, ask the
accumulated self-check questions in chat and wait for real answers. Otherwise the
questions are written and left to accumulate.

If the phase just closed ends a tier, say so and suggest running Phase 27 so the
project is always presentable.
