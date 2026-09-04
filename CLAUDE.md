# CLAUDE.md — sin-e-phile

**This file is loaded automatically at the start of every session and carries the
standing rules. `SPEC.md` is the constitution, but you do *not* re-read it end to
end — see the session start ritual below.**

*Synced against `SPEC.md` spec_version **1.9.0**. When `SPEC.md` is amended, resync
this file in the same commit; a stale line here is worse than a stale line anywhere
else, because this one loads into every session.*

---

## What this is

sin-e-phile is a Windows desktop media engine unifying streaming, torrenting, and a
local film library behind a semantic search engine and a discovery-first recommender.
Tauri v2 + Rust backend + React/TypeScript frontend.

It is a **portfolio project** built by a student who is new to both Rust and React.
Code the author cannot explain is a failure condition, not a shortcut.

`SPEC.md` is the complete specification and the constitution of this project. It
outranks convenience, it outranks "we could just", and it outranks anything you think
you remember from a previous session.

---

## Session start ritual — run this every session, without being asked

**Read two files: `PROJECT_STATE.json`, and the current phase doc it names
(`docs/phases/phase-NN-<slug>.md`). That is the reading list.** Not `SPEC.md` end to
end, and never a hunt. Read a `SPEC.md` section only when the work actually touches
it — §15's entry for the phase is already what the phase doc is generated from.

1. Read those two.
2. Run `python tools/statecheck/check.py`. If it fails, run `/unstuck` instead of
   planning — there is no point planning against a state you cannot trust.
3. Run `python tools/doctor/doctor.py` and fix or report any missing prerequisite.
4. **Verify state against reality**: `git status`, `git log --oneline -10`,
   `cargo test --workspace`, `npm test`. If the repository disagrees with
   `PROJECT_STATE.json`, **the repository is right.** Correct the state file and
   record the discrepancy in `SESSION_LOG.md`.
5. Report in five lines or fewer: phase → done → next → blockers → decisions waiting.
   Bullets, not tables. Do not restate the spec.
6. Raise any blocker with `needs_user: true` *before* planning.
7. Produce a plan. **Wait for approval before implementing.**

Steps 1–7 are what `/next` does. See `docs/COMMANDS.md` for the six commands.

**If `last_updated` is more than 14 days old**, run the cold-resume ritual in
`SPEC.md` §10.11 instead: the last **five** session-log entries, **every ADR written
since the last completed phase**, every test suite and eval harness compared against
the recorded numbers, `cargo update --dry-run` and `npm outdated` for drift, then a
full re-orientation briefing. Assume they have forgotten everything, because they
have.

## Session end ritual — never end a session without all of these

1. Tests pass, or failures are recorded as an explicit blocker with an explanation.
2. `cargo fmt`, `cargo clippy -- -D warnings`, and frontend lint are clean.
3. `PROJECT_STATE.json` updated — subtask statuses, exit-criteria evidence, and a
   `next_action` written as an unambiguous instruction. Not "continue the torrent
   engine". Instead: "implement `SubtitleAligner::estimate_framerate_scale` in
   `crates/subtitle-align/src/lib.rs`; approach is in
   `docs/specs/subtitle-alignment.md` §3".
4. `PROGRESS.md` regenerated from `PROJECT_STATE.json`.
5. `SESSION_LOG.md` entry appended — including the phase retrospective, if a phase
   ended (ADR-0016 A5).
6. `docs/specs/*.md` written for anything built this session. Prose expansion of
   `HOW_IT_WORKS.md` and `GLOSSARY.md` is deferred to Phase 27; a 2–3 sentence stub
   and one-line terms are enough now.
7. Learning note written or updated (`docs/learning/phase-NN-notes.md`).
8. Everything committed and pushed.

**When a phase completes**, its finished record must be written back into the
`phases` array — status `complete`, a `completion_commit`, and evidence on every
exit criterion — *before* `current_phase` advances. The schema enforces this.

**Update `PROJECT_STATE.json` after every completed subtask, not just at session
end.** A session can be interrupted at any moment. State must never be more than one
subtask stale.

---

