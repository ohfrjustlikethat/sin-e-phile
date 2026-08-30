# Progress

> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by
> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two
> can never disagree). Edit the state file, then regenerate.

**Spec version 1.2.0** · 3 session(s) completed · last updated 2026-08-31

---

## Where we are right now

**Phase 1 — Application Shell and Capability Tiers** (`blocked`, branch `phase/01-application-shell`)

0 of 7 exit criteria met with evidence.

> Spikes A, B then C run FIRST, before any other Phase 1 work, in that order. Each is throwaway code in spikes/, timeboxed to ~2 hours, with findings written to docs/RISKS.md and numbers to docs/eval-results.md. A failed spike escalates under SPEC.md 10.9 and stops.

### Subtasks — 0/10 complete

- [!] **1.1** Spike A - libmpv in a Tauri v2 window (R1). Child-window approach DONE and mostly successful; HTML-over-video compositing FAILS after 3 distinct attempts. Render-API-into-texture approach UNTESTED. Escalated as B2.
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

Start Phase 1 on branch phase/01-application-shell. Run the three de-risking spikes FIRST, in order A then B then C, before any other Phase 1 work - SPEC.md 15 is explicit that discovering a failure in Phase 8 is catastrophic while discovering it now costs a day. Spike A: prove libmpv can render video inside a Tauri v2 window under Rust control with HTML UI drawn over it; try BOTH the render-API-into-a-texture and child-window-overlay approaches and record what each costs. Throwaway code in spikes/spike-a-libmpv/. Timebox ~2 hours. Write findings to docs/RISKS.md under R1 and any numbers to docs/eval-results.md. If the trigger in R1 fires (no frame rendered in the timebox, or UI cannot be composited without flicker or z-order failure), escalate under 10.9 and STOP - do not proceed to the rest of Phase 1 hoping it works out.

---

## Blockers

- **B2** **(needs you)** Spike A, risk R1. libmpv embeds in a Tauri v2 window and hardware-decodes correctly (d3d11va), first frame ~1.0-1.2 s, survives resize. But HTML UI CANNOT be composited OVER the video using child-window z-order, which SPEC.md 4 requires for the player chrome and Phase 11 requires for the pause overlay. Root cause: a transparent WebView2 composites against what is behind the WINDOW, not against sibling child HWNDs - a Windows compositing property, not a Tauri bug. The render-API-into-a-texture approach, which Spike A also mandates, is untested and is materially more work. Needs a decision on which path to spend time on before Phase 1 continues. Options and a recommendation are in the session report and docs/RISKS.md under R1. UPDATE 2026-08-31: option 1 (still-frame substitution) WORKS - pause to overlay visible measured at 161/95/140 ms across three cycles, all within the 200 ms budget, on a DEBUG build. So Phase 11's pause overlay is NOT blocked, and the problem shrinks to chrome-over-PLAYING-video only. DECISION DEADLINE: before Phase 8. Phases 2-7 are independent of compositing and proceed regardless; only 8 and 11 depend on it, and 11 is now substantially unblocked.

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

---

## Decisions pending

- **P1** — decide by Phase 27 Source-only distribution versus Phase 27 packaging and the 2.3 installed-size budget. See docs/DECISIONS_PENDING.md.
- **P5** — decide by Phase 12 Where the Phase 12 review-queue confidence threshold sits, given >95% top-1 and <1% false-confident pull against each other. Measure, do not guess. See docs/DECISIONS_PENDING.md.
- **P6** — decide by Phase 27 Windows Sandbox pass on a genuinely bare machine. The E1 CI job proves a clean checkout builds, but windows-latest ships Rust, Node and MSVC preinstalled, so it does not prove SETUP.md is complete from nothing.
- **P7** — decide by Phase 8 B2 / R1: how to composite player chrome over PLAYING video. Phase 11's pause overlay is solved by still-frame substitution (measured, within budget). Ranked fallbacks per the author: 1) DirectComposition visual hosting if tractable without forking wry; 2) still-frame plus a reserved chrome strip during playback; 3) layered TOP-LEVEL window, DWM-composited, which is a different mechanism from child-HWND z-order and is untried; 4) HTML5 plus remux, last. Chrome-beside-video is explicitly NOT on the list - amend SPEC.md 4 deliberately rather than back into it. DEADLINE: before Phase 8.
