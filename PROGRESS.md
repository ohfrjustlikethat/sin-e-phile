# Progress

> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by
> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two
> can never disagree). Edit the state file, then regenerate.

**Spec version 1.3.0** · 3 session(s) completed · last updated 2026-08-31

---

## Where we are right now

**Phase 1 — Application Shell and Capability Tiers** (`in_progress`, branch `phase/01-application-shell`)

0 of 7 exit criteria met with evidence.

> Spike A COMPLETE and R1 retired: libmpv embeds in Tauri v2, hardware-decodes, and the compositing problem is solved by still-frame substitution on pause plus region cutouts during playback (ADR-0021), with no patched dependency. SPEC.md 9.3 amended by ADR-0020. Spikes B and C are next, before any other Phase 1 work.

### Subtasks — 1/10 complete

- [x] **1.1** Spike A - libmpv in a Tauri v2 window (R1). DONE: embeds, d3d11va hardware decode, survives resize. Compositing solved by still-frame on pause + SetWindowRgn cutouts during playback. Both approaches the spec mandates were evaluated; DirectComposition (wry #1762) rejected as unmerged and retained as an upgrade path. · `28eb3d6`
- [ ] **1.2** Spike B - librqbit sequential streaming (R2). Measure time-to-first-usable-bytes against a legal well-seeded torrent; audit runtime per-piece priority control
- [ ] **1.3** Spike C - ort/ONNX on Windows (R3). Measure QUERY-embedding p95 specifically; escalate above ~30ms
- [ ] **1.4** Tauri v2 + React + TS strict + Vite + Tailwind building
- [ ] **1.5** Window: custom title bar, remembered size/position, min 1024x640
- [ ] **1.6** Left nav rail, five destinations, collapse/expand
- [ ] **1.7** Typed IPC with generated TS types - changing a Rust signature must break the TS build
- [ ] **1.8** tiers.rs - detect RAM, cores, GPU/hw-decode; classify Tier 0/1/2; persist with manual override
- [ ] **1.9** Settings screen showing detected hardware and what the tier enables, in plain language
- [ ] **1.10** tracing to rotating file in data/logs/; error boundary; Rust panic handler writing a crash report

### Exit criteria

- [ ] **E1** All three spikes completed, with findings and measurements recorded in `docs/RISKS.md`.
- [ ] **E2** Any spike that failed has an ADR recording the fallback decision and the author's approval.
- [ ] **E3** App launches to interactive in < 2 s on the dev machine (Tier 2 target; the governing budget is §2.3's < 4 s on Tier 0).
- [ ] **E4** IPC types are generated, not hand-written; changing a Rust command signature breaks the TypeScript build.
- [ ] **E5** Tier detection is correct on the dev machine and on a deliberately-constrained run (simulate Tier 0 via override).
- [ ] **E6** Idle RAM < 200 MB on the dev machine (Tier 2 target; the governing budget is §2.3's < 250 MB on Tier 0).
- [ ] **E7** A deliberately-triggered panic writes a crash log and shows a graceful error screen.

---

## What's next

Spike B, then Spike C, before any other Phase 1 work. Spike B (librqbit, risk R2): measure time-to-first-usable-bytes with sequential priority against a legal well-seeded torrent - an Internet Archive item or a Linux ISO - and AUDIT whether the API exposes runtime per-piece or per-range priority control at all, since without that the Phase 7 deadline scheduler cannot be written. Pass the torrent URL as a CLI argument; NEVER commit one, the guard blocks .torrent URLs and bare infohashes by design. R2 triggers: no runtime piece-priority control, or TTFB above 20 s. Then Spike C (ort, risk R3): measure QUERY-embedding p95 specifically - model loaded, one ~30-token query, wall clock to a returned vector - and escalate above ~30 ms rather than widening the 80 ms search budget. Numbers to docs/eval-results.md as produced. Then Phase 1 subtasks 1.4-1.10.

---

## Blockers

None.

---

## All 28 phases

Tiers are the legitimate stopping points from `SPEC.md` Appendix E. **Tier B is the definition of done** — complete it and the project has succeeded.

| | # | Phase | Tier | Depends on | Sessions | Criteria met |
|---|---|---|---|---|---|---|
| [x] | 0 | Bootstrap and Project Infrastructure | A | nothing | 1 | 8/8 |
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
- **D5** (raised in Phase 1) Player has TWO compositing paths - still-frame when paused, region cutouts when playing - and the chrome silhouette must stay in sync with the region or the uncovered part of the hole shows the desktop. Phase 11 owns this invariant. Retire if wry PR #1762 merges and DirectComposition replaces both.

---

## Decisions pending

- **P1** — decide by Phase 27 Source-only distribution versus Phase 27 packaging and the 2.3 installed-size budget. See docs/DECISIONS_PENDING.md.
- **P5** — decide by Phase 12 Where the Phase 12 review-queue confidence threshold sits, given >95% top-1 and <1% false-confident pull against each other. Measure, do not guess. See docs/DECISIONS_PENDING.md.
- **P6** — decide by Phase 27 Windows Sandbox pass on a genuinely bare machine. The E1 CI job proves a clean checkout builds, but windows-latest ships Rust, Node and MSVC preinstalled, so it does not prove SETUP.md is complete from nothing.
