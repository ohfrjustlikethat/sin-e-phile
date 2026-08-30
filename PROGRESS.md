# Progress

> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by
> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two
> can never disagree). Edit the state file, then regenerate.

**Spec version 1.2.0** · 2 session(s) completed · last updated 2026-08-31

---

## Where we are right now

**Phase 0 — Bootstrap and Project Infrastructure** (`in_progress`, branch `phase/00-bootstrap`)

7 of 8 exit criteria met with evidence.

> Split into 0a (rails), 0b (documents) and 0c (lean-profile amendments, rulings on the open items, and E1). spec_version 1.2.0 shifts the project to the lean documentation profile of ADR-0016: shipping is the priority, learning is deferred.

### Subtasks — 15/17 complete

- [x] **0.1** git init, .gitignore, .env.example, phase branch, public GitHub repo, secret scanning + push protection · `bb6222a`
- [x] **0.2** Directory skeleton per SPEC.md §7 including the amended additions · `bb6222a`
- [x] **0.3** SPEC.md audit, 7 ADRs, 17 amendments to spec_version 1.1.0 · `7bbd241`
- [x] **0.4** tools/doctor/ prerequisite checker and core.hooksPath bootstrap · `deecf52`
- [x] **0.5** tools/guard/ posture guard, denylist, allowlist, self-test · `deecf52`
- [x] **0.6** Secret scanning: stdlib fallback + pinned gitleaks in CI · `78932d4`
- [x] **0.7** .githooks/ pre-commit and commit-msg via core.hooksPath · `deecf52`
- [x] **0.8** CI workflow, four jobs on windows-latest · `0aa6e23`
- [x] **0.9** Verify the rails fire on deliberate plants · `0aa6e23`
- [x] **0.10** LICENSE (GPL-3.0), README.md with pitch drafted, CONTRIBUTING.md, CHANGELOG.md · `66fd304`
- [x] **0.11** PROJECT_STATE.json with all 28 phases and exit criteria + JSON schema; PROGRESS.md · `66fd304`
- [x] **0.12** docs/SETUP.md with live-verified §14 service terms · `66fd304`
- [x] **0.13** docs/GLOSSARY.md (30 terms), docs/HOW_IT_WORKS.md skeleton · `66fd304`
- [x] **0.14** docs/RISKS.md (R1–R10 with concrete triggers), docs/DECISIONS_PENDING.md · `66fd304`
- [x] **0.15** Seed ADRs 0002–0008 (numbers reserved in docs/adr/README.md) · `66fd304`
- [~] **0.16** docs/learning/phase-00-notes.md written; the five self-check questions must now be ASKED IN CHAT and answered before Phase 0 is done (SPEC.md 10.10)
- [~] **0.17** Lean-profile amendments (ADR-0016), rulings P2/P3/P4 (ADR-0017/0018/0019), E1 fresh-clone CI job, docs/eval-results.md

### Exit criteria

- [x] **E1** `git clone` on a clean machine + following `docs/SETUP.md` produces a working dev environment.
      - *Evidence:* CI job 'Fresh clone builds (E1)' in .github/workflows/ci.yml: clones the branch fresh on a clean windows-latest runner with no local state, runs tools/doctor/doctor.py (must exit 0), asserts doctor bootstrapped core.hooksPath, builds per docs/SETUP.md, and re-runs the guard selftest and tree scan. LIMITATION, stated honestly: windows-latest ships Rust, Node, MSVC and the Windows SDK preinstalled, so this proves a clean CHECKOUT builds and that doctor runs green - it does NOT prove SETUP.md's install instructions are complete for a bare machine. A Windows Sandbox pass on a bare image is outstanding as P6. Chosen over a one-time manual check because it is repeatable and catches SETUP.md rotting later. Proven to detect a genuinely missing prerequisite: its first run failed because windows-latest ships no FFmpeg, and doctor reported exactly that (run 33326995943). The job now installs FFmpeg per SETUP.md, so it exercises that instruction rather than assuming it.
