# 0012 — Dev tooling in Python; git hooks via core.hooksPath

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0
- **Amends:** `SPEC.md` §12.5, §12.6, §14

## Context

Three gaps in `SPEC.md` around the Phase 0 safety rails, all discovered while
planning their implementation.

**No implementation language is specified for `tools/guard/` or `tools/doctor/`.**
Both must run as pre-commit hooks, which means they must work on a fresh clone
before `cargo build` has ever succeeded. `doctor` in particular exists to diagnose a
*broken or absent* toolchain — writing it in Rust would mean it cannot run in the
situation it was built for.

**No hook installation mechanism is specified.** `.git/hooks/` is not tracked by
git, so a hook committed to the repository does not activate on clone. `SPEC.md`
§12.5 and §12.6 both require hooks to run, without saying how they get there.

**`SPEC.md` §12.6 says "gitleaks or equivalent"** without resolving it. gitleaks is
neither a Rust nor a Node dependency, so it does not arrive with `cargo build` or
`npm install`.

## Decision

### Python 3.12 for `tools/guard/` and `tools/doctor/` — permanently

Not "for now". These are development tools, not shipped code. They never enter the
Tauri bundle, never affect installed size, and never touch the §2.3 performance
budgets. Rewriting them in Rust later would be churn with no user-visible benefit
and no learning benefit that the actual application code does not already provide
more directly.

Python also fits the job: regex scanning, walking git history via `subprocess`, and
producing readable diagnostics are things Python does well with zero dependencies.
Both tools use the **standard library only** — no `pip install` step, because a
hook that requires dependency installation is a hook that fails on a fresh clone.

**Python 3.12+ is added to `SPEC.md` §14 as a prerequisite** and to `doctor`'s own
checks. It is a *development* prerequisite; a user running a built application never
needs it.

### Hooks via `core.hooksPath`

Hooks live in a tracked `.githooks/` directory. Activation is
`git config core.hooksPath .githooks`, run by `tools/doctor/` as a bootstrap step so
that the first `doctor` run on a fresh clone wires them up. `doctor` reports hook
activation status explicitly, so an unwired clone is visible rather than silently
unprotected.

Two hooks:

- **`pre-commit`** — posture guard (staged content) and secret scan. Fast path only:
  it scans staged changes, not full history. Full-history scanning is CI's job,
  because a pre-commit hook whose cost grows with repository age eventually gets
  bypassed.
- **`commit-msg`** — Conventional Commits validation, per §10.5.

Hooks are POSIX `sh`. Git for Windows ships the shell that runs them, so this works
on the target platform without additional tooling.

### gitleaks pinned by version in CI; regex fallback locally

CI downloads a **version-pinned** gitleaks binary and verifies its checksum.
`tools/guard/secretscan.py` provides a standard-library regex fallback covering the
high-frequency key shapes relevant here — TMDB, Trakt, Fanart.tv, OpenSubtitles,
generic long hex and base64 secrets, private key headers — so the pre-commit hook
works whether or not gitleaks is installed locally. `doctor` reports which of the
two is active.

Defence in depth, deliberately: the local fallback is fast and approximate, CI is
thorough and authoritative, and GitHub push protection (ADR pending, §12.6) is the
backstop if both are bypassed.

## Consequences

**Easier.** The rails work on a fresh clone with nothing installed but git and
Python. `doctor` can diagnose a completely broken toolchain, which is its entire
purpose. No Node dependency for hooks, so no husky, and no `npm install` before the
guard protects anything.

**Harder.** A third language in the repository. Mitigated by scope: two
standard-library scripts that a reader can understand without knowing the project.

**Harder.** `core.hooksPath` is per-clone configuration. A fresh clone is
**unprotected until `doctor` runs once.** This is the weak point of the design and
it is accepted knowingly: CI is the backstop that catches anything a
missing-hook clone lets through, which is why the guard runs in both places rather
than only in the hook. `docs/SETUP.md` makes running `doctor` the first step.

**Harder.** The local regex fallback will produce occasional false positives on
high-entropy strings — a base64 test fixture, a long hash. The hook prints the
matching line and the reason, and `--no-verify` remains available for a genuine
false positive, but CI still runs gitleaks and will fail the push. The escape hatch
is local and temporary, never permanent.

## Alternatives Considered

**Guard and doctor in Rust as a `tools/` workspace crate.** Rejected: `doctor` must
run before the Rust toolchain is known to work, which is a circular dependency in
the one scenario it exists to handle. Also adds compile time to every hook
invocation, and slow hooks get bypassed.

**PowerShell.** Genuinely viable on a Windows-only project, and it needs no extra
prerequisite. Rejected on readability: the guard is regex-heavy and history-walking,
and Python expresses both far more clearly. Since a portfolio reader will open
`tools/guard/` specifically to check whether the legal posture is real, clarity
there has outsized value.

**husky + lint-staged.** Rejected: requires Node and `npm install` before hooks
work, which delays protection past the point where the first mistake can happen, and
adds a dependency tree to a security-relevant tool.

**gitleaks as the only secret scanner, no fallback.** Rejected: makes the
pre-commit hook silently non-functional on a machine without gitleaks, and silent
non-functionality in a safety rail is the worst possible failure mode. §12.5 makes
this point about the guard, and it applies identically here.

**Committing hooks to `.git/hooks` via a setup script instead of `core.hooksPath`.**
Rejected: copies drift from their source, and a stale copy is a hook that appears to
be running while enforcing an old rule.