## Five protocols that matter more than any feature

### Evidence, not opinion (`SPEC.md` §10.8)

An exit criterion is met only with an **artefact**: a passing test name, a measured
number plus the command that produced it, a file path, a commit SHA, or an explicit
`manual: <what the author did and observed>`.

Never "implemented and working", "tested manually", or "looks good". If evidence
can't be produced, the criterion is **not met** — say so and explain what's blocking
it.

Marking a criterion met without evidence is a correctness bug, because it destroys
every future session's ability to trust the state file.

**A check that has never been seen to fail is not evidence.** When you add an audit,
a guard, or a harness, break the thing it protects and confirm it fails, then restore.

### Three attempts, then stop (`SPEC.md` §10.9)

After three **genuinely distinct** approaches to the same problem fail — not three
variations of one idea — stop.

Record it as a blocker with `needs_user: true` including what was tried and what
happened. Present the author with honestly-costed options: different library,
different approach, defer to a later phase, or cut the requirement. Recommend one,
say why, **and wait.**

Do not burn a session grinding. If a blocker will take more than a session, propose
working a later independent phase instead — working out of order beats sitting stuck.

### The understanding gate fires at tier boundaries, not every phase (`SPEC.md` §10.10)

**Gates: the end of Phase 8, the end of Phase 18, the end of whatever tier the author
stops at, and whenever the author asks.** At those points, ask the five self-check
questions from the relevant learning notes, in the chat, and wait for real answers.

Every phase still *writes* its five questions. They accumulate unasked until a gate,
which is what makes a gate meaningful rather than a formality.

Struggling on one → re-explain differently and fix that note. Struggling on most →
say directly that we went too fast, and propose simplifying the implementation or
splitting the phase. Do not accept "yeah I get it". Ask them to explain it back.

### How to ask the author for a decision

**Never dump the raw problem.** Doing the analysis and handing over the conclusion is
the job; handing over the confusion is not. Every decision put to the author has four
parts:

- **The decision**, in one sentence.
- **Two or three options**, with honest costs — including for the one you prefer.
  "Slightly more work" is not a cost. "An extra `cargo sqlx prepare` after every
  schema change for 24 more phases" is.
- **Your recommendation**, and why, in one line.
- **The default you will take if the answer is "your call".**

That last line matters most: it means the author can answer in two words and you keep
moving.

**Never block on a decision where a reasonable default exists.** Take the default,
record it, and say that you did. Only a decision that is genuinely the author's to
make — one where proceeding either way would be unsafe or would waste real work —
stops the session.

`/decide` lists everything currently open in this shape.

### Nothing blocks development, including the author's learning (ADR-0016 A3)

**Never pause a session to walk the author through a concept.** Write it into the
learning note and keep going. They will read it when they read it.

The only things that stop you are: a tier boundary, a §10.9 escalation, a spec
amendment, and a decision the author owns. Learning is never one of them.

---

## Changing the spec (`SPEC.md` §2.8)

`SPEC.md` is amendable — deliberately, never casually. If you find yourself thinking
*"the spec says X but Y is obviously better here"*, that is the moment to **stop and
raise it**, not to proceed.

Write an ADR → get the author's explicit approval → edit `SPEC.md` directly (no stale
text patched around) → bump `spec_version` → log it in the `## Amendments` section and
`SESSION_LOG.md` → **resync this file in the same commit**.

**Never build contrary to the spec intending to update it afterwards.** Amend first,
then build.

---

## Standing rules

1. `SPEC.md` outranks memory. Re-read the *relevant section* rather than recalling it.
2. Verify state against the repository, not against the state file.
3. Plan before implementing. Wait for approval.
4. Small commits, conventional messages, always-buildable `main`.
5. **No content source URLs, indexer names, scrapers, or catalogues. Ever. Anywhere**
   — and when the qBittorrent backend lands, **it is a transport only**: its search
   plugins, RSS feeds and tracker lists are never read, which would be this same
   violation arriving by the back door (ADR-0029).
   — not in code, config, tests, docs, or commit history. See `SPEC.md` §2.1. This is
   not negotiable and there is no convenient exception. `tools/guard` enforces this in
   CI and pre-commit, scanning history as well as the working tree. **Never suppress
   the guard to make CI pass** — remove the content and rewrite history before pushing.
   All test vectors use **RFC 2606 reserved domains** (ADR-0009). Adding a line to
   `tools/guard/allowlist.txt` requires an ADR (ADR-0010).
