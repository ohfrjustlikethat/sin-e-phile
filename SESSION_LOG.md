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


---

## Session 2 — 2026-08-31 — Phase 0 (session 0b of 0a/0b): the documentation set

**Phase:** 0 — Bootstrap and Project Infrastructure · **Branch:** `phase/00-bootstrap`
**Spec version:** 1.1.0 (unchanged this session)

### What was attempted

The second half of Phase 0: the documents and the state system. Session 0a built the
safety rails; this session wrote everything that has to exist for a future session —
or an employer — to understand the project.

### What was completed

**Seed ADRs 0002–0008**, recording the decisions `SPEC.md` §5 locked before the
repository existed: Tauri over Electron, librqbit over libtorrent, libmpv over
libVLC, SQLite + FTS5 + HNSW over a vector database, the ships-empty source posture,
GPL-3.0, and portable-by-default storage. Each states what was actually rejected and
why. Two produced new pending decisions: ADR-0005 flags `sqlite-vec` as the
alternative most likely worth revisiting, and ADR-0007 flags that GPL-3.0 makes the
three extracted crates unusable by most of the Rust ecosystem — which defeats the
point of extracting them (**P4**).

**The state system**, which turned out to be the interesting engineering:

- `tools/state/build_state.py` **generates** the 28-phase table with all 154 exit
  criteria by parsing `SPEC.md` §15, rather than transcribing them. Transcription
  would create a second copy of the specification that drifts the moment the spec is
  amended. It also derives branch slugs by the spec's own convention, reproducing
  §10.1's `torrent-engine` example exactly.
- `docs/schemas/project-state.schema.json` plus a **hand-rolled stdlib validator**
  (ADR-0012 forbids `pip install` in these tools). It makes §10.8 structural: a
  criterion with `met: true` and no evidence fails validation, and the banned
  phrases are rejected by pattern.
- `PROGRESS.md` is now **generated** from the state file, so §10.1's "the two can
  never disagree" is mechanical rather than a promise. CI fails if it is stale.