- [ ] **E2** CI passes on `phase/00-bootstrap`, and again on `main` after the merge. (§10.5 puts all phase work on a branch, so "green on `main`" can only be verified post-merge; both are required.)
- [x] **E3** `PROJECT_STATE.json` validates against its schema and enumerates all 28 phases with their exit criteria.
      - *Evidence:* python tools/state/validate_state.py --check -> 'PROJECT_STATE.json valid - 28 phases, 154 exit criteria'. The phase table is GENERATED from SPEC.md 15 by tools/state/build_state.py, so it cannot drift from the spec; build_state.py --check enforces that in CI. Schema at docs/schemas/project-state.schema.json. Verified to reject bad input: three planted violations (met-with-null-evidence, evidence 'looks good', and the spec's own anti-example next_action 'continue the torrent engine') produced 5 errors, exit 1.
- [x] **E4** Eight seed ADRs exist and are non-trivial.
      - *Evidence:* docs/adr/0001-0008 written and indexed in docs/adr/README.md. Each has Context / Decision / Consequences / Alternatives Considered, and each Alternatives section records what was actually rejected and why - ADR-0005 names sqlite-vec as the alternative most likely worth revisiting, ADR-0007 flags that GPL-3.0 makes the three extracted crates unusable by most of the Rust ecosystem (logged as P4). Seven further ADRs 0009-0015 came out of the Phase 0 spec audit.
- [x] **E5** The posture guard fails CI when fed a deliberately-planted test string, and passes on the clean tree. Verify it actually works — an unverified guard is worse than none, because it produces false confidence.
      - *Evidence:* Planted an RFC 2606 default_source_url + tracker announce URL, staged it: `python tools/guard/guard.py --staged` → exit 1, 2 findings (default-source-key, tracker-announce). Plant removed; `--tree` and `--history` → exit 0 clean. `--selftest` → 30/30 checks, covering 12 must-fire and 16 must-not-fire vectors.
- [x] **E6** Secret scanning fails CI on a deliberately-planted fake key.
      - *Evidence:* Planted a fake TMDB key and a fake GitHub token: `python tools/guard/secretscan.py --staged` → exit 1, 2 findings (generic-api-key-assignment, github-token). The plant exposed a real bug — `a` in the placeholder-exclusion list disabled the generic rule for any value starting with `a` — fixed in 78932d4.
- [x] **E7** `doctor` correctly reports a missing prerequisite when one is removed from PATH.
      - *Evidence:* PATH stripped of FFmpeg and Node: `python tools/doctor/doctor.py --no-fix` → 3 required MISS (Node.js, npm, FFmpeg), exit 1, each with an actionable fix line. Full PATH → all required ok, exit 0.
- [x] **E8** `docs/RISKS.md` exists with all Appendix D risks and at least one concrete trigger condition each.
      - *Evidence:* docs/RISKS.md covers all ten Appendix D risks plus a new R11 (third-party API terms change), each with owner, likelihood, impact, mitigation and at least one CONCRETE trigger. Examples: R3 fires if Spike C query-embedding p95 exceeds ~30 ms; R7 fires at 30 days without a commit; R10 fires if tools/guard/allowlist.txt gains a line without an ADR. R3's impact was raised to Moderate/Severe by ADR-0015.

---

## What's next

Phase 0 is not done. Two things remain, in this order. (1) ASK THE AUTHOR the five self-check questions at the end of docs/learning/phase-00-notes.md, in chat, and wait for real answers (SPEC.md 10.10). If they struggle on one, re-explain that concept differently and fix the note; if they struggle on most, say plainly that we went too fast and propose simplifying. (2) The author runs a clean-machine check for E1: clone the repo on a machine that has not built it, follow docs/SETUP.md, and confirm tools/doctor/doctor.py reports what is missing and that the build works - then record it as manual: evidence. Only then merge phase/00-bootstrap to main, confirm CI green on main to close E2, and tag phase-00. Phase 1 begins with the three de-risking spikes, and SPIKE C RUNS FIRST because ADR-0015 raised R3 to Moderate/Severe.

---

## Blockers

None.

---

## All 28 phases

Tiers are the legitimate stopping points from `SPEC.md` Appendix E. **Tier B is the definition of done** — complete it and the project has succeeded.

| | # | Phase | Tier | Depends on | Sessions | Criteria met |
|---|---|---|---|---|---|---|
| [~] | 0 | Bootstrap and Project Infrastructure | A | nothing | 1 | 7/8 |
| [ ] | 1 | Application Shell and Capability Tiers | A | 0 | 1–2 | 0/7 |
| [ ] | 2 | Design System and Visual Language | A | 1 | 1–2 | 0/5 |
| [ ] | 3 | Data Layer and Portable Storage | A | 1 | 1–2 | 0/5 |
| [ ] | 4 | Metadata Backbone | A | 3 | 2–3 | 0/7 |
| [ ] | 5 | Semantic Search Engine | A | 4 | 2 | 0/5 |
| [ ] | 6 | Source Resolver and Addon Protocol | A | 3 | 1–2 | 0/6 |
| [ ] | 7 | Torrent Engine and Streaming Server | A | 6 | 2–3 | 0/6 |
| [ ] | 8 | Player Core — MILESTONE: FIRST DEMOABLE BUILD 🏁 | A | 5, 7 | 2–3 | 0/6 |
| [ ] | 9 | Intelligent Source Selection | B | 8 | 1–2 | 0/6 |
| [ ] | 10 | Subtitle Pipeline | B | 8 | 2 | 0/6 |
| [ ] | 11 | Player Experience Layer | B | 8, 9, 10 | 2 | 0/5 |
| [ ] | 12 | Local Library Engine | B | 4 | 2–3 | 0/6 |
| [ ] | 13 | Download Manager and the Stream-vs-Download Advisor | B | 7, 9, 12 | 1–2 | 0/5 |
| [ ] | 14 | Profiles and First-Run Onboarding | B | 3, 5 | 2 | 0/6 |
| [ ] | 15 | Taste Model | B | 5, 14 | 2 | 0/5 |
| [ ] | 16 | Recommendation Engine | B | 15 | 2–3 | 0/6 |
| [ ] | 17 | Discovery Engine | B | 16 | 2 | 0/6 |
| [ ] | 18 | Browsing Surfaces 🏁 | B | 17 | 2–3 | 0/5 |
| [ ] | 19 | Watchlist and External Sync | C | 18 | 1–2 | 0/5 |
| [ ] | 20 | Windows Platform Integration | C | 12, 18 | 2–3 | 0/5 |
| [ ] | 21 | Performance Engineering and the Low-End Path | B | 18 (20 optional) | 2 | 0/4 |
| [ ] | 22 | Vision Layer (Tier 2) | D | 11 | 1–2 | 0/5 |
| [ ] | 23 | Binge Intelligence | C | 11, 12 | 1–2 | 0/4 |
| [ ] | 24 | Live Channels | D | 18 | 2 | 0/5 |
| [ ] | 25 | Manga and Comics | D | 3, 5, 16 | 2–3 | 0/5 |
| [ ] | 26 | Connected Playback | D | 11 | 2 | 0/5 |
| [ ] | 27 | Hardening, Packaging, and Portfolio Finalisation | B | everything | 2–3 | 0/5 |

Legend: `[x]` complete · `[~]` in progress · `[!]` blocked · `[?]` awaiting review · `[ ]` not started. 🏁 marks Phase 8 (first demoable build) and Phase 18 (complete product).

---

## Known debt

- **D1** (raised in Phase 0) The hashed denylist matches exact tokens only — no substring, fuzzy, or homoglyph matching. Accepted in ADR-0009; the structural matcher covers the shapes that carry real risk. Revisit only if a near-miss is ever observed.
- **D2** (raised in Phase 0) A fresh clone is unprotected by the git hooks until tools/doctor runs once, because core.hooksPath is per-clone config. CI is the backstop. Accepted in ADR-0012.
- **D3** (raised in Phase 0) Bare-domain detection is disabled inside source files (attribute access is shaped identically). URLs are still checked everywhere, as is the denylist. Documented in tools/guard/README.md.
- **D4** (raised in Phase 0) tools/state/validate_state.py implements a subset of JSON Schema draft 2020-12 by hand, because ADR-0012 fixed these tools as stdlib-only. It rejects any schema construct it does not implement rather than passing silently, but it is not a conformant validator. Revisit only if the schema needs constructs it lacks.

---

## Decisions pending

- **P1** — decide by Phase 27 Source-only distribution versus Phase 27 packaging and the 2.3 installed-size budget. See docs/DECISIONS_PENDING.md.
- **P5** — decide by Phase 12 Where the Phase 12 review-queue confidence threshold sits, given >95% top-1 and <1% false-confident pull against each other. Measure, do not guess. See docs/DECISIONS_PENDING.md.
- **P6** — decide by Phase 27 Windows Sandbox pass on a genuinely bare machine. The E1 CI job proves a clean checkout builds, but windows-latest ships Rust, Node and MSVC preinstalled, so it does not prove SETUP.md is complete from nothing.
