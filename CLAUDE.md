# CLAUDE.md — sin-e-phile

**This file is loaded automatically at the start of every session. `SPEC.md` is not. Read `SPEC.md` before doing anything else.**

---

## What this is

sin-e-phile is a Windows desktop media engine unifying streaming, torrenting, and a local film library behind a semantic search engine and a discovery-first recommender. Tauri v2 + Rust backend + React/TypeScript frontend.

It is a **portfolio project** built by a student who is new to both Rust and React. Code the author cannot explain is a failure condition, not a shortcut.

`SPEC.md` is the complete specification and the constitution of this project. It outranks convenience, it outranks "we could just", and it outranks anything you think you remember from a previous session.

---

## Session start ritual — run this every session, without being asked

1. **Read `SPEC.md`.** At minimum Sections 1–2 (mission, constraints), Section 10 (session protocol), and Section 15's entry for the current phase.
2. Run `tools/doctor` and fix or report any missing prerequisite before anything else.
3. Read `PROJECT_STATE.json`, then `PROGRESS.md`, then the last two entries of `SESSION_LOG.md`, then `docs/phases/phase-NN-<slug>.md` for the current phase. If the phase names risks, read those entries in `docs/RISKS.md`.
4. **Verify state against reality.** Run `git status`, `git log --oneline -10`, `cargo test`, `npm test`. If the repository disagrees with `PROJECT_STATE.json`, **the repository is right.** Correct the state file and record the discrepancy in `SESSION_LOG.md`.
5. Report to the author in five lines or fewer: current phase → what's done → what's next → blockers → anything needing a decision.
6. Raise any blocker with `needs_user: true` *before* planning.
7. Produce a plan. **Wait for approval before implementing.**

**If `last_updated` in `PROJECT_STATE.json` is more than 14 days old**, run the cold-resume ritual in `SPEC.md` §10.11 instead: read five session-log entries, run every eval harness and compare to recorded numbers, check for dependency drift, and give the author a full re-orientation briefing. Assume they have forgotten everything, because they have.

## Session end ritual — never end a session without all of these

1. Tests pass, or failures are recorded as an explicit blocker with an explanation.
2. `cargo fmt`, `cargo clippy -- -D warnings`, and frontend lint are clean.
3. `PROJECT_STATE.json` updated — subtask statuses, exit-criteria evidence, and a `next_action` written as an unambiguous instruction. Not "continue the torrent engine". Instead: "implement `SubtitleAligner::estimate_framerate_scale` in `crates/subtitle-align/src/lib.rs`; approach is in `docs/specs/subtitle-alignment.md` §3".
4. `PROGRESS.md` regenerated from `PROJECT_STATE.json`.
5. `SESSION_LOG.md` entry appended.
6. Docs written for anything built this session.
7. Learning note written or updated (`docs/learning/phase-NN-notes.md`).
8. Everything committed and pushed.

**Update `PROJECT_STATE.json` after every completed subtask, not just at session end.** A session can be interrupted at any moment. State must never be more than one subtask stale.

---

## Three protocols that matter more than any feature

### Evidence, not opinion (`SPEC.md` §10.8)

An exit criterion is met only with an **artefact**: a passing test name, a measured number plus the command that produced it, a file path, a commit SHA, or an explicit `manual: <what the author did and observed>`.

Never "implemented and working", "tested manually", or "looks good". If evidence can't be produced, the criterion is **not met** — say so and explain what's blocking it.

Marking a criterion met without evidence is a correctness bug, because it destroys every future session's ability to trust the state file.

### Three attempts, then stop (`SPEC.md` §10.9)

After three **genuinely distinct** approaches to the same problem fail — not three variations of one idea — stop.

Record it as a blocker with `needs_user: true` including what was tried and what happened. Present the author with honestly-costed options: different library, different approach, defer to a later phase, or cut the requirement. Recommend one, say why, **and wait.**

Do not burn a session grinding. If a blocker will take more than a session, propose working a later independent phase instead — working out of order beats sitting stuck.

### The understanding gate is active (`SPEC.md` §10.10)

At the end of every phase, **ask the author the five self-check questions** from the learning note, in the chat, and wait for real answers.

Struggling on one → re-explain differently and fix that note. Struggling on most → say directly that we went too fast, and propose simplifying the implementation or splitting the phase.

Do not accept "yeah I get it". Ask them to explain it back. The author asked to keep up with everything; honouring that sometimes means slowing down, and saying so is more useful than a green checkmark.

---

## Changing the spec (`SPEC.md` §2.8)

`SPEC.md` is amendable — deliberately, never casually. If you find yourself thinking *"the spec says X but Y is obviously better here"*, that is the moment to **stop and raise it**, not to proceed.

Write an ADR → get the author's explicit approval → edit `SPEC.md` directly (no stale text patched around) → bump `spec_version` → log it in the `## Amendments` section and `SESSION_LOG.md`.

**Never build contrary to the spec intending to update it afterwards.** Amend first, then build.

---

## Standing rules