**Documentation:** `README.md` with the pitch section written properly,
`docs/RISKS.md`, `docs/SETUP.md`, `docs/GLOSSARY.md`, `docs/HOW_IT_WORKS.md`,
`docs/DECISIONS_PENDING.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `LICENSE` (GPL-3.0
fetched canonically, not typed), GitHub issue and PR templates, and stubs for
`ARCHITECTURE`, `PERFORMANCE`, `MANUAL_TESTS` and `INTERVIEW_PREP` that say honestly
what they are waiting on.

**`docs/learning/phase-00-notes.md`** and `docs/phases/phase-00-bootstrap.md` with
its retrospective.

### Evidence (§10.8)

| Claim | Artefact |
|---|---|
| E3 — state validates and enumerates 28 phases | `python tools/state/validate_state.py --check` → "valid — 28 phases, 154 exit criteria". Table generated from `SPEC.md` §15; `build_state.py --check` enforces sync in CI. |
| The schema actually rejects bad input | Planted three violations — `met:true` with null evidence, evidence `"looks good"`, and `next_action: "continue the torrent engine"` (the spec's own anti-example) → 5 errors, exit 1. Restored, revalidated clean. |
| E4 — eight seed ADRs, non-trivial | `docs/adr/0001`–`0008`, each with a real Alternatives Considered section. Plus 0009–0015 from the audit. |
| E8 — risk register with concrete triggers | `docs/RISKS.md`: 11 risks, each with an observable trigger. R3 → Spike C p95 > ~30 ms. R7 → 30 days without a commit. R10 → allowlist gains a line without an ADR. |
| Guard still clean after ~40 new files | `--selftest` 31/31, `--tree` and `--history` clean. |
| §14 terms verified live | TMDB, AniList, Jikan, IMDb, MovieLens, OpenSubtitles, Trakt fetched 2026-08-30/31; dates recorded per service in `docs/SETUP.md`. |

### What was learned / what broke

**The schema caught me.** I marked the 0b subtasks complete with `commit: "PENDING"`,
because the commit that completes them does not exist while they are being written.
Validation rejected it. The correct ordering is: commit the work, then record the
real SHA — which is now what happened, in two commits. A small thing, but it is the
schema doing exactly its job on its author.

**The guard blocked the GPL-3.0 licence.** The canonical text cites `fsf.org`, which
is not on the allowlist. The fix was to add it to the guard's *infrastructure* set
rather than the allowlist — ADR-0010 scopes the allowlist to content and metadata
sources, and the FSF is neither — plus a regression vector, per `CONTRIBUTING`: fix
the guard, add a vector, never add an exemption. Self-test now 31 checks.

**Live verification of §14 was worth doing and produced four findings the spec did
not record**, three of which constrain later phases: TMDB forbids caching beyond
6 months (Phase 4 TTL design) and prohibits AI/ML training use (**P2** — embeddings
are inference, but that reading needs a ruling, not an assumption); MovieLens does
not generally permit redistribution (**P3** — bears on shipping a derived item-item
matrix in Phase 16); Trakt has tightened its free tier and is revising limits for
2026. These became risk **R11**.

Worth noting: the OpenSubtitles free tier is **20 downloads/day with an account, 5
without** — even tighter than §14's "single-digit" note implies for the no-account
case. The Phase 10 design already assumes this and puts embedded tracks first.

### Blockers

**B1, `needs_user: true`.** Exit criterion **E1** — clone on a clean machine plus
`docs/SETUP.md` produces a working dev environment — **cannot be evidenced from this
machine**, which already has every prerequisite. `doctor` was verified both with a
full PATH (all pass, exit 0) and a stripped one (3 MISS, exit 1, each with a fix
line), so the diagnostic path works — but neither proves the clean-machine path.
Per §10.8, evidence that cannot be produced means the criterion is **not met**, and
we say what is blocking it rather than marking it done.

### What the next session should do first

**Phase 0 is not finished.** In order:

1. **Ask the author the five self-check questions** at the end of
   `docs/learning/phase-00-notes.md`, in chat, and wait for real answers (§10.10).
   Struggling on one means re-explaining and fixing the note; struggling on most
   means saying plainly that we went too fast.
2. **The author clears B1** by cloning onto a clean machine or VM and following
   `docs/SETUP.md`, recorded as `manual:` evidence.
3. Then merge `phase/00-bootstrap` to `main`, confirm CI green there to close **E2**,
   and tag `phase-00`.

Phase 1 then begins with the three de-risking spikes — and **Spike C runs first**,
not last, because ADR-0015 raised R3's impact to Moderate/Severe by establishing
that query embedding runs on every tier.


---

## Session 3 — 2026-08-31 — Phase 0 session 0c: lean profile, rulings, Phase 0 closed

**Phase:** 0 → complete. **Branch:** `phase/00-bootstrap` → squash-merged to `main`,
tagged `phase-00`. **spec_version:** 1.1.0 → **1.2.0**.

### What was done

- **Lean documentation profile adopted** (ADR-0016, six amendments A1–A6). Author
  reweighted the goals: shipping is the priority, learning happens later. Principle:
  record what cannot be reconstructed, defer what can.
- **Rulings on the open items**, one ADR each: 0017 (dual-license the crates),
  0018 (TMDB inference-not-training + swappable text source), 0019 (MovieLens matrix
  computed on device).
- **E1 evidenced by a fresh-clone CI job** rather than a one-time manual check.
- **`docs/eval-results.md` created** — the one never-deferred documentation output.
- Phase 0 closed: 8/8 exit criteria met with evidence.

### Retrospective (folded in per ADR-0016)

**Deviated:** Phase 0 ran three sessions, not the spec's one. 0a rails, 0b documents,
0c amendments and closure. The spec's session counts are now explicitly estimates.

**Harder than expected:** the guard's false-positive rate, three rounds (118 → 12 →
0). Every round was a design flaw, not a typo.

**Debt incurred:** D1 exact-token-only denylist; D2 fresh clone unprotected until
doctor runs; D3 bare-domain detection off inside source files; D4 hand-rolled schema
validator implements only a subset of draft 2020-12.

### Gotchas worth keeping

- **`vswhere` needs `-prerelease -all`** or it silently ignores Insiders/Preview
  Visual Studio and a present MSVC toolchain looks absent.
- **The tooling shell inherits a stale `PATH`** — freshly installed toolchains look
  missing. `doctor` reads the registry `PATH` to tell the two cases apart.
- **`npm` is a `.cmd` shim**; `CreateProcess` cannot run it directly, so it must go
  through `cmd /c`.
- **Canonical licence texts trip the guard** — GPL cites `fsf.org`, Apache-2.0 cites
  `apache.org`. Both are legal infrastructure, not content sources, so they belong in
  `INFRASTRUCTURE_DOMAINS`, not the allowlist (ADR-0010 scopes the allowlist to
  content and metadata sources). Each fix got a regression vector; selftest is now 32.
- **`windows-latest` ships Rust, Node and MSVC but not FFmpeg.** The E1 job caught
  this on its first run, which is the job working. It now installs FFmpeg per
  `SETUP.md`, exercising that instruction rather than assuming it.
- **Bash heredocs in this environment break on prose apostrophes.** Write generator
  scripts to the scratchpad and execute them instead.

### Evidence (§10.8)

| Claim | Artefact |
|---|---|
| E1 — fresh clone builds | CI job "Fresh clone builds (E1)", run 33327204841. Proven to detect a real missing prerequisite (no FFmpeg) on run 33326995943. Limitation stated in the evidence string: the runner ships Rust/Node/MSVC, so it proves a clean checkout builds, not that SETUP.md is complete from nothing. Bare-machine pass outstanding as P6. |
| E2 — CI green on branch and main | Branch run 33327097633; main run 33327204841 at `cc6a89e`, all five jobs. |
| Phase 0 complete | `python tools/state/validate_state.py --check` → 8 of 8 criteria met with evidence. Tag `phase-00`. |
| Guard still clean after the licence files | selftest 32/32; `--tree` and `--history` clean. |

### Blockers

None. B1 cleared by the E1 CI job.

### Next session

**Phase 1, spikes first, in order A → B → C**, before any other Phase 1 work. Each is
throwaway code in `spikes/`, ~2 hour timebox, findings to `docs/RISKS.md` and numbers
to `docs/eval-results.md`. **If a spike's trigger fires, escalate under §10.9 and
stop** — do not proceed hoping it works out.

Note the author's ordering overrides the earlier suggestion to run Spike C first;
A → B → C is the spec's own order and the author reaffirmed it.


---

## Session 4 — 2026-08-31 — Phase 1: all three spikes, then the application shell

**Phase:** 1 — Application Shell and Capability Tiers · **Branch:** `phase/01-application-shell`
**Spec version:** 1.3.0 · **Status:** code-complete, 7/7 exit criteria evidenced, not yet merged

### What was done

**All three de-risking spikes, and none failed** — so no fallback was taken on any
locked technology decision, and R1, R2 and R3 are all retired.

**Spike A** ran long and is written up in full in `docs/learning/phase-01-notes.md`
by the author's request, for the Phase 27 case study. Short version: HTML cannot be
composited over video on Windows using child-HWND z-order, confirmed independently
by `ventic/ventic` who ship the same architecture. Solved by inverting the problem
(ADR-0021): still-frame substitution on pause, region cutouts during playback.
`SPEC.md` §9.3 amended deliberately (ADR-0020) rather than carrying a requirement
the platform cannot meet.

**Spike B** — librqbit. TTFB 1.0/2.9/3.1 s against a 20 s trigger; seek
re-prioritisation 0.6/0.8/2.4 s against a 5 s target. The API audit mattered more
than the numbers: `ManagedTorrent::stream` gives a position-tracking 32 MiB priority
window that the piece picker already honours, so **much of the Phase 7 scheduler
already exists**. Also found that **librqbit has no webseed support**, which means
Phase 6's Internet Archive backend must use direct HTTP rather than torrents (D6).

**Spike C** — `ort`. Query-embedding p95 **1.63 ms** at true query length, 8.13 ms
padded, against a 30 ms trigger. Load 82 ms, resident +51.6 MB.

**The application shell**: Tauri v2 + React 19 + TS strict + Vite + Tailwind 4,
custom title bar, five-destination nav rail, generated IPC, `crates/tiers`, Settings,
logging and a crash handler.

### Evidence (§10.8)

| Claim | Artefact |
|---|---|
| Cold start to interactive | 515 / 660 ms, release, logged as `cold_start_ms` in `data/logs/` |
| Idle RAM | 42.2 MB (`WorkingSet64`), release binary 10.1 MB |
| Rust signature change breaks the TS build | Added a `u32` arg to `has_capability` → `ipc.ts` regenerated → `npm run build` failed `TS2554` at `SettingsScreen.tsx(145,64)`. Reverted, clean. |
| Deliberate panic writes a crash log | `SINEPHILE_PANIC_TEST=1` → `crash-1788162818.txt` with version, location, message, backtrace |
| Tier detection | `Capable`, 32189 MB, 24 cores, RTX 5070 Ti, hw decode — logged at startup; 8 boundary tests in `crates/tiers` |
| Test + lint status | `cargo test --workspace` 8 passed; clippy and fmt clean; `npm test` 9 passed; lint and build clean |

### What broke, and what it taught

**`cargo test` cannot run inside `src-tauri` on Windows at all.** Every test binary
dies at load with `STATUS_ENTRYPOINT_NOT_FOUND`, because tao imports comctl32 **v6**
entry points and `cargo test` binaries get no side-by-side manifest. Three fixes were
tried and all failed: rustflags apply to every dependency (`LNK1327`),
`rustc-link-arg` duplicates the manifest `tauri-build` already embeds (`CVT1100`),
and `rustc-link-arg-tests` — which would be exactly right — **is rejected by stable
cargo**, verified in an isolated crate.

**This is the session's most consequential finding.** It would have blocked unit tests
for the filename parser, the source scorer, the aligner and the recommender.
ADR-0022: `src-tauri` carries no test harness and all testable logic lives in
`crates/`. That makes §7's `crates/` split **load-bearing rather than aspirational** —
it was framed as being about reuse, and it turns out to be the only way pure logic is
testable at all.

**Specta forbids `u64`/`usize` across the IPC boundary**, because JS numbers are f64
and would lose precision above 2^53. `u32` is not a workaround here but the correct
type: 4 billion MB is four petabytes of RAM.

**Cold start was nearly measured dishonestly.** Timing to `MainWindowHandle` gave
267 ms — for a window with nothing in it. The window is now created hidden and
revealed only when the frontend reports it has painted, which roughly doubles the
number to something true.

**A scripted edit wrote a literal newline into Rust source** instead of the `\n`
escape. Output was still correct, so only CI's `cargo fmt --check` caught it.

**CI caught two ordering bugs local runs could not**, because `dist/` already existed
locally: `tauri::generate_context!()` resolves `frontendDist` at *compile* time, so no
cargo command runs without it, and the fresh-clone job built Rust before the frontend.
Also widened the Rust job to `--workspace`, which had been silently skipping
`crates/tiers` and its 8 tests.

**Guard needed three additions**, each with a regression vector rather than an
exemption: npm registry and funding domains (lockfiles are committed per R8), a
`SELF_IDENTIFIERS` set for the app's own reverse-DNS bundle id, and `src-tauri/gen/`
ignored as build output. Selftest is now 37 checks.

### Blockers

None.

### What the next session should do first

Verify CI is green on `phase/01-application-shell`, then merge to `main`, confirm
green there, and tag `phase-01`. Then **Phase 2 — design system**, which depends only
on Phase 1 and is unblocked.

**The understanding gate does not fire yet.** ADR-0016 moved it to tier boundaries,
so the questions written in the Phase 1 note accumulate and are asked with Phases 1–8
at the end of Phase 8.

Two items outstanding for the author, neither blocking: **P8** (Tier 0 embedding
measurement on constrained hardware, before Phase 21) and the **TMDB enquiry** still
drafted and unsent at `docs/correspondence/tmdb-ai-clause.md`.

---

## Session 5 — 2026-08-31 — Phase 2: design system, built from the chosen mockup

**Phase 2 complete.** All five exit criteria met with evidence. Took Take B with
Take A's 74vh hero (ADR-0024), built tokens and every component from it, then
measured the claims instead of asserting them.

### Built

Tokens (`src/styles/tokens.css`) with warm non-linear greys, three separated ink
levels all clearing AA, and the `--oxblood` / `--oxblood-text` split so an accent
that fails as small text cannot be used as small text. Fonts bundled locally, no
network request. 15 primitives, 6 media primitives, a virtualised `Rail`, the
command palette, and a dev-only `#design` gallery rendering every one of them in
every state.