6. Explain unfamiliar Rust/React/domain concepts in the phase learning note as a list
   of **concept + `file:line`, no prose** (ADR-0016 A4). Four sections only: what we
   built, why, new concepts, the five questions. No code tour.
7. Never expand a phase's scope silently. Out-of-scope ideas go to `known_debt` or a
   GitHub issue.
8. When choosing between clever and clear, choose clear.
9. When genuinely uncertain, say so and ask. Do not guess confidently.
10. Measure before optimising. Record the numbers **the moment they are produced**,
    in `docs/eval-results.md` — metric, value, date, commit, command. This is the one
    thing ADR-0016 explicitly refuses to defer.
11. If a phase's exit criteria cannot be met, say so and explain why. Do not redefine
    them.
12. Never write more than ~400 lines of new logic without explaining what it does and
    why — **as bullets in the learning note, not as a mid-session pause** (ADR-0016
    A3). The explanation stays mandatory; the interruption does not.
13. **The test suite and eval harnesses only grow.** Before merging any phase, run
    everything and compare against the previous phase's recorded numbers. A quality
    regression in an earlier phase's metric blocks the merge exactly as a failing test
    does.
14. Never commit a secret. If one is ever pushed, **rotate it first**, then clean
    history. Assume anything pushed to a public repo is compromised the moment it
    lands.
15. Dependencies are pinned. Never upgrade one mid-phase for its own sake.

---

## Locked technology decisions

Do not revisit without an ADR explaining what changed.

| Layer | Choice |
|---|---|
| Shell | Tauri v2 |
| Backend | Rust (stable) |
| Frontend | React 18+ / TypeScript strict / Vite / Tailwind |
| State | Zustand (client) + TanStack Query (server) |
| Torrent | librqbit in-process — required, and the **only streaming path**. A user's own qBittorrent optionally as a bulk-download backend, **never bundled** (ADR-0029) |
| Player | libmpv, embedded, custom UI over it |
| Database | SQLite via `sqlx`, WAL, portable `./data/` |
| Text search | SQLite FTS5 (BM25) |
| Vector search | HNSW, persisted |
| Embeddings | ONNX Runtime (`ort`) + quantised sentence-transformer |
| Media processing | FFmpeg |
| Dev tooling | **Python 3.12, stdlib only, permanently** (ADR-0012) |
| Licence | GPL-3.0, source only |

**Explicit non-choices:** no Electron, no bundled qBittorrent, no cloud LLM in the
critical path, no server component of any kind, no Docker, no cross-platform
abstractions.

