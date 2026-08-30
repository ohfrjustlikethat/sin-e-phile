# 0002 — Tauri v2 over Electron

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0 (recording a decision locked in `SPEC.md` §5)
- **Risk:** R1

## Context

The application needs a desktop shell that can render a rich, animated, poster-heavy
interface and embed a native video surface, on Windows, and be **excellent on a weak
machine** — `SPEC.md` §2.3 budgets Tier 0 at under 250 MB idle RAM, under 4 s cold
start, and under 120 MB installed.

The author is learning both Rust and React, so the shell also determines how much of
the project is written in the language whose learning is a stated goal.

## Decision

**Tauri v2.** The frontend is React/TypeScript rendered in the system WebView2. The
backend is a Rust process. The two communicate over a typed IPC boundary.

## Consequences

**Easier.** Binaries in the ~10 MB range rather than ~150 MB, because the webview
belongs to the operating system rather than being bundled. Idle memory that fits the
Tier 0 budget rather than exceeding it before the app does anything. The backend is
Rust, which is where the torrent engine, the scheduler and the search index need to
live anyway — so the interesting work happens in the language that is the strongest
CV signal.

**Harder.** Tauri's ecosystem is far smaller than Electron's: fewer worked examples,
more first-party implementation. Embedding libmpv (**R1**) is genuinely fiddlier
than it would be under Electron — severe enough to warrant Spike A in Phase 1 before
anything depends on it.

**Harder.** The IPC boundary is a real boundary. Every backend capability the
frontend needs must be explicitly exposed as a command, with types that must not
drift — which is why Phase 1 requires *generated* TypeScript types rather than
hand-written ones.

**Constraint accepted.** WebView2 must be present. It ships with current Windows 11,
and `doctor` checks for it.

## Alternatives Considered

**Electron.** Rejected on §2.3 alone. A bundled Chromium costs roughly 150 MB
installed and 200+ MB idle, failing the installed-size and Tier 0 RAM budgets before
a single feature exists. It would also pull application logic toward Node,
undercutting the point of the project.

**Native Win32/WinUI 3.** Genuinely the best fit for shell integration (Phase 20) and
for embedding a video surface. Rejected because a rail-heavy, animated media UI is
far faster to build well in React, §9's design system assumes CSS, and React is the
more transferable skill.

**egui or another Rust-native immediate-mode GUI.** All-Rust and appealing. Rejected:
§9's typography, motion and layout requirements fit immediate-mode poorly, and it
would mean building the entire design system up from primitives.

**wry directly, without Tauri.** Tauri *is* the useful layer over wry — bundling,
updater, IPC, plugins. Rejected as reinventing it.
