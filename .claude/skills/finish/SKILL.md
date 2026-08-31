---
name: finish
description: Full session end ritual — tests, lints, guard, state file, PROGRESS, session log, phase doc, commit, push. Leaves the repo resumable by a session that knows nothing. Use when the user types /finish or says they are done for the session.
---

# /finish

Leave the repository so that a session which knows **nothing** can pick it up.
That is the whole test. Run every step; report any you skipped and why.

## 1. It builds and it passes

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run lint && npm test && npm run build
```

Failures are not "known issues". Either fix them, or record a blocker with an
explanation and say so out loud in the summary.

**Run `cargo fmt` immediately before committing, not once mid-session.** Files
written late have slipped past a mid-session format twice now.

## 2. The posture holds

```bash
python tools/guard/guard.py --tree
python tools/guard/guard.py --selftest
python tools/guard/secretscan.py --tree
```

## 3. The state is true

- `PROJECT_STATE.json`: subtask statuses, **evidence strings on every met criterion**,
  and a `next_action` that is an instruction naming a file and an approach — not
  "continue the X". `statecheck` enforces the shape; you own the substance.
- `python tools/state/validate_state.py --check`
- `python tools/state/validate_state.py --progress`
- `python tools/phasedoc/generate.py --log <N>`
- `python tools/statecheck/check.py` — **this must pass before you push.**

If a phase completed, write its record back into the `phases` array with a
`completion_commit`, and run `/closephase` rather than doing it by hand.

## 4. The record

- `SESSION_LOG.md`: what was built, the **measured numbers**, decisions, bugs found
  and what each one teaches, blockers, and what the next session should do first.
  Numbers go in the moment they are produced — ADR-0016 refuses to defer them.
- `docs/eval-results.md` for any eval or performance number: metric, value, date,
  commit, command.
- Learning note (`docs/learning/phase-NN-notes.md`): four sections, concepts as
  `concept + file:line`, no prose tour.
- `docs/specs/*.md` for anything built that another phase will build on.

## 5. Ship it

```bash
git add -A && git commit && git push
```

The pre-push hook runs the full-history guard and `statecheck`. If it refuses, it is
right — fix the state rather than passing `--no-verify`.

## 6. Tell the author

Short. What landed, the numbers, anything that needs them, and the single next
action. If something is genuinely unfinished, say which part and why — never round
"mostly done" up to done.