`docs/specs/design-system.md` documents all of it. `docs/learning/phase-02-notes.md`
carries the five self-check questions — which do **not** fire yet; ADR-0016 moved the
gate to tier boundaries, so Phases 1–8 are asked together at the end of Phase 8.

### The measured numbers

```
rail       14/500 mounted · median 16.7ms · p95 16.7ms · worst 16.8ms · 0 dropped
keyboard   rail is 1 tab stop; End reaches card 499 of 500
focus      45 stops, 18 distinct, 0 without a ring
motion     38 non-opacity transitions normally → 0 under reduce
contrast   29 enforced pairs pass WCAG AA (3 decorative recorded, not enforced)
```

### Four silent bugs, none visible on screen

This is the story of the phase. Not one produced an error message, and the app
looked fine throughout.

1. **Virtualisation was doing nothing.** A grid item defaults to `min-width: auto`
   and will not shrink below its content, so the 97,060px track expanded its `1fr`
   column to 97,060px. The `ResizeObserver` then reported a viewport that wide and
   all 500 cards mounted. The virtualisation code was correct and ran happily.
2. **The rail rendered at zero height** — absolutely-positioned children contribute
   none. Replaced absolute positioning with a leading spacer in normal flow, which
   removes the class of bug rather than patching it.
3. **487 of 500 cards were unreachable by keyboard.** Virtualisation breaks Tab
   silently: only the mounted window exists in the DOM, so Tab walked ~13 cards and
   left the rail. More overscan only moves the wall. Fixed with a roving tabindex —
   the rail is one Tab stop, arrows move through all 500, scrolling each card into
   existence before focusing it.
