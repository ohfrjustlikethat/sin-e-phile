# Phase 0 — Bootstrap and Project Infrastructure

**Depends on:** nothing. **Sessions:** 1 (actually 2 — see the retrospective).
**Branch:** `phase/00-bootstrap`.

---

## Phase spec

*(Copied from `SPEC.md` §15 at the start of the phase.)*

**Goal.** Create a repository that already looks like a serious project before it
does anything.

**Deliverables.** Git repo initialised and pushed. Full directory skeleton from
Section 7. `LICENSE` (GPL-3.0), `README.md` (skeleton with the pitch section
drafted), `CONTRIBUTING.md`, `CHANGELOG.md`. `PROJECT_STATE.json` with all 28 phases
enumerated and their exit criteria populated from this document, plus its JSON
schema. `PROGRESS.md`, `SESSION_LOG.md` with entry one. `docs/SETUP.md` listing
every prerequisite with verified current terms and step-by-step key acquisition.
`docs/GLOSSARY.md` seeded with 30 terms. `docs/HOW_IT_WORKS.md` skeleton. ADRs
0001–0008 recording the decisions already locked in Section 5. `.github/` with CI
workflow, issue templates, PR template. `.gitignore` per §12.6. `.env.example`.
Conventional-commit enforcement via a commit-msg hook.

**Also, and importantly — the safety rails:** `tools/guard/` (the posture guard,
§12.5) wired into CI and a pre-commit hook, with its denylist, built *before* there
is anything to catch. Secret scanning in CI and pre-commit (§12.6); GitHub push
protection and secret scanning enabled. `tools/doctor/` checking every prerequisite
and printing exactly what is missing. `docs/RISKS.md` — the Appendix D register with
owner, likelihood, impact, mitigation and trigger for each. `docs/DECISIONS_PENDING.md`.

**Exit criteria.** See `PROJECT_STATE.json` for live status and evidence.

**Learning note.** What each file in the repo root is for; what conventional commits
and ADRs are and why professionals use them; how CI works.

---

## Retrospective

### What was actually built

Everything specified, plus a substantial amount that was not — because reading
`SPEC.md` closely before starting turned up twelve real defects in it.

**The audit came first, and it changed the phase.** Twelve issues were raised and
all twelve ruled on by the author. Seven needed new design and became ADRs
(0009–0015); ten were contradictions or imprecision fixed directly. `SPEC.md` went
to spec_version 1.1.0 with 17 amendments, each ADR-first per §2.8.

The four that changed what Phase 0 *is*:

- **§12.5 required a plaintext denylist of site names; §2.1 forbids exactly that.**
  The guard could not be built as specified. Resolved by ADR-0009: salted-hash
  denylist plus plaintext structural patterns.
- **§12.5 had no allowlist at all**, despite §2.1 requiring shipped legal sources.
  A guard with only denial would have fired on the Internet Archive backend, and a
  guard that fires on correct code gets ignored. ADR-0010.
- **§12.2's realistic-filename corpus could not have been committed** under §2.1.
  This would have surfaced in Phase 12 with 500 fixtures already hand-labelled.
  ADR-0011 sets the redaction policy in advance.
- **Tier B contained Phase 21 but not Phase 20, and Phase 21 depended on Phase 20.**
  The tier designated as the definition of done was unreachable as specified.

Beyond the spec's list, three things were built that it did not ask for:

- `tools/state/build_state.py` — **generates** the 28-phase table by parsing
  `SPEC.md` §15, so the state file cannot drift from the specification. CI checks it.
- A **hand-rolled schema validator** (stdlib only, per ADR-0012) enforcing the §10.8
  evidence standard structurally rather than by convention.
- `docs/DECISIONS_PENDING.md` with five entries, three of which came from live
  verification of §14's service terms.

### What deviated from the plan, and why

**The phase ran two sessions, not one.** Flagged before starting rather than
discovered afterwards: Phase 0 as specified is eight ADRs, ~154 exit criteria, a
30-term glossary, a verified-terms setup document, a risk register, a guard, a
doctor, hooks and CI. That is two sessions of writing. Split into 0a (rails) and 0b
(documents) with the author's agreement.

**ADR numbering is non-chronological.** 0009–0015 were written before 0002–0008.
The seed decisions were reserved a contiguous block at the front of the index,
because that is where a first-time reader looks for them. Noted in `docs/adr/README.md`.

**Live verification of §14 turned up four constraints the spec did not record**,
and they are not cosmetic: TMDB forbids caching beyond 6 months and prohibits AI/ML
training use; MovieLens does not generally permit redistribution; Trakt has tightened
its free tier and is revising limits for 2026. These became risk **R11** and pending
decisions **P2** and **P3**.

### What was harder than expected

**The guard, by a wide margin.** Not the concept — the false-positive rate. Three
rounds against the real tree: **118 findings → 12 → 0**, each round a genuine design
flaw rather than a typo. Bare domains versus file extensions; URL hosts versus path
segments; attribute access in source files; and filesystem-walking versus
git-tracked files.

That last one was **caught by CI failing**, not by local testing: the tree scan read
a downloaded gitleaks archive's own README and reported someone else's example key
as a finding in this project.

**The deliberate plant found a real bug**, which is the whole argument for that exit
criterion existing. The secret scanner's placeholder list contained the bare word
`a`, so any credential whose value began with `a` was skipped silently. The planted
key began `a1b2c3`. It would have shipped as false confidence.

**The guard blocked two of the author's own commits** — once for a README sentence
that *described* a false positive by containing one, once for a 77-character commit
subject. Both fixed by removing the content rather than adding an exemption, which is
the behaviour §12.5 demands.

### What debt was incurred

Recorded in `PROJECT_STATE.json` under `known_debt`:

- **D1** — the hashed denylist matches exact tokens only. Accepted in ADR-0009; the
  structural matcher covers the shapes that carry real risk.
- **D2** — a fresh clone is unprotected by hooks until `doctor` runs once, because
  `core.hooksPath` is per-clone configuration. CI is the backstop.
- **D3** — bare-domain detection is disabled inside source files. URLs and the
  denylist still apply everywhere.

### What the next session should do

Phase 1, beginning with **the three de-risking spikes, before anything else**.
Spike C (ONNX Runtime) should run **first**, not last: ADR-0015 raised R3's impact
from Moderate to Moderate/Severe by establishing that query embedding runs on all
tiers, so an unusable `ort` now breaks semantic search everywhere. Its escalation
trigger — query-embedding p95 above ~30 ms — is pre-decided in `docs/RISKS.md`.
