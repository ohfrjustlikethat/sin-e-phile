# Commands

Type one word. Cheat sheet.

## The six

| Command | What it does |
|---|---|
| `/status` | Five lines: phase, what's done, next action, blockers, decisions waiting on you. |
| `/next` | Orients from state, plans the next unit of work, waits for approval, executes, records. **The one you'll use most.** |
| `/finish` | Session end: tests, lints, guard, state, PROGRESS, session log, phase doc, commit, push. |
| `/closephase` | Verifies every exit criterion has real evidence, merges, tags, opens the next phase doc. **Refuses if evidence is missing.** |
| `/unstuck` | Recovery. Re-reads everything from disk, verifies against git and the tests, reports what diverged. For after a crash or a compaction. |
| `/decide` | Every open decision, with options, honest costs, a recommendation, and the default that gets taken if you say "your call". |

If you only remember two: **`/next` to work, `/unstuck` when lost.**

## What each one reads

`/status` and `/next` read `PROJECT_STATE.json` and the current phase doc
(`docs/phases/phase-NN-<slug>.md`). That is the whole reading list — not `SPEC.md`
end to end. `SPEC.md` §15's entry for the phase is what the phase doc is generated
from.

## The tools behind them

| Command | Checks |
|---|---|
| `python tools/statecheck/check.py` | Can a session that knows nothing pick this up? Five checks; runs in CI and pre-push. |
| `python tools/statecheck/check.py --selftest` | Proves each of those checks actually fires. |
| `python tools/phasedoc/generate.py --open N` | Generate a phase doc from `SPEC.md` §15. |
| `python tools/phasedoc/generate.py --log N` | Refresh its checklists, append completed subtasks. |
| `python tools/phasedoc/generate.py --close N` | Write the closing section. Refuses without evidence. |
| `python tools/doctor/doctor.py` | Prerequisites, and whether your PATH is merely stale. |
| `python tools/guard/guard.py --tree` | Posture guard: no content sources, and no SQL under `src-tauri/`. |
| `python tools/guard/guard.py --selftest` | 48 vectors proving the guard fires and stays quiet correctly. |
| `python tools/state/validate_state.py --check` | Schema, evidence rules, and phase progression. |
| `npm run audit:contrast` | WCAG AA on every colour token pair. |
| `npm run audit:ui` | 60fps rail, focus rings, keyboard reach, reduced motion. |

## What statecheck refuses to let happen

1. The current phase has no document, or one that disagrees with `current_phase`.
2. `PROGRESS.md` is out of sync with `PROJECT_STATE.json`.
3. A phase behind the current one is left unclosed.
4. `next_action` is empty, or is a direction ("continue the torrent engine") rather
   than an instruction naming a file and an approach.
5. **A run of commits changed code and none of them touched `PROJECT_STATE.json`** —
   checked over the last five commits.

The last one is the important one. It makes *finished the work, forgot to record it*
mechanically impossible rather than something anyone has to remember.

It is a window rather than a strict "the newest code commit must touch state",
deliberately. The strict version fired on a trailing comment fix in a migration —
work already recorded one commit earlier — and a rule that demands a meaningless
edit to get past it is a rule that gets `--no-verify`'d within a week, and then
protects nothing.

## How decisions get put to you

Never as a raw problem. Always as: the decision in one sentence, two or three options
with honest costs, a recommendation with its reason, and **the default that gets
taken if you answer "your call"**. Two words is a complete answer.

Where a reasonable default exists and nothing is blocked, the default is taken, and
recorded, and you are told — rather than the project waiting on a reply.