4. **`prefers-reduced-motion` was adding motion.** `transition-duration` on `*`
   without pinning `transition-property` makes *every* property animate, because the
   property defaults to `all`.

Also fixed on the way: the app crashed in any plain browser because
`@tauri-apps/api` reads `window.__TAURI_INTERNALS__.metadata` at render — the title
bar now degrades instead of taking the whole app down, which is what let the gallery
be rendered and audited at all. PosterCard printed the title twice in its
artwork-free state. Card spine numbers had no legibility scrim over bright frames.
The gallery had never been shown with real artwork; 24 real public-domain stills now
live in `public/stills/`.

### New harness — `tools/uiaudit`

The exit criteria include "60fps with 500 cards", "keyboard-navigable with visible
focus at every step", and "respects `prefers-reduced-motion`". None can be signed off
from a screenshot, and §10.8 does not accept "looks good".

So: ~300 lines driving the real gallery in headless Chrome over the DevTools
Protocol, with **zero dependencies** (Node 24 has a built-in WebSocket client).
Wired into CI alongside the contrast audit.

**Both audits were verified to fail before being trusted.** Reverting `min-w-0` →
`rail is not scrollable`, exit 1. Removing `transition-property: opacity` → `180
non-opacity transitions survive`, exit 1. A check never seen to fail is not evidence.