**IMDb's dataset files sort as TEXT, not as numbers** (ADR-0032): `"tt10001008" <
"tt1000101"`, and the last row of `title.basics` is `tt9916880`, which is not the
largest id. Never assume a dataset's ordering — check it, in four lines, before
building on it.

**The catalogue refreshes in layers** (ADR-0030, `docs/specs/catalogue-freshness.md`).
The IMDb datasets are a snapshot, so: incremental refresh past the highest id already
stored, AniList airing schedules (no key), **search-triggered backfill** when a miss
finds a source anyway, and full re-ingest. Backfill is the one that stops an
out-of-date catalogue being a dead end.

**No free source publishes an absolute episode number** (ADR-0031). AniList publishes
per-entry numbering and an entry is one cour, so its numbers restart each season where
IMDb's do. `absolute_number` is NULL by design, and **must never be derived by
cumulating episode counts across seasons** — that arithmetic is wrong for exactly the
long-running series the feature exists for, and a confidently wrong number is worse
than a NULL. AniList also paginates only to 5,000 entries, so its catalogue is swept
per `seasonYear`.

**TMDB is optional, and no key ever ships** (ADR-0013, ADR-0027). The app is fully
functional on the offline IMDb + MovieLens catalogue with no key. Each user supplies
their **own** key, per profile, in settings, under their own acceptance of TMDB's
terms, removable at any time. Every TMDB-dependent surface degrades to the §9.4
typographic treatment rather than breaking.

---

## Hard constraints

- **Windows only.** Use Windows APIs directly where they give a better result.
- **Portable by default.** All data in `./data/` next to the executable. Installed
  mode using `%APPDATA%` is an opt-in, never the default.
- **Zero-cost operation.** Fully functional with no paid services.
- **No telemetry.** Nothing leaves the machine except explicit user-configured API
  calls.
- **Performance budgets** (`SPEC.md` §2.3), enforced against **Tier 0** hardware:
  cold start < 4s, search < 80ms p95 *including query embedding*, play → first frame
  < 500ms local / < 8s streaming, idle RAM < 250MB, 60fps scroll. Phase 1's < 2s and
  < 200MB are **Tier 2 dev-machine** targets, not these.
- **Tier gating** goes through `tiers.rs` only. No feature checks hardware directly.
  Every gated feature degrades to something *good*, never to something broken or
  empty. **Tier 0 embeds queries but never catalogue documents** (ADR-0015); ONNX
  Runtime is required at runtime on all tiers.

---

## Architecture rules — structural, not remembered

**`cargo test` cannot run inside `src-tauri` at all** on Windows (ADR-0022). Every
test binary dies at load with `STATUS_ENTRYPOINT_NOT_FOUND`. Therefore:

- **All testable logic lives in `crates/`**, which do not depend on Tauri.
  `src-tauri` re-exports them so call sites are unchanged.
- **`src-tauri` is a thin IPC and wiring layer with no logic worth testing**, and
  carries `test = false` on both targets.
- Ask at *every* phase: if logic is going into `src-tauri` that ought to be testable,
  it belongs in a crate instead. "Is this testable?" and "does this belong in
  `src-tauri`?" have the same answer.

**SQL is runtime-checked, and one test is what makes that safe** (ADR-0026). The
`query!` macros are not used: SQLite gives them too little nullability information,
so most columns would need hand-written `as "col!"` assertions, and dynamic SQL
cannot use them at all. Instead,
`crates/persistence/tests/repository_surface.rs` exercises **every** repository
method against a freshly migrated database. **A new repository method without a line
in that file does not pass review** — it is the whole of the protection, and its
absence is invisible.

Consequently: **business logic never lives in `src-tauri/src/commands/`**, and **raw
SQL never appears anywhere under `src-tauri/`** — it lives in `crates/persistence/`,
with `src-tauri/src/persistence/` containing re-exports and nothing else. `tools/guard`
enforces the SQL rule.

---

## Where things are

```
SPEC.md                  the specification — §15 has the current phase
CLAUDE.md                this file — the standing rules
PROJECT_STATE.json       machine-readable resume state (schema-validated)
PROGRESS.md              generated from PROJECT_STATE.json — never edit by hand
SESSION_LOG.md           append-only session history, incl. phase retrospectives
docs/phases/             ONE file per phase — what a session reads to know its job
docs/COMMANDS.md         the six slash commands, and the tools behind them
.claude/skills/          the slash commands themselves
docs/adr/                architecture decision records
docs/learning/           the author's explainers — a deliverable, not a nicety
docs/specs/              protocol and algorithm specifications (build inputs)
docs/design/mockups/     the Phase 2 mockups, kept as they were shown
docs/RISKS.md            risk register with pre-decided responses
docs/DECISIONS_PENDING.md  what is deliberately undecided
docs/PERFORMANCE.md      measured numbers
docs/eval-results.md     every eval/perf number, recorded when produced
docs/MANUAL_TESTS.md     what cannot be automated
docs/ARCHITECTURE.md     structure, incl. the schema ER diagram
docs/HOW_IT_WORKS.md     plain-English system explanation
src-tauri/src/           Rust application — thin
  commands/              Tauri IPC surface — no logic here
  persistence/           re-exports of crates/persistence — nothing else
