# Risk register

`SPEC.md` Appendix D, made operational. Reviewed at the start of any phase whose
risks it names.

**Every risk has a trigger** — the observable condition that means the fallback
should be taken. Triggers are decided *in advance*, on purpose, so the decision is
not made at 2am while frustrated. If a trigger fires, that is not a failure; it is
the plan working.

**Status legend:** `open` (live, unmitigated) · `spiked` (measured, awaiting
decision) · `retired` (mitigated or no longer applicable) · `fired` (trigger hit,
fallback taken — the ADR records what happened).

---

## R1 — libmpv cannot be cleanly embedded in a Tauri v2 window

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | **Severe** — Phase 8 and everything downstream |
| **Owner** | Phase 1 (Spike A), Phase 8 |
| **Status** | `open` |

Rendering a native video surface with a webview UI drawn over it is the fiddliest
integration in this project. Everything from Phase 8 onward assumes it works.

**Mitigation.** Spike A in Phase 1, before anything depends on it. Try **both**
approaches — render-API-into-a-texture, and child-window-overlay — and record what
each costs, not merely whether it works.

**Trigger.** Neither approach produces a video frame rendered inside the Tauri
window under Rust control within the two-hour timebox, **or** an approach works but
cannot draw HTML UI over the video without flicker or z-order failure.

**Fallback**, in order, each an ADR:
1. Child window positioned and clipped under the webview.
2. libVLC, which has friendlier bindings.
3. HTML5 `<video>` with FFmpeg remuxing, accepting the codec limitations.

---

## R2 — librqbit's streaming control is insufficient for the Phase 7 scheduler

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | **Severe** — the "8 seconds to first frame" promise |
| **Owner** | Phase 1 (Spike B), Phase 7 |
| **Status** | `open` |

**Mitigation.** Spike B measures real numbers against a legal, well-seeded torrent
and audits whether the API exposes enough control to build a deadline scheduler.

**Trigger.** Either of:
- The API does not permit per-piece or per-range priority to be set and changed at
  runtime — without this the Phase 7 scheduler cannot be written at all.
- Time-to-first-usable-bytes on a healthy swarm exceeds **20 s** with sequential
  priority enabled. (The budget is 8 s; 20 s is the point at which the gap is
  architectural rather than a matter of tuning.)

**Fallback.** libtorrent-rasterbar via FFI. Costs a C++ toolchain and binding work,
roughly a week, but it is the proven path that every serious client uses.

---

## R3 — ONNX Runtime is painful to build on Windows, or too slow on Tier 0

|  |  |
|---|---|
| **Likelihood** | Low–Medium |
| **Impact** | **Moderate/Severe** — raised from Moderate by ADR-0015 |
| **Owner** | Phase 1 (Spike C), Phase 5 |
| **Status** | `open` |

**Impact was raised during the Phase 0 audit.** ADR-0015 established that query
embedding runs on *all* tiers, so an unusable `ort` breaks semantic search
everywhere, not merely on Tier 0. **Spike C therefore gates Phase 5** and should be
run early in Phase 1, not last.

**Mitigation.** Spike C measures **query-embedding latency specifically** — model
already loaded, a single ~30-token query, wall clock to a returned vector, p50 and
p95 — on the dev machine and on a constrained VM approximating Tier 0. Not document
throughput; not an amortised average.

**Triggers**, any one:
- `ort` cannot be built or loaded on Windows without a toolchain beyond what
  `docs/SETUP.md` already requires.
- **Query-embedding p95 exceeds ~30 ms.** That is a third of the §2.3 80 ms search
  budget, leaving too little for ANN search, BM25, fusion and render. Escalate under
  §10.9 — **do not widen the budget.**
- Resident model memory threatens the §2.3 250 MB Tier 0 idle RAM budget.

