# Session Log

Append-only (`SPEC.md` §10.1). One entry per session. **Never edit past entries** —
a corrected fact goes in a later entry saying what changed and why.

---

## Session 1 — 2026-08-30 — Phase 0 (session 0a of 0a/0b): specification audit and safety rails

**Phase:** 0 — Bootstrap and Project Infrastructure · **Branch:** `phase/00-bootstrap`
**Spec version at end of session:** 1.1.0

### What was attempted

Session Zero (`SPEC.md` §10.7). Read `SPEC.md` in full, confirmed the five topics
the author asked about, audited the specification for contradictions, then — after
the author's rulings — executed the first half of Phase 0.

The author split Phase 0 into two sessions on the basis that it is honestly ~2
sessions of writing, not the 1 the spec estimates. **0a is the rails; 0b is the
documents.**

### What was completed

**The audit.** Twelve issues raised, all twelve ruled on by the author. Seven
required new design and became ADRs; ten were contradictions or imprecision fixed
directly. Highlights:

- §12.5 required a plaintext denylist of site names, which §2.1 forbids outright.
  The two sections directly contradicted each other.
- §12.5 had no allowlist at all, despite §2.1 requiring shipped legal sources.
- §12.2's "real-world messy filenames" corpus could not have been committed under
  §2.1 — a collision that would have surfaced in Phase 12 with 500 fixtures already
  hand-labelled.
- §14 marked TMDB required while §3.3 promised two-minute keyless onboarding.
- Tier B contained Phase 21 but not Phase 20, and Phase 21 depended on Phase 20 —
  so the tier designated as the definition of done was unreachable as specified.

**`SPEC.md` amended to 1.1.0** with 17 amendments logged under `## Amendments`,
each ADR-first per §2.8. Seven ADRs written (0009–0015) plus ADR-0001.

**The safety rails, all verified working:**

- `tools/guard/` — posture guard implementing ADR-0009/0010: plaintext structural
  patterns, salted-SHA-256 denylist, allowlist. Modes: `--staged`, `--tree`,
  `--history`, `--selftest`.
- `tools/guard/secretscan.py` — stdlib regex secret scan for the pre-commit path.
- `tools/doctor/doctor.py` — prerequisite checker and hook bootstrapper.
- `.githooks/pre-commit` and `commit-msg`, activated via `core.hooksPath`.
- `.github/workflows/ci.yml` — four jobs on `windows-latest`.
- Public repo created at `ohfrjustlikethat/sin-e-phile`, secret scanning and push
  protection enabled.

### Evidence (§10.8)

| Claim | Artefact |
|---|---|
| Guard fires on planted violations | `python tools/guard/guard.py --staged` on a planted `default_source_url` + announce URL → exit 1, 2 findings (`default-source-key`, `tracker-announce`) |
| Guard is quiet on a clean tree | `--tree`, `--history` → exit 0, "clean" |
| Guard self-test passes both directions | `python tools/guard/guard.py --selftest` → 30/30 (12 must-fire, 16 must-not-fire, 2 structural) |
| Secret scan fires on a planted fake key | `python tools/guard/secretscan.py --staged` → exit 1, 2 findings (`generic-api-key-assignment`, `github-token`) |
| Doctor reports missing prerequisites | PATH stripped of FFmpeg and Node → 3 MISS, exit 1, each with an actionable fix line |
| Doctor passes on a good machine | Full PATH → all required ok, exit 0 |
| commit-msg hook enforces §10.5 | Rejected a 77-character subject: "keep it under 72" |
| CI green on the phase branch | Run `33314370192`, all four jobs ✓ |
| Toolchain verified by real compile | `cargo new` + `cargo build` + run → "Hello, world!", host `x86_64-pc-windows-msvc` |

### What was learned / what broke

**The guard needed three rounds of tuning against the real tree: 118 false
positives, then 12, then 0.** Each round exposed a genuine design flaw, not a
typo — bare domains versus file extensions, URL hosts versus path segments,
attribute access in source files, and filesystem-walking versus git-tracked files.
This is the concrete argument for §12.5's insistence on *verifying* the guard: a
guard written and never tested would have shipped, produced constant noise, and
been bypassed within a week.

**The first CI run failed, correctly**, on the fourth of those: the tree scan read
the downloaded gitleaks archive's own README, which contains an example API key.

**A real bug was found by the deliberate plant**, exactly as the exit criterion
intends. The secret scanner's placeholder-exclusion list included the bare word
`a`, so any secret whose value began with `a` — including the planted
`a1b2c3…` TMDB key — was skipped entirely. Short, common placeholder words match
the start of real secrets and silently disable the rule.

**Environment gotcha, twice.** The tooling shell inherits a stale `PATH`, so
freshly-installed toolchains look missing. `doctor` now reads the registry `PATH`
and distinguishes "not installed" from "installed, restart your terminal". Related:
`vswhere` silently ignores Insiders/Preview Visual Studio installs unless passed
`-prerelease`, which made a present MSVC toolchain look absent.

### Blockers

None. The author installed Rust 1.98.0, Node 24.20.0 and MSVC 14.50.35717 during
the session; all verified.

### What the next session should do first

**Session 0b — the documents.** In order: `LICENSE` (GPL-3.0), `README.md` with
the pitch section drafted, `CONTRIBUTING.md`, `CHANGELOG.md`; then
`PROJECT_STATE.json` enumerating **all 28 phases with their exit criteria** plus
`docs/schemas/project-state.schema.json`; then `PROGRESS.md`; then `docs/SETUP.md`
with **live-verified** current terms for the §14 services; `docs/GLOSSARY.md`
(30 terms); `docs/HOW_IT_WORKS.md` skeleton; `docs/RISKS.md` (R1–R10, each with a
concrete trigger, including ADR-0015's ~30 ms Spike C escalation trigger);
`docs/DECISIONS_PENDING.md`; ADRs 0002–0008; and
`docs/learning/phase-00-notes.md`, whose five self-check questions must then be
**asked in chat and answered** before Phase 0 can be called done (§10.10).
