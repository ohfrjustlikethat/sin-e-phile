# Changelog

All notable changes to this project are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). This
project does not yet follow semantic versioning — it is versioned by **phase**
(`SPEC.md` §10.5 tags each completed phase `phase-NN`), and by `spec_version` for
the specification itself.

## [Unreleased]

### Phase 0 — Bootstrap and project infrastructure

**Added**

- Posture guard (`tools/guard/`) enforcing the zero-content-sources rule of
  `SPEC.md` §2.1 across the working tree and the full git history, with a 30-check
  self-test proving it fires when it should and stays quiet when it should not.
- Local secret scanner, plus a version-pinned, checksum-verified gitleaks in CI.
- `tools/doctor/` prerequisite checker, which also bootstraps the git hooks and
  distinguishes a missing tool from a stale shell `PATH`.
- `tools/state/` — generates the phase table by parsing `SPEC.md` §15 so the state
  file cannot drift from the specification, validates it against a JSON schema that
  rejects evidence-free claims, and regenerates `PROGRESS.md`.
- Git hooks: posture guard and secret scan on pre-commit, Conventional Commits on
  commit-msg.
- CI on `windows-latest`: posture, Rust, frontend and hygiene jobs.
- 15 architecture decision records.
- `docs/RISKS.md` with a pre-decided trigger for each of 11 risks;
  `docs/DECISIONS_PENDING.md`; `docs/SETUP.md` with live-verified service terms;
  `docs/GLOSSARY.md`; `docs/HOW_IT_WORKS.md`.
- GPL-3.0 licence.

**Changed**

- `SPEC.md` amended in 17 places to spec_version 1.1.0 (see its `## Amendments`
  section). The substantive ones: the posture guard redesigned so it no longer
  contradicts §2.1; an allowlist added, which §12.5 lacked entirely; TMDB made
  optional so the app works with no API key; Tier 0 clarified to embed queries but
  not documents; and Phase 21 unhooked from Phase 20, so that Tier B — the
  definition of done — is actually reachable.

**Fixed**

- Secret scanner missed any credential whose value began with `a`, because `a` was
  listed among the placeholder words. Found by a deliberately planted key, which is
  precisely what that exit criterion exists for.
- Posture guard produced 118 false positives on its first real run; three rounds of
  design fixes brought it to zero without weakening detection.
