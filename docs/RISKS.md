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
| **Status** | `spiked` — escalated, see below |

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

### Spike A findings — 2026-08-31, dev machine (Tier 2)

**Status: `spiked`, partially answered, escalated under §10.9 as blocker B2.**

libmpv `20260830-git-e8673660ab`, Tauri 2.11.5, WebView2 151.0.4129.107.

**What works — and it is most of the risk:**

| Question | Answer |
|---|---|
| Does libmpv drive from Rust on Windows? | **Yes.** Loaded dynamically with `libloading`; ~10 FFI declarations, no bindgen, no pkg-config, **and no MSVC import library** — the MinGW `libmpv.dll.a` never has to be converted. |
| Hardware decode? | **Yes — d3d11va**, selected automatically, VO `gpu-next` `d3d11[nv12]`. |
| Render into a child HWND via `wid`? | **Yes.** First frame 970–1023 ms in a bare Win32 host, 1019–1192 ms inside a Tauri window. |
| Survive a mid-playback parent resize? | **Yes.** `time-pos` advanced through it; surface not torn down. |
| Does it work inside a *Tauri* window? | **Yes.** The Tauri top-level HWND accepts a sibling child alongside the `Chrome_WidgetWin_*` webview host. |

**What does not work — and it is the part R1 is actually about:**

**HTML UI cannot be composited *over* the video using child-window z-order.**
Three genuinely distinct attempts:

1. **Tauri `transparent(true)`, video child at `HWND_BOTTOM`.** Video never
   appeared. The webview painted opaque over it.
2. **Plus `ICoreWebView2Controller2::put_DefaultBackgroundColor` = `A=0`.** The call
   succeeded (verified, not assumed). Video still never appeared. **A transparent
   webview composites against what is behind the *window*, not against sibling
   child HWNDs** — which is the root cause, and it is a Windows compositing
   property, not a Tauri bug.
3. **Inverted: video child at `HWND_TOP`, inset 120 px.** Video renders correctly
   and **covers** the HTML wherever they overlap. HTML renders fine outside the
   video rect.

**Conclusion.** The child-window approach gives **UI *beside* video, never UI *over*
video.** That is insufficient for `SPEC.md` §4, which requires custom player chrome
drawn over the video with auto-hide, and for Phase 11's pause overlay, which is the
project's signature screen.

**The render-API-into-a-texture approach — the other approach Spike A mandates —
remains untested.** `render_gl.h` is present in the dev build. Testing it means
creating a GL or D3D11 context, an `mpv_render_context`, and a present path, which is
materially more work than the three attempts above.

Escalated per §10.9 rather than continuing: see blocker **B2** in
`PROJECT_STATE.json` for the costed options.

### Prior-art survey — 2026-08-31 (B2 step ②)

**The core finding is independently confirmed by a shipped project.** `ventic/ventic`
(126★, Nuxt + Tauri + embedded mpv, streams torrents, and — notably — ships with no
sources, the same posture as this project) states it plainly in
`src-tauri/src/player.rs`:

> "The native surface always paints above the webview, so DOM controls can never be
> composited on top of the video."

So this is not a mistake in our spike. It is the known behaviour, reached
independently by someone shipping the same architecture.

**A fifth option exists that was not on the original list: region cutouts.**
ventic's solution is to invert the problem — rather than compositing the UI over the
video, **cut holes in the video window** where the chrome goes, and let the page
underneath show through:

> "Shaping gets the same result the other way round: cut the control-bar rectangles
> *out* of mpv's window and the page underneath shows through the holes — rendering
> and input both, since a window's input region follows its bounding shape. The video
> window itself never changes size for the UI, so nothing rescales when a bar
> appears."

They use the X11 Shape extension. **The Windows equivalent is `SetWindowRgn`**, and
child windows are clipped to their parent's region — so setting a region on the video
child we own clips mpv's own render window inside it. Untested here, but cheap to
test and it builds directly on the configuration that already works (attempt 3).

*Caveat to verify:* the comment references a `player_windows.rs` that **is not
present in the repository** — only the X11 and macOS backends ship. So the Windows
half of this is our inference from a working Linux implementation, not an observed
Windows precedent.

**Its cost is real and specific:** a window region is **binary, not per-pixel alpha**.
Hard-edged holes only. `SPEC.md` §9.3's gradient scrim under the player chrome, and
any blur-behind, cannot survive a region cutout — the chrome becomes an opaque panel
with a hard edge (rounded corners are possible via `CreateRoundRectRgn`; soft edges
are not).

### DirectComposition: the path exists, but is unmerged

**wry PR [#1762]** — "feat(windows): add DirectComposition (visual) hosting for
WebView2" — adds exactly the capability option A needs.

| | |
|---|---|
| State | **Open, mergeable, `blocked`, and has NO REVIEWS** |
| Opened / last updated | 2026-07-07 / 2026-07-22 |
| Author | external contributor, not a Tauri maintainer |
| Size | 1 commit, 5 files, +740/−26 |
| Runtime verification | production use in a Windows PDF app, as a **wry 0.55.1 fork**; not run on Windows by the PR's CI |

**Crucially, it would not require forking Tauri.** The PR ships
`register_composition_visual_target(hwnd, visual)` specifically for this case — in
its own words, "the piece that lets a Tauri app opt a window into composition hosting
today **with no `tauri-runtime-wry` changes**". So the integration is a
`[patch.crates-io]` override of **wry alone**, pinned to a commit — a dependency
override, not a fork we author or maintain.

**But the ongoing burden is real:** the branch lives in a contributor's fork, is
unreviewed after ~6 weeks, and the adjacent request #391 (offscreen rendering) has
been open **since 2021** — so wry has historically not prioritised this area. Pinning
to an unmerged branch means owning the rebase whenever Tauri's wry requirement moves.
Under R8's pin-and-do-not-upgrade policy that is bounded, but it is not zero, and it
lands on a beginner.

Known gaps the PR itself flags: OLE drag-drop untested in composition mode, touch/pen
best-effort, and JS `window.close()` destroys the host window.

### Existing mpv-in-Tauri plugins

`nini22P/tauri-plugin-libmpv` (21★) and `tauri-plugin-mpv` (29★) exist and work by
passing the **Tauri top-level HWND** as `wid`, so mpv creates its own child inside it,
with `"transparent": true` and a transparent CSS background. That is a different
configuration from the one tested in attempts 1–3 (where we created the child
ourselves), and it is worth one cheap test — though ventic's finding suggests the
outcome is the same, since the native surface still ends up above the webview.

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