src/                     React frontend
  design-system/         tokens and primitives
  features/              home, films, tv, watchlist, live, player, search,
                         downloads, settings
crates/                  extracted, independently testable crates
  tiers/  persistence/  filename-parser/  subtitle-align/  source-protocol/
tools/guard/             posture guard + secret scanner
tools/doctor/            prerequisite checks
tools/state/             state build + schema validation
tools/statecheck/        five checks that make being lost impossible
tools/phasedoc/          generates and maintains docs/phases/
tools/contrast/          WCAG AA token audit
tools/uiaudit/           headless-Chrome UI audit (fps, focus, keyboard, motion)
tools/ingest/            offline dataset ingestion
tools/eval/              evaluation harnesses
fixtures/                test corpora
spikes/                  throwaway de-risking experiments
.githooks/               activated via core.hooksPath (ADR-0012)
```

---

## Commands

**Six commands.** `/status` · `/next` · `/finish` · `/closephase` · `/unstuck` ·
`/decide`. Full cheat sheet in `docs/COMMANDS.md`; the skills themselves are in
`.claude/skills/<name>/SKILL.md`.

```bash
python tools/statecheck/check.py         # is the repo resumable by a cold session?
python tools/phasedoc/generate.py --log N  # keep the phase doc current

npm run tauri dev          # run the app in development
npm run tauri build        # production build
cargo test --workspace     # Rust tests (never from inside src-tauri)
cargo fmt && cargo clippy -- -D warnings
npm test                   # frontend tests
npm run lint

npm run audit:contrast     # WCAG AA on every token pair
npm run audit:ui           # 60fps rail, focus rings, keyboard reach, reduced motion

python tools/doctor/doctor.py            # prerequisites
python tools/guard/guard.py --tree       # posture guard (also --staged --history --selftest)
python tools/guard/secretscan.py --tree  # secret scan
python tools/state/validate_state.py --check      # schema + evidence rules
python tools/state/validate_state.py --progress   # regenerate PROGRESS.md

cargo run -p eval -- all   # evaluation harnesses (from the phase they exist)
```

---

## Definition of done for a phase

Every exit criterion met **with evidence** (§10.8) recorded in `PROJECT_STATE.json` ·
full test suite and all eval harnesses pass with no regression against the previous
phase · lints clean · posture guard and secret scan pass · learning note written with
its five questions · `HOW_IT_WORKS.md` and `README.md` updated · retrospective folded
into the `SESSION_LOG.md` entry · the phase record written back into the `phases`
array with a `completion_commit` · branch merged and tagged `phase-NN`.

**The five questions are written, not asked** — unless this phase ends a tier, in
which case the understanding gate fires (§10.10).

---

## Where the project can legitimately stop

28 phases is months of solo work, and abandonment is the most likely failure mode of
this project (R7) — more likely than any technical risk. So the tiers in `SPEC.md`
Appendix E are real stopping points, not a nice-to-have:

- **Tier A (Phases 0–8)** — a working vertical slice. Something to show.
- **Tier B (Phases 9–18, then 21, then 27)** — **this is the project.** Complete,
  coherent, measured, with case studies. If the author completes Tier B and stops,
  they have succeeded. Phase 21 depends on 18, with 20 optional (amendment 11).
- **Tier C (19, 20, 23)** — depth. Strengthens it meaningfully.
- **Tier D (22, 24, 25, 26)** — breadth. Fully independent; any, all, or none.

Phase 27 (portfolio finalisation) is run **whenever the author decides to stop**,
against whatever exists. It is not the last phase — it's the phase you run when
you're done. Suggest running it at the end of Tier B regardless, so the project is
always presentable.

**A finished Tier B project beats an abandoned Tier D one, always.** If the author
seems to be losing momentum, say this out loud rather than starting Phase 24.

---

## If context has been compacted

If you are resuming after compaction and are unsure of the current state: stop,
re-read `SPEC.md` §10.2, `PROJECT_STATE.json`, and the last `SESSION_LOG.md` entry,
then run the session start ritual from step 3. Do not continue from a half-remembered
plan.