**Fallbacks.** In order: a smaller or more aggressively quantised model; caching
query embeddings; debouncing so not every keystroke embeds; FTS5-first with semantic
results arriving progressively; lazy model load on first search (also helps the
< 4 s Tier 0 cold-start budget). Last resort, ADR-0014's path: precomputed document
embeddings plus keyword-only search on Tier 0.

---

## R4 — Catalogue ingestion is far larger or slower than expected

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | Moderate — first-run experience |
| **Owner** | Phase 4 |
| **Status** | `open` |

**Partially mitigated already.** ADR-0013 made the offline IMDb + MovieLens
catalogue the base rather than an optimisation, so the critical path no longer
depends on a third-party API's continued free tier.

**Mitigation.** Measure in Phase 4 before committing to a shape. Scope by a
vote/popularity threshold rather than ingesting everything.

**Triggers**, any one:
- Full ingestion exceeds **2 hours** on the dev machine.
- The resulting database exceeds **4 GB**.
- The embedding artefact exceeds the ADR-0014 budget of ~77 MB per 200k titles by
  more than 50%.

**Fallback.** Tiered catalogue: ship a core index of ~200k well-known titles, fetch
the long tail live on demand and cache. The "vastest library" promise is met by the
source layer, not by the local index.

---

## R5 — Windows shell integration hits platform limits

|  |  |
|---|---|
| **Likelihood** | **High** |
| **Impact** | Low — these are enhancements |
| **Owner** | Phase 20 |
| **Status** | `open` |

Windows 11 context-menu entries require a packaged (sparse MSIX) app. Icon overlay
handlers are limited to roughly 15 slots system-wide, and cloud-storage apps have
already taken them.

**Mitigation.** Research the constraints in Phase 20 *before* implementing.

**Trigger.** Enumerating `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers`
on the dev machine shows **no free slot** ahead of sin-e-phile alphabetically.

