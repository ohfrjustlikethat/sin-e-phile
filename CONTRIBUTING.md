# Contributing

This is a personal portfolio project built to a fixed specification, so it is not
looking for feature contributions. Bug reports, correctness fixes and pointed
criticism are very welcome.

If you are reading this to evaluate the engineering rather than to contribute, the
files worth your time are `SPEC.md`, `docs/adr/`, and `tools/guard/README.md`.

## Ground rules

**`SPEC.md` outranks everything**, including convenience and including good ideas.
If a change requires deviating from it, the deviation is written up as an ADR and
the spec is amended *first* (§2.8). Never build contrary to the spec intending to
update it afterwards.

**No content sources. Ever. Anywhere.** No indexer URLs, no tracker names, no
default source URLs, no magnet links or infohashes outside clearly-marked legal
fixtures — not in code, config, tests, docs, or commit messages. See `SPEC.md` §2.1
and ADR-0006. This is enforced by `tools/guard/` on every commit and push, including
across the whole git history.

**Never suppress the guard to make CI pass.** If it fires, remove the content; if it
is already committed, rewrite history before pushing. If you believe it is a false
positive, fix the guard and add a vector to `tools/guard/tests/vectors.py` so the
case stays fixed.

**Evidence, not opinion.** A claim that something works needs an artefact: a passing
test name, a measured number *with the command that produced it*, or a file path.
`PROJECT_STATE.json` is schema-validated to reject "looks good" literally.

## Getting set up

```bash
git clone https://github.com/ohfrjustlikethat/sin-e-phile.git
cd sin-e-phile
python tools/doctor/doctor.py
```

Run `doctor` first. It reports what is missing and installs the git hooks by setting
`core.hooksPath` — until it runs once, **your commits are not being checked**.

## Before you push

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm run lint && npm run test

python tools/guard/guard.py --selftest
python tools/guard/guard.py --tree
python tools/state/build_state.py --check
python tools/state/validate_state.py --check
```

CI runs all of these on `windows-latest`.

## Commits

Conventional commits, enforced by the `commit-msg` hook:

```
type(scope): subject          # max 72 chars, imperative mood
```

Types: `feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore`
`revert`.

Write the body for someone reading it in six months with no memory of the change.
Explain *why*, not *what* — the diff already says what.

## Branches

One branch per phase, `phase/NN-slug`. Merged to `main` only when every exit
criterion for that phase is met **with evidence** recorded in `PROJECT_STATE.json`.
`main` must always build.

## If you found a bug

Open an issue with what you did, what happened, what you expected, and your
`python tools/doctor/doctor.py` output. If it is a **security** issue — especially
one touching the local HTTP server, addon manifest parsing, or path handling in
library scanning — please report it privately via GitHub Security Advisories rather
than in a public issue.
