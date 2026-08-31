---
name: status
description: Five lines on where the project is — phase, done, next action, blockers, decisions waiting on the author. Use when the user types /status or asks where things stand.
---

# /status

Answer in **five lines or fewer**. Nothing else. No preamble, no offer to help, no
restating the spec.

## Do this

1. Read `PROJECT_STATE.json`.
2. Read the current phase doc named by `current_phase` (`docs/phases/phase-NN-<slug>.md`).
3. Run `python tools/statecheck/check.py` — if it fails, that IS the status; say so.

## Output shape

```
Phase N — <title> · <n>/<m> subtasks · <n>/<m> criteria evidenced
Done:      <the last thing that actually landed, with its commit>
Next:      <next_action, verbatim if it is short enough to be useful>
Blockers:  <blocker, or "none">
Waiting:   <open decisions with needs_user, by id, or "nothing">
```

## Rules

- Five lines. If something does not fit, it was not the important thing.
- **Never say "complete" without evidence.** If a criterion is marked met with no
  evidence string, report that as a problem, not as progress.
- If `statecheck` fails, lead with it: a repository that cannot say where it is has
  no other status worth reporting.
- Do not offer next steps. `/next` does that.