Building the harness cost about an hour to a self-inflicted problem worth recording:
Chrome's profile was written inside the project, Vite watches the project tree, and
Chrome keeps `Cookies` locked — the watcher hit `EBUSY` and Vite exited after serving
one request. The symptom was a blank page with no error, because the harness was
discarding the dev server's stderr and only listening for `Runtime.exceptionThrown`.
A module that fails to *fetch* never throws. It now logs both.

### Blockers

None.

### What the next session should do first

Verify CI is green on `phase/02-design-system`. Two new jobs run there — **Contrast
audit** and **UI audit** — and the UI audit needs Chrome on the runner, so check that
step specifically rather than trusting a green tick. Then merge, confirm green on
`main`, tag `phase-02`, and start Phase 3.

Still outstanding for the author, neither blocking: **P8** (Tier 0 embedding
measurement before Phase 21) and the **TMDB enquiry** drafted at
`docs/correspondence/tmdb-ai-clause.md`.

### State correction — Phase 2 record

`phases[2]` still read `not_started` with no evidence after Phase 2 was merged and
tagged: the completed record had been written to `current_phase` only, never copied
back into the `phases` array the way Phase 1's was. The repository was right and the
state file was wrong, so the record was synced from `current_phase` and given
`completion_commit: 3c2077f` (the merge). 20 of 154 exit criteria now carry evidence.

Also noted, not a defect: there is no `docs/phases/phase-NN-*.md` for Phases 1–3.
Only `phase-00-bootstrap.md` exists. Per-phase documents stopped being written under
the lean profile (ADR-0016) — the phase specification lives in `SPEC.md` §15 and the
working record in `PROJECT_STATE.json`. A `next_action` written last session pointed
at `docs/phases/phase-03-*.md`, which does not exist; the phase was read from
`SPEC.md` §15 instead.

---

## Session 6 — 2026-09-01 — Phase 3: the data layer, plus four rulings

Phase 3 code-complete, all five exit criteria evidenced. Also carried out four
instructions from the author before starting.

### The four rulings