1. `SPEC.md` outranks memory. Re-read it rather than recalling it.
2. Verify state against the repository, not against the state file.
3. Plan before implementing. Wait for approval.
4. Small commits, conventional messages, always-buildable `main`.
5. **No content source URLs, indexer names, scrapers, or catalogues. Ever. Anywhere** — not in code, config, tests, docs, or commit history. See `SPEC.md` §2.1. This is not negotiable and there is no convenient exception. `tools/guard` enforces this in CI and pre-commit, scanning history as well as the working tree. **Never suppress the guard to make CI pass** — remove the content and rewrite history before pushing.
6. Explain unfamiliar Rust/React/domain concepts as they are introduced, in the phase learning note, against the actual code just written.
7. Never expand a phase's scope silently. Out-of-scope ideas go to `known_debt` or a GitHub issue.
8. When choosing between clever and clear, choose clear.
9. When genuinely uncertain, say so and ask. Do not guess confidently.
10. Measure before optimising. Record the numbers.
11. If a phase's exit criteria cannot be met, say so and explain why. Do not redefine them.
12. Never write more than ~400 lines of new logic without stopping to explain what it does and why.
13. **The test suite and eval harnesses only grow.** Before merging any phase, run everything and compare against the previous phase's recorded numbers. A quality regression in an earlier phase's metric blocks the merge exactly as a failing test does.
14. Never commit a secret. If one is ever pushed, **rotate it first**, then clean history. Assume anything pushed to a public repo is compromised the moment it lands.
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
| Torrent | librqbit, in-process |
| Player | libmpv, embedded, custom UI over it |
| Database | SQLite via `sqlx`, WAL, portable `./data/` |
| Text search | SQLite FTS5 (BM25) |
| Vector search | HNSW, persisted |
| Embeddings | ONNX Runtime (`ort`) + quantised sentence-transformer |
| Media processing | FFmpeg |
| Licence | GPL-3.0, source only |

**Explicit non-choices:** no Electron, no bundled qBittorrent, no cloud LLM in the critical path, no server component of any kind, no Docker, no cross-platform abstractions.

---

## Hard constraints

- **Windows only.** Use Windows APIs directly where they give a better result.
- **Portable by default.** All data in `./data/` next to the executable.
- **Zero-cost operation.** Fully functional with no paid services.
- **No telemetry.** Nothing leaves the machine except explicit user-configured API calls.
- **Performance budgets** (`SPEC.md` §2.3), enforced against Tier 0 hardware: cold start < 4s, search < 80ms p95, play → first frame < 500ms local / < 8s streaming, idle RAM < 250MB, 60fps scroll.
- **Tier gating** goes through `tiers.rs` only. No feature checks hardware directly. Every gated feature degrades to something *good*, never to something broken or empty.

---

## Where things are

```
SPEC.md                  the specification — read this
PROJECT_STATE.json       machine-readable resume state
PROGRESS.md              human-readable progress
SESSION_LOG.md           append-only session history
docs/adr/                architecture decision records
docs/phases/             per-phase spec + retrospective
docs/learning/           the author's explainers — a deliverable, not a nicety
docs/specs/              protocol and algorithm specifications
docs/HOW_IT_WORKS.md     plain-English system explanation
src-tauri/src/           Rust core
  commands/              Tauri IPC surface — thin, no logic here
src/                     React frontend
  design-system/         tokens and primitives
  features/              home, films, tv, watchlist, live, player, search, settings
crates/                  extracted reusable crates
  filename-parser/  subtitle-align/  source-protocol/
tools/ingest/            offline dataset ingestion
tools/eval/              evaluation harnesses
fixtures/                test corpora
```

Business logic never lives in `src-tauri/src/commands/`. Raw SQL never appears outside `src-tauri/src/persistence/`.

---

## Commands

```bash
npm run tauri dev          # run the app in development
npm run tauri build        # production build
cargo test                 # Rust tests
cargo fmt && cargo clippy -- -D warnings
npm test                   # frontend tests
npm run lint
cargo run -p eval -- all   # evaluation harnesses (from the phase they exist)
```

---

## Definition of done for a phase

Every exit criterion met **with evidence** (§10.8) recorded in `PROJECT_STATE.json` · full test suite and all eval harnesses pass with no regression against the previous phase · lints clean · posture guard and secret scan pass · phase retrospective written · learning note written **and the author has answered its five self-check questions** · `HOW_IT_WORKS.md` and `README.md` updated · branch merged and tagged `phase-NN`.

**The understanding gate is a hard gate.** If the author can't explain it back, the phase is not done — rewrite the note or simplify the code.

---

## Where the project can legitimately stop

28 phases is months of solo work, and abandonment is the most likely failure mode of this project — more likely than any technical risk. So the tiers in `SPEC.md` Appendix E are real stopping points, not a nice-to-have:

- **Tier A (Phases 0–8)** — a working vertical slice. Something to show.
- **Tier B (Phases 9–18, 21, 27)** — **this is the project.** Complete, coherent, measured, with case studies. If the author completes Tier B and stops, they have succeeded.
- **Tier C (19, 20, 23)** — depth. Strengthens it meaningfully.
- **Tier D (22, 24, 25, 26)** — breadth. Fully independent; any, all, or none.

Phase 27 (portfolio finalisation) is run **whenever the author decides to stop**, against whatever exists. It is not the last phase — it's the phase you run when you're done. Suggest running it at the end of Tier B regardless, so the project is always presentable.

**A finished Tier B project beats an abandoned Tier D one, always.** If the author seems to be losing momentum, say this out loud rather than starting Phase 24.

---

## If context has been compacted

If you are resuming after compaction and are unsure of the current state: stop, re-read `SPEC.md` §10, `PROJECT_STATE.json`, and the last `SESSION_LOG.md` entry, then run the session start ritual from step 3. Do not continue from a half-remembered plan.
