# 0022 — Testable logic lives outside the Tauri crate

- **Status:** Accepted · **Date:** 2026-08-31 · **Phase:** 1
- **Relates to:** `SPEC.md` §7, §12.1

## Context

- `cargo test` inside `src-tauri` **cannot run at all** on Windows. Every test
  binary dies at load with `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139), before a
  single test executes.
- Cause: tao (via Tauri) imports `SetWindowSubclass`, `RemoveWindowSubclass`,
  `DefSubclassProc` and `TaskDialogIndirect`, which **comctl32 v6** exports and v5
  does not. Binding v6 needs a side-by-side manifest. `tauri-build` embeds one into
  the *application* binary but not into test binaries.
- Three fixes were tried. A manifest via `.cargo/config.toml` rustflags applies to
  every dependency in the graph and fails them all with `LNK1327`. Via
  `cargo::rustc-link-arg` it applies to the app binary too, which already has a
  manifest — `CVT1100: duplicate resource`. The targeted key,
  `cargo::rustc-link-arg-tests`, is **rejected by stable cargo** (verified on
  1.98.0 in an isolated crate; it needs nightly `-Z extra-link-arg`).
- This is not a `tiers` problem. It would block unit tests for the filename
  parser, the source scorer, the subtitle aligner and the recommender — everything
  §12.1 requires tests for.

## Decision

**`src-tauri` carries no test harness** (`test = false` on both targets) and stays
what §7 already says it should be: a thin IPC and wiring layer with no logic worth
testing.

**All testable logic lives in `crates/`**, which do not depend on Tauri. `src-tauri`
re-exports them so call sites are unchanged. First instance: `crates/tiers`, holding
the §8 tier logic with 8 unit tests.

**This is an architectural constraint, not a preference** (author's ruling,
2026-09-01). **Check it at every phase:** if logic is going into `src-tauri` that
ought to be testable, it belongs in a crate instead. The question "is this testable?"
and the question "does this belong in `src-tauri`?" have the same answer, and
`src-tauri` stays what §7 already calls it — a thin IPC surface with no logic.

## Consequences

- Unit tests run, on stable, with no nightly flag and no manifest games.
- **This makes §7's `crates/` split load-bearing rather than aspirational.** It was
  framed as being about reuse and about the repository reading as engineering; it is
  now also the only way pure logic is testable. `filename-parser`, `subtitle-align`
  and `source-protocol` were already planned as crates, so the direction was right.
- New pressure toward a good habit: anything worth testing must be pulled out of the
  Tauri crate, so "is this testable?" and "does this belong in `src-tauri`?" become
  the same question.
- Cost: more crates and a workspace to maintain, and a re-export shim per extracted
  module. Small, and paid once.
- **Integration tests that genuinely need a running Tauri app remain impossible**
  under `cargo test`. Those belong in the §12.3 manual test plan, or in a WebDriver
  harness later. Recorded as debt D7 rather than pretended away.

## Alternatives Considered

- **Nightly `-Z extra-link-arg`.** Rejected: `SPEC.md` §5 pins stable Rust, and a
  nightly requirement would land in `SETUP.md` for every contributor.
- **Ship a manifest for all targets via rustflags.** Tried; breaks every dependency
  build with `LNK1327`.
- **`cargo::rustc-link-arg` for the whole crate.** Tried; the app binary then has two
  manifests and the link fails with `CVT1100`.
- **Skip unit tests in `src-tauri` and test only through the UI.** Rejected: §12.1
  requires unit tests for all pure logic, and a UI-only test suite is slow, flaky,
  and cannot cover the property tests §12.1 asks for.
- **A separate integration-test crate that links Tauri.** Rejected: it hits exactly
  the same loader failure, since the problem is linking Tauri at all in a test binary.
