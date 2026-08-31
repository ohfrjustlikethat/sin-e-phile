---
name: unstuck
description: Recovery. Re-read state from disk, verify against git and the test suite, report any divergence, and state plainly where we are. For after a compaction, a crash, or when the thread has been lost. Use when the user types /unstuck or when you are unsure of the current state.
---

# /unstuck

You have lost the thread, or the context was compacted, or something crashed.
**Do not continue from a half-remembered plan.** Rebuild the picture from disk.

## 1. Trust nothing you remember

Read, in this order, from disk:

1. `PROJECT_STATE.json`
2. `docs/phases/phase-NN-<slug>.md` for `current_phase`
3. The last `SESSION_LOG.md` entry
4. `CLAUDE.md` — it is the standing rules and it may have changed

## 2. Verify against reality

```bash
git status                        # uncommitted work?
git log --oneline -10             # what actually landed
git branch --show-current         # the expected phase branch?
git tag -l "phase-*"              # which phases really closed
cargo test --workspace
npm test
python tools/statecheck/check.py
python tools/doctor/doctor.py
```

**The repository is right. The state file is a claim about the repository.**
Where they disagree, the state file is wrong — correct it and record the discrepancy
in `SESSION_LOG.md`.

## 3. Look for the specific divergences that actually happen here

- A phase marked complete whose criteria have no evidence strings.
- A phase behind `current_phase` left at `not_started` because the finished record
  went to `current_phase` and was never copied back. (This happened.)
- `PROGRESS.md` regenerated from a state file that has since changed.
- A phase doc that does not match `current_phase`.
- Uncommitted work in the tree that the state file does not mention.
- A `next_action` that is a direction rather than an instruction.
- Work finished in the last commit and never recorded. `statecheck` catches this one.

## 4. Report plainly

```
Where we are:   Phase N — <title>, <branch>, <n> commits since the tag
What landed:    <the last things that actually shipped, with SHAs>
What diverged:  <state file vs repository, or "nothing">
Uncommitted:    <files, or "clean">
Tests:          <pass/fail, with numbers>
Next action:    <from state, or the honest "this needs deciding">
```

Then stop. Do not start work. Let the author say what happens next — or run `/next`
if they ask you to carry on.

## 5. If you cannot reconstruct it

Say so directly. "I cannot tell whether X was finished; the state file says done, the
tests disagree, here is the evidence both ways" is far more useful than a confident
guess. Guessing is what makes a bad state worse.
