## What this changes

<!-- Why, not what. The diff already says what. -->

## Phase

<!-- e.g. Phase 7 — torrent-engine. Or "outside a phase" for a bug fix on main. -->

## Evidence

<!-- SPEC.md §10.8: artefacts, not opinions. A passing test name, a measured number
     WITH the command that produced it, a file path, or an explicit
     "manual: what I did and observed". Delete this section only for pure docs. -->

## Checklist

- [ ] `cargo fmt --all --check` and `cargo clippy -- -D warnings` clean
- [ ] `cargo test` and `npm run test` pass
- [ ] `python tools/guard/guard.py --selftest` and `--tree` pass
- [ ] `python tools/state/build_state.py --check` and `validate_state.py --check` pass
- [ ] `PROJECT_STATE.json` updated (subtask status, exit-criterion evidence, `next_action`)
- [ ] `PROGRESS.md` regenerated
- [ ] Docs updated for anything built
- [ ] An ADR exists for any non-obvious decision
- [ ] **No content sources anywhere**, including commit messages (SPEC.md §2.1)

## If this deviates from SPEC.md

<!-- §2.8: amend the spec FIRST, with an ADR and explicit approval. Link them here.
     Never build contrary to the spec intending to update it afterwards. -->