**① Persistence location.** `crates/persistence` with `src-tauri/src/persistence/`
as re-exports, as proposed — ADR-0022 already decided it and `SPEC.md`'s own Phase 3
wording ("no raw SQL outside `persistence/`") permits it; CLAUDE.md's paraphrase was
the narrow one. Made structural rather than remembered: the guard now fails on `sqlx`
or a SQL literal anywhere under `src-tauri/`, and on any statement in the re-export
module that is not a re-export.

**② CLAUDE.md resynced against SPEC.md 1.4.0** (not 1.2.0 — three amendment rounds
had landed). Eleven disagreements, listed in the commit. The worst were the session
ritual pointing at `docs/phases/phase-NN-*.md` files the lean profile stopped
creating, and the understanding gate still described as firing every phase when A2
moved it to tier boundaries. The amendment procedure now ends with "resync this file
in the same commit".

**③ The sync bug is structural now.** Advancing `current_phase` requires every phase
behind it to be a closed record: status `complete`, a `completion_commit`, and
evidence on every criterion. `skipped` is legal for Tier C/D but needs a reason.
Verified to fire before being trusted.

**④ Standing instruction recorded**: nothing blocks development, including the
author's learning. No mid-session teaching pauses; concepts go in the learning note.
Written into CLAUDE.md as a fourth protocol.

### The schema

