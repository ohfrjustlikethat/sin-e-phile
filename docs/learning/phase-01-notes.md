# Phase 1 — Learning notes

**Application shell and capability tiers.** In progress; Spike A complete.

> **This note deliberately breaks the lean profile** (ADR-0016). The author asked
> for Spike A written up in full — not for learning, but because
> *"hit a platform constraint, found it independently confirmed by another project,
> adopted their inversion of the problem"* is the strongest engineering story this
> project has produced, and it should not be reconstructed from memory in six months
> for the Phase 27 case study.
>
> The rest of Phase 1 reverts to the four lean sections.

---

## Spike A — the compositing wall, and the way round it

### What the spike was for

`SPEC.md` marks three locked technology decisions as bets and requires each to be
de-risked in Phase 1, before anything depends on them. **R1** is the worst of them:
libmpv embedded in a Tauri v2 window, rated *Medium likelihood, Severe impact*,
because Phase 8 and everything downstream sits on it.

The spec's framing is worth keeping: discovering this fails in Phase 8, after five
phases have been built on top, is catastrophic. Discovering it in Phase 1 costs a
day.

It cost a day. That was a good trade.

### What worked immediately

Three of the four questions answered themselves quickly, and all three were
genuinely good news.

**libmpv drives from Rust with no toolchain pain.** The mpv dev build ships
`libmpv.dll.a`, a MinGW import library that MSVC cannot link — the "proper" fix is
`dumpbin /exports` → `.def` → `lib /def:` to synthesise an MSVC `.lib`. Loading the
DLL dynamically with `libloading` sidesteps all of it: about ten FFI declarations, no
bindgen, no pkg-config, no build script. R3's toolchain worry does not apply to R1.

**Hardware decode works out of the box** — `d3d11va`, selected automatically, video
output `gpu-next` on `d3d11[nv12]`. That is a Phase 8 exit criterion already
half-evidenced.

**Video renders into a child window we own** and survives a mid-playback parent
resize, with `time-pos` advancing through it. First frame around a second, in both a
bare Win32 host and inside a real Tauri window.

### The wall

The fourth question is the one R1 is actually about, and it failed.

`SPEC.md` §4 requires player chrome **drawn over the video**, auto-hiding after 2.5 s
idle. Phase 11's pause overlay — the signature screen — requires a full-window panel
over a dimmed, blurred frame. Both need HTML composited **on top of** video.

Three genuinely distinct attempts, which is what §10.9 requires before stopping:

1. **Tauri `transparent(true)`, video child at `HWND_BOTTOM`.** Video never
   appeared; the webview painted opaque over it.
2. **Plus `ICoreWebView2Controller2::put_DefaultBackgroundColor` with alpha 0.** The
   call succeeded — verified, not assumed — and the video still never appeared.
   That is what identified the cause.
3. **Inverted: video child at `HWND_TOP`.** Video rendered correctly and **covered**
   the HTML wherever they overlapped.

**The cause, and it is not a bug:** a transparent WebView2 composites against what is
behind the **window** — the desktop — not against sibling child HWNDs inside the same
window. A native child HWND is not part of the webview's compositing tree. There is
no z-order arrangement that fixes this, which is why three attempts converged rather
than diverging.

### Stopping, and why it mattered

§10.9 says: after three genuinely distinct attempts at the same problem, stop, record
what was tried, present honestly-costed options, recommend one, and wait.

Stopping was correct and the escalation changed the outcome twice.

**First**, the author pushed back on the recommended ordering. My ranking put
"chrome beside video" second; theirs ranked by what each option costs the *product*,
and deleted mine entirely — *"if we get there, I'd rather amend §4 deliberately than
back into it."* That is a better instinct than mine was.

**Second**, they proposed something I had not: **still-frame substitution.** On
pause, capture the frame, hide the video window, and show the capture as an image in
the page. Playback is stopped, so nothing behind the overlay needs to be live — and
once the frame is an `<img>`, everything is HTML and normal compositing applies.

It works, and comfortably. **161 / 95 / 140 ms** pause-to-overlay-visible across
three cycles, against §11's 200 ms budget, on an unoptimised build.

The load-bearing detail is one word: `screenshot-raw **window**` rather than
`screenshot-raw video`. Window capture is at *window* resolution, so a 4K source in a
1280×800 window still yields a 1280×800 image. The cost does not scale with source
resolution. Capturing at source resolution would have blown the budget on exactly the
content where the overlay matters most.

That single insight shrank R1 from *"HTML over video"* to *"chrome over **playing**
video"* — a much smaller problem.

### The prior art, which was worth more than another spike

Before spending a session on DirectComposition, the author asked for thirty minutes
of prior art. That was the highest-value thirty minutes of the phase.

**`ventic/ventic`** — 126★, Nuxt + Tauri + embedded mpv, streams torrents, and ships
with no sources. Nearly this project's premise, including the legal posture. Its
`player.rs` says:

> "The native surface always paints above the webview, so DOM controls can never be
> composited on top of the video."

Independent confirmation, from someone shipping the same architecture, that the wall
is real and not a mistake in our spike. **That is worth more than another spike**,
because a spike can only tell you *you* failed; this tells you the constraint is
inherent.

And they solved it, by **inverting the problem**:

> "cut the control-bar rectangles *out* of mpv's window and the page underneath shows
> through the holes — rendering and input both, since a window's input region follows
> its bounding shape."

They use the X11 Shape extension. The Windows equivalent is **`SetWindowRgn`**.

**Why the inversion is the insight.** Every attempt so far tried to put the UI in
front of the video. The answer is to take the video *away* where the UI needs to be.
Same pixels on screen; completely different mechanism; and it works within what the
platform actually offers rather than against it.

### Proving it

The author named four places it would break, and was right to insist on each:

**1. Hit-testing through the hole.** *"You asserted input follows the shape — that's
true for the shaped window itself, but the click still has to REACH the webview
behind it. Test it, don't assume."*

Correct to challenge. My first two attempts to test it were polluted by other windows
on the desktop — `WindowFromPoint` asks "what is on screen at this pixel", which
depends on every other application. The right probe is
**`RealChildWindowFromPoint`**, which asks *within this window, which child owns this
point* — immune to anything in front.

```
INSIDE  hole  -> WRY_WEBVIEW
OUTSIDE hole  -> SpikeVideoHost
seam, 1px above the edge -> SpikeVideoHost
seam, 1px below the edge -> WRY_WEBVIEW
```

Pixel-exact, and structural rather than inferred.

**2. Region plus resize.** Holes track correctly through mid-playback resizes.

**3. Flicker — the one that would have killed it.** Chrome auto-hides after 2.5 s
idle, so the region changes on every show and hide. A repaint flash would make the
player strobe whenever the mouse moves.

**No flicker.** 758 samples at ~7 ms intervals across 12 toggles; zero frames lost
colour. `SetWindowRgn` costs a median of 0.467 ms.

But that result is *created*, not free. Two defences are both required:

- a **NULL class background brush** and swallowing `WM_ERASEBKGND`, so Windows never
  paints the newly-exposed area;
- **`SetWindowRgn(redraw=false)`** with a targeted invalidate.

Without them, Windows erases the exposed region with the class brush before mpv
repaints — which is precisely the strobe. Had the spike used the default window
class, it would have flickered, and the honest conclusion would have been "cutouts
don't work". **The result depended on knowing why it would flicker before measuring
whether it did.**

**4. Seam quality.** Razor sharp — video ends exactly at the panel edge, no bleed, no
tearing, no fringe.

But the capture showed a defect worth more than the passes: a pale wedge at the
panel's **rounded corners**. The hole is a rectangle; the panel is rounded; and
because the window is transparent, the uncovered corner shows **the desktop**.

Generalised: **the hole and the opaque chrome must be the same shape.** Any part of a
hole the chrome does not paint is a window straight through the application. Fixable
with `CreateRoundRectRgn`, and now recorded in ADR-0020 as a rule rather than as a
bug someone rediscovers in Phase 11.

### Why DirectComposition lost to an unmerged PR

wry **PR #1762** adds `CreateCoreWebView2CompositionController` hosting — exactly the
capability that would restore full per-pixel alpha and the original §9.3 design.

It is also better than it first looked: it ships
`register_composition_visual_target(hwnd, visual)` specifically so an embedder can
opt in **with no `tauri-runtime-wry` changes**. Integration would be a
`[patch.crates-io]` override of wry alone — a pinned dependency, not a fork we author
and maintain.

It still lost:

- **Open, mergeable, and with no reviews** after six weeks.
- From an external contributor, verified in production **as a fork**.
- The adjacent request (#391, offscreen rendering) has been open **since 2021** — so
  this is not an area wry has historically prioritised.

Betting a phase on an unreviewed PR merging is a bet on someone else's schedule. The
cutout path needs nothing but stock Tauri and stock wry, and it is measured. #1762
becomes the *upgrade* if it lands, not the thing the project waits on.

### What the phase actually cost, and what it bought

One day. It bought: R1 closed; two Phase 8 exit criteria half-evidenced; a player
architecture with measured numbers behind every claim; and a design constraint
(ADR-0020) discovered now rather than in Phase 11 when the pause overlay is being
built.

The three things worth carrying forward:

1. **Three distinct attempts, then stop.** Grinding a fourth variation would have
   found nothing — the three failures had one cause, and more attempts at the same
   mechanism could not have revealed it.
2. **Prior art beats another spike.** Thirty minutes of reading found both the
   confirmation *and* the solution. A day of spiking DirectComposition would have
   found neither.
3. **Test where it breaks, not that it works.** The four tests the author specified
   were all more pointed than "does it work". Two of them — hit-testing and flicker —
   were where an unexamined assumption would have shipped a broken player.

---

## Sections for the rest of Phase 1

*(Lean format, per ADR-0016. Filled in as the phase completes.)*

### What we built

- **Three de-risking spikes, all passed.** libmpv embeds in Tauri and
  hardware-decodes; librqbit already provides most of the Phase 7 scheduler;
  `ort` embeds a query in 1.63 ms p95 against a 30 ms trigger. Throwaway code in
  `spikes/`.
- **The application shell.** Tauri v2 + React 19 + TypeScript strict + Vite +
  Tailwind 4, custom title bar, five-destination nav rail that remembers its
  collapsed state, honest phase-badged placeholders.
- **A generated IPC boundary.** `tauri-specta` derives `src/lib/ipc.ts` from the
  Rust command signatures on every debug run, so the two sides cannot drift.
- **`crates/tiers`** — §8 hardware detection and capability gating, with the
  Settings screen that shows what the machine enables in plain language, and what
  happens instead where it does not.
- **Logging and a crash handler** writing to portable `data/` next to the exe.

### Why this approach

- Player composition: ADR-0021. Still-frame on pause, cutouts during playback.
  Chosen over DirectComposition because it needs no patched dependency and is
  measured; over HTML5-plus-remux because that trades away "plays everything".
- `SPEC.md` §9.3 amended by ADR-0020 rather than carrying a requirement the platform
  cannot meet.
- **Testable logic moved out of `src-tauri` (ADR-0022).** Not a preference — a test
  binary that links Tauri cannot launch on Windows at all, and the targeted fix is
  nightly-only. Rejected: nightly Rust, manifest-via-rustflags (breaks every
  dependency), and UI-only testing (§12.1 requires unit tests).
- **Cold start measured to *interactive*, not to window creation.** The flattering
  number was 267 ms; the honest one is ~515–660 ms. The window is created hidden and
  revealed when the frontend reports it has painted.

### New concepts

| Concept | Where |
|---|---|
| Dynamic FFI, `libloading`, no import library | `spikes/spike-a-libmpv/src/mpv.rs:83` |
| `unsafe impl Send` and why not `Sync` | `spikes/spike-a-libmpv/src/mpv.rs:90` |
| mpv node API, tagged union over FFI | `spikes/spike-a-tauri/src/snapshot.rs:28` |
| `SetWindowRgn`, region ownership transfer | `spikes/spike-a-tauri/src/cutout.rs:44` |
| `WM_ERASEBKGND` suppression to stop flicker | `spikes/spike-a-tauri/src/main.rs` (`video_host_proc`) |
| `RealChildWindowFromPoint` vs `WindowFromPoint` | `scratchpad/test1_probe.ps1` |
| Tauri `with_webview` → `ICoreWebView2Controller2` | `spikes/spike-a-tauri/src/main.rs` (setup) |
| Windows DPI awareness and coordinate spaces | harness; see the note on `ClientToScreen` |
| Zustand store with selective `persist` | `src/lib/store.ts:26` |
| TanStack Query defaults for a local backend | `src/main.tsx:12` |
| React error boundary (class component, `getDerivedStateFromError`) | `src/app/ErrorBoundary.tsx:14` |
| `tauri-specta` generated bindings | `src-tauri/src/lib.rs:26` (`ipc_builder`) |
| Specta forbids `u64`/`usize` across IPC (JS numbers are f64) | `crates/tiers/src/lib.rs:101` |
| `OnceLock` for process-start timing | `src-tauri/src/lib.rs` (`PROCESS_START`) |
| Tauri capability permissions (allowlist per window) | `src-tauri/capabilities/default.json` |
| Cargo workspace + why `test = false` on the Tauri crate | `src-tauri/Cargo.toml:22`, ADR-0022 |
| DXGI adapter enumeration for GPU detection | `crates/tiers/src/lib.rs` (`probe_gpu`) |
| Tailwind v4 CSS-first theme (`@theme inline`) | `src/styles/global.css:6` |

### Five self-check questions

*(Written now, asked at the Phase 8 tier boundary — §10.10 as amended by ADR-0016.)*

1. Why can't HTML be drawn on top of video in a Tauri window on Windows? What is the
   actual mechanism, not just "it doesn't work"?
2. The pause overlay and the playback chrome use two completely different techniques.
   Why can't the pause overlay's approach be used during playback?
3. `screenshot-raw` is called with `window` rather than `video`. What breaks if you
   change that one word?
4. The region cutout doesn't flicker. Name the two things that make that true — and
   what you would see if either were removed.
5. wry PR #1762 would let us keep the original §9.3 design. Why did we not use it,
   and what would have to change for that answer to flip?