**Fallback.** Ship the legacy context menu (still reachable via "Show more
options"), or produce a sparse package purely for shell registration. Drop icon
overlays and document why — knowing *why* a platform limit exists is itself a good
interview answer.

---

## R6 — Subtitle alignment does not reach 90% on the fixture corpus

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | Low–Moderate |
| **Owner** | Phase 10 |
| **Status** | `open` |

**Mitigation.** Build the corpus **first**, in Phase 10, so the target is measured
rather than assumed.

**Trigger.** After the aligner is complete and tuned, corpus accuracy within 150 ms
sits below **90%** and two distinct improvement attempts have not closed the gap.

**Fallback.** Lower the target *with evidence*, and lean harder on the
embedded-track path — which is already the primary route and needs no alignment at
all. Report the honest number in the case study: **a measured 76% with an
explanation beats an unverified claim of 95%.**

---

## R7 — The project is abandoned partway

|  |  |
|---|---|
| **Likelihood** | **High** |
| **Impact** | **Severe** |
| **Owner** | Every session |
| **Status** | `open` — permanently |

**This is the most likely failure mode of the entire project, and it is not
technical.** 28 phases is months of solo work.

**Mitigation.** The tier structure in `SPEC.md` Appendix E. Every tier boundary is a
legitimate stopping point that still produces a portfolio project. Phase 8 is
demoable. Phase 18 is a complete product. Phase 27 is run **whenever you decide to
stop**, against whatever exists.

**Triggers**, any one:
- **More than 30 days** since the last commit.
- Three consecutive sessions that end without a subtask completed.
- The author says, in any words, that they are losing interest or falling behind.

**Fallback.** Stop at a tier boundary, run Phase 27's portfolio work against what
exists, and ship it. **A finished Tier B project beats an abandoned Tier D one,
always.** If momentum is visibly going, say so out loud rather than starting
Phase 24.

---

## R8 — Dependency drift over a long project

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | Low–Moderate |
| **Owner** | Every session; cold resume especially |
| **Status** | `open` |

**Mitigation.** Lockfiles committed. `cargo audit` / `npm audit` in CI. The cold
resume ritual (§10.11) checks for drift after a gap. Dependencies pinned; never
upgraded mid-phase for their own sake.

**Triggers**, any one:
- `cargo audit` or `npm audit` reports a vulnerability with no patched version.
- A locked dependency's repository has had **no commit in 12 months** and an issue
  affecting this project is open.
- A pinned crate fails to build on a current stable Rust.

**Fallback.** Pin the working version and defer upgrading. If a crate is genuinely
abandoned, vendor it or replace it — with an ADR.

---

## R9 — The author falls behind and stops understanding the code

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | **Severe** — the project fails at its actual purpose even if the app works |
| **Owner** | Every phase |
| **Status** | `open` |

**Mitigation.** The active understanding gate (§10.10). Learning notes as a hard
deliverable. The ~400-line explanation rule (§2.2).

**Triggers**, any one:
- The author cannot answer **two or more** of a phase's five self-check questions
  without prompting.
- The author answers "yeah I get it" without being able to explain it back.
- A learning note is skipped or deferred "to next session" more than once.

**Fallback.** **Stop feature work entirely.** Spend a session or more on a code tour
and a rewrite of the weak learning notes. Simplify implementations that cannot be
explained. This is always worth doing over adding a feature.

---

## R10 — The §2.1 posture erodes

|  |  |
|---|---|
| **Likelihood** | Medium |
| **Impact** | **Severe** — takedown, and the portfolio piece disappears |
| **Owner** | Every session |
| **Status** | `open` — mitigated from Phase 0, never retired |

A source URL appears in a fixture, a doc example, or a "temporary" default, across
dozens of sessions.

**Mitigation.** The automated posture guard (`tools/guard/`), scanning working tree
*and* history, in CI and pre-commit, **verified in Phase 0** — see `SESSION_LOG.md`
entry 1 for the planted-string evidence. Plus ADR-0011's fixture redaction policy,
which removes the Phase 12 collision before it can happen.

**Triggers**, any one:
- The guard fires on a real (non-planted) violation.
- Anyone proposes suppressing the guard, adding a `# guard:ignore`, or exempting a
  directory.
- **`tools/guard/allowlist.txt` gains a line without an accompanying ADR.** This is
  the likeliest erosion path by a wide margin, because adding a line is easy and
  always feels justified in the moment.
- The structural exemption list in `guard.py` grows beyond the four pinned paths.

**Fallback.** Remove, rewrite history before pushing, and strengthen the denylist.
**Never suppress the guard.** Phase 27's §2.1 re-verification must read
`allowlist.txt` line by line, not merely confirm the guard passes.

---

## R11 — Third-party API terms change under the project *(new, Phase 0)*

|  |  |
|---|---|
| **Likelihood** | Medium–High |
| **Impact** | Low–Moderate |
| **Owner** | Phase 4, Phase 19 |
| **Status** | `open` |

Added during the Phase 0 live verification of `SPEC.md` §14, which turned up three
constraints the spec did not record:

- **TMDB forbids caching data for longer than 6 months** on the non-commercial tier,
  which directly constrains Phase 4's cache TTL design.
- **TMDB prohibits use of its data for AI/ML training.** Computing embeddings is
  inference rather than training, but the distinction deserves an explicit decision
  rather than an assumption — logged as **P2** in `DECISIONS_PENDING.md`.
- **Trakt tightened its free tier** and is actively revising limits for 2026,
  including limiting free users' connected applications.
- **MovieLens states it does not generally permit public redistribution**, which
  bears on shipping a derived item-item matrix — logged as **P3**.

**Mitigation.** `docs/SETUP.md` records the verification date beside every service.
Every integration degrades gracefully to the offline catalogue (ADR-0013).

**Trigger.** A service's terms change such that free personal use is no longer
sufficient, **or** a verified term in `SETUP.md` is more than 12 months old at the
start of a phase that depends on it.

**Fallback.** Drop the service to optional-and-absent, and confirm the app is still
good without it — which ADR-0013 already requires of TMDB, the most load-bearing of
them.