Four reversible migrations, 19 tables. The two decisions worth remembering are in
ADR-0025 (one `media_items` table with all eight kinds from day one, because SQLite
cannot alter a `CHECK` constraint) and in `0003_series.up.sql` (episode numbering is
stored per source and resolved by lookup, because the conversions are not arithmetic
and Phase 12's false-confident budget is 1%).

### Measured

```
by_id           p50 0.032ms  p95 0.050ms  p99 0.098ms
by_exact_title  p50 0.081ms  p95 0.128ms  p99 0.179ms
by_external_id  p50 0.056ms  p95 0.091ms  p99 0.120ms
bulk insert     43.7s (11,440 rows/sec) · database 145.4 MB · 500,000 rows
```

Budget is 100 ms per indexed lookup. Worst p99 is 558× inside it.

### Five silent bugs

None of them produced a wrong answer in ordinary use; all five were found by
something that measures or checks rather than by running the app.

1. **`idx_titles_text` was full-scanning.** SQLite only uses an index whose collation
   matches the comparison's. 26.679 ms → 0.081 ms, 330×. **It passed the exit
   criterion either way** — the benchmark is the only reason it was found, and it
   would have surfaced in Phase 5 against a catalogue ten times larger.
2. **`PRIMARY KEY (…, COALESCE(character, ''))`** — expressions are legal in an
   index, illegal in a primary key.
3. **`MIN(CASE e.source …)` in a correlated subquery** reads the outer row; SQLite
   rejects it as "misuse of aggregate".
4. **`open_in` probed writability before creating the directory**, so every fresh
   portable install would have reported "not writable" for a directory that was
   merely absent. Found by the E4 portability test.
5. **The architecture guard rejected the file it protects.** Line-based, so a
   rustfmt-wrapped `pub use foo::{…}` failed on three of its four lines. Now
   statement-based, with that shape as a permanent vector. Selftest is 48 checks.

`--tree` had passed on the re-export module because the file was untracked; `--staged`
caught it at commit time, which is exactly the division of labour the guard documents.

### Blockers

None. **One decision the author owes**, and it should be made before Phase 4:

**P9 — compile-time-checked SQL.** The data layer uses runtime-checked
`sqlx::query()`, not the `query!` macros. `SPEC.md` §2's tech table gives compile-time
checking as the *reason* sqlx was chosen ("valuable for a learner"), so this is a real
deviation and it is being surfaced rather than quietly kept. Honest costs both ways
are in `known_debt` and `docs/DECISIONS_PENDING.md`. It matters now because Phase 4
writes far more SQL than Phase 3 did, and converting after that costs more.

### What the next session should do first

Verify CI green on `phase/03-data-layer`, merge, tag `phase-03`, then set
`phases[3].completion_commit` to the merge SHA and advance `current_phase` to 4.
**Get the P9 ruling before starting Phase 4 work.**

Still outstanding for the author, neither blocking: **P8** (Tier 0 embedding
measurement before Phase 21) and the **TMDB enquiry** at
`docs/correspondence/tmdb-ai-clause.md`.

---

## Session 7 — 2026-09-01 — two rulings, the navigation system, and Phase 4 begins

### Rulings

**P9 closed — runtime-checked SQL, spec amended to 1.5.0** (ADR-0026). The author's
reasoning: if most columns need `as "col!"` nullability annotations then
"compile-time checked" is really "compile-time asserted by me", which is weaker than
it sounds; add the `cargo sqlx prepare` ritual across 24 more phases and dynamic SQL
being impossible anyway, and the macros lose. §2's rationale now says what is true —
single-file portability, WAL, a mature async driver.

Compensating control: `tests/repository_surface.rs` exercises **every** repository
method against a freshly migrated database. Standing requirement in CLAUDE.md.
Verified honestly: typo'd `primary_title` → `primary_titel` and **`cargo build`
reported zero errors** — the demonstration that the compile-time guarantee is
genuinely gone — while the test failed with `no such column`.

**P2 closed — no TMDB key ever ships** (ADR-0027). Per-profile, in settings, under
the user's own acceptance, removable. Every dependent surface degrades to the §9.4
typographic state, which Phase 2 already built. The enquiry is not sent and no longer
tracked; kept and marked closed, because the analysis is the useful part.

### The navigation system (SPEC 1.6.0, ADR-0028)

Phase docs reinstated as **generated** working files; six commands as skills;
`docs/COMMANDS.md`; `tools/statecheck` in CI and pre-push; the decision protocol as a
fifth CLAUDE.md protocol; the session reading list cut to two files.

Verified in the direction that matters: a deliberately vague `next_action` was
**refused by the pre-push hook**.

### Phase 4 opened, subtask 4.1 complete

Migration 0005 puts `ingest_jobs` and `ingest_steps` in the app database (author's
ruling — two files is what would break the copy-the-folder promise). The runner is
built on one idea: **a checkpoint commits in the same transaction as the work it
describes**. Moving it onto a separate connection broke 8 of 10 tests and deadlocked
a ninth, which is the demonstration that the property is load-bearing.

### Six bugs, and what each one cost

1. **`Job::begin` only adopted `running` jobs** — but a crashed job is marked
   `failed`, which is precisely the one to resume. Every crash silently started over:
   70 rows where 50 were expected.
2. **`run_step` upserted a step to `running` before reading its status**, so a
   completed step was marked running by the very query about to ask whether it had
   completed.
3. **Three tests hard-coded schema version 4** and broke on migration 0005. Now
   derived from `Db::latest_schema_version()`.
4. **`statecheck` did not validate the state file's own schema** — a subtask marked
   complete with a null commit reached CI. Pre-push is meant to be the stricter gate.
5. **`statecheck` rewrote the file it was checking.** `read_text`/`write_text`
   translate newlines, so its restore converted `PROGRESS.md` from LF to CRLF on
   every run — and it compared normalised text, so it was blind to its own damage.
   CI saw a 118-line diff on a file whose content had not changed.
6. **Then the fix over-corrected**: comparing bytes is right for the restore and
   wrong for the comparison, because CI checks out with `autocrlf`. Compare
   normalised, restore exact.

(5) and (6) are the useful pair. A check that modifies what it checks is worse than
no check; a check that cries wolf gets deleted. Both were mine, one session apart.

### Also

The **statecheck rule was deliberately weakened** after it fired on real work: a
trailing comment fix in a migration was refused although the work was recorded one
commit earlier. It is now a window — if any of the last five commits changed code,
one of them must have touched `PROJECT_STATE.json`. A rule that demands a meaningless
edit is a rule that earns `--no-verify`.

`--force` regenerating `phase-00` destroyed a hand-written retrospective. Restored
from git; the generator now refuses to overwrite any document it did not produce.

The phase-doc generator found its own bug on first real use: Phase 4's Risks section
came out empty because R4 names Phase 4 as owner while the Phase 4 entry never
mentions R4. Lookup is bidirectional now.

### Blockers

None. No decisions waiting that block Phase 4.

### What the next session should do first

`/next`. Subtask 4.2: IMDb dataset download, verification and normalisation, built as
steps on the runner. **R4 is this phase's named risk** — measure size and time before
committing to a shape, and scope by a popularity threshold rather than ingesting
everything.

Still outstanding for the author, non-blocking: **P8** (Tier 0 embedding measurement
before Phase 21).

---

## Session 8 — 2026-09-04 — Phase 4: AniList ingestion, and five bugs only real data found

**Phase 4, subtasks 4.4 (catalogue half) — 6 of 13 done.** Commits `87f15f6`,
`f80ae81`, `fb034ba`.

### What was built

`ingest anime` — pages the AniList catalogue, runs every entry through the matcher,
and promotes what matches to `anime_film`/`anime_series` with romaji, native and
English titles written as asserted facts plus AniList and MAL external ids. This is
the only place in the pipeline where a title becomes *anime* rather than merely
animated, and it happens because AniList's `format` says so — IMDb cannot make that
distinction at all.

Also `ingest repair-variants`, and `crates/metadata-api`'s `AniList::owned`.

### The five bugs

**1. The matcher counted title rows, not items.** A catalogue entry carries one row
per spelling, and `Death Note`, `DEATH NOTE` and `Death note` all normalise to the
same key — so one unambiguous series arrived as five rivals. Death Note, One Piece,
Naruto and Attack on Titan were every one of them refused as *ambiguous against
themselves*: 97 of 250 outcomes. **No fixture could have caught this**, because a
fixture never has three spellings of one title.

**2. The upsert's `ON CONFLICT` target was copied from a stale comment.** Migration
0001 documents `idx_titles_unique` as `(item, variant, language, region)`; migration
0007 had since redefined it to `(item, variant, title)`. SQLite rejected it outright,
which is the good case — an applied migration is history, not documentation.

**3. `is_resuming` reported a fresh run on every real resume.** It counted only steps
marked `complete`; a crash lands *mid*-step, leaving a cursor on a step that never
completed. A predicate that is true in the rare case and false in the common one is
worse than no predicate.

**4. AniList refuses to paginate past 5,000 entries.** The first full run died there
having reached the top 5,000 anime and no further. `pageInfo.total` reports exactly
`5000` for *any* query including one whose real total is far smaller, so it is a trap
rather than a bound. The sweep is now partitioned by `seasonYear`, verified against
the live endpoint first — `id_greater` would have been the natural keyset cursor and
does not exist.

**5. 41,193 title rows were labelled English while being French or Spanish** — 5.5%
of every `english` title. `akas::variant` read the release region before checking
whether the language was already known, and IMDb files the Spanish *Spirited Away*
under region US, the French under CA. A release region is where a title was used, not
what language it is in.

### Tuning, measured

Same 250 most-popular entries, re-run after each change: **40.4% → 68.4% → 73.6%**
matched; ambiguous **97 → 23 → 9**. The two narrowing rules that did the second half
both use evidence already held rather than preference — a known agreeing year beats no
year at all, and AniList states film-or-series. Neither can create a match or reject
the last candidate standing. Ranking the remaining nine by vote count would resolve
most of them and would be wrong: popularity is evidence about a title, not about its
identity.

### The full sweep, and one idea killed by measuring it

**14,737 AniList entries, 7,328 matched (49.7%), 809 s.** 5,917 `anime_series` and
1,466 `anime_film` now exist in a catalogue that could not previously distinguish anime
from any other animation. `repair-variants` then corrected 41,193 rows in 39 s.

**Long-vowel folding was proposed, measured and rejected.** `Obake no Q-tarou` is in the
catalogue as `Q-Taro the Ghost`, findable under `obake no q taro`, so folding Japanese
long vowels would recover it. It would recover 200 of 5,997 — 3.3% — while collapsing
3,459,678 distinct titles to 3,432,608, manufacturing 27,070 collisions to do it. Bad by
two orders of magnitude, and every collision lands in a matcher built to refuse
ambiguity. The example was real and unrepresentative; the bucket is actually full of TV
specials and promotional shorts IMDb never listed.

**A sixth bug: offset pagination over a popularity ordering.** Three sweeps returned
14,737 / 14,344 / 14,737. Ids do not reorder; popularity ranks do. Now `sort: ID`, which
is also what makes ascending-year plus ascending-id mean the same thing — earlier first
— so a series keeps season one's mapping. Season-aware matches consequently dropped to
**zero**, which is the design working: season-stripping still fires, but its result now
lands in `already_claimed` rather than creating a second mapping.

### What that says about testing

Three of the five were invisible to unit tests and visible within minutes of running
against six million real rows. Fixtures test the logic you thought of. The 250-title
sample became the real harness, and re-running it after every change is what turned
"the matcher seems better" into three numbers.

### Blockers

None.

### What the next session should do first

Finish 4.4's remaining half — episode numbering into `episode_numbering` — then 4.11's
50-title fixture, which is E5's actual evidence and which `data/anilist-unmatched.tsv`
now exists to be chosen from.

Still outstanding for the author, non-blocking: **P8** (Tier 0 embedding measurement
before Phase 21).
