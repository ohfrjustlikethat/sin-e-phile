# 0007 — GPL-3.0, and the dependency licence audit

- **Status:** Accepted
- **Date:** 2026-08-31
- **Phase:** 0 (recording a decision locked in `SPEC.md` §5)

## Context

The project links two pieces of software whose licences propagate: **libmpv**
(GPL/LGPL depending on build configuration) and **FFmpeg** (LGPL-2.1+, or GPL when
built with GPL-licensed components such as x264 or several filters). Media tooling
of this kind is copyleft territory by default, and pretending otherwise is how
projects acquire licence violations they only discover later.

A licence also has to be chosen *before* there is code to license, because
retrofitting one across a codebase with external contributions is far harder than
choosing correctly at the start.

## Decision

**GPL-3.0**, with **source published and no compiled installers distributed**.

A **dependency licence audit** is a hard deliverable of Phase 27, with results
documented and the GPL obligations satisfied. `cargo audit` and `npm audit` run in
CI from Phase 0.

## Consequences

**Easier.** The obligation that libmpv and FFmpeg linkage imposes is simply
satisfied rather than worked around. There is no need to reason about whether a
particular FFmpeg build configuration pulls in GPL components, because the
project is already GPL.

**Easier.** It is the correct posture for the project on its own merits: a tool of
this kind that a user runs on their own machine should be inspectable and
modifiable by that user. Combined with §2.7's no-telemetry rule, "you can read
exactly what this does and it sends nothing anywhere" is a claim that can be
verified rather than merely asserted.

**Consequence, not cost — publishing source only.** No compiled installers are
distributed. This sidesteps distributing binaries that combine GPL components, and
suits a portfolio project, where the audience clones and reads rather than installs.

**Harder.** Every future dependency must be licence-compatible. GPL-3.0 is
incompatible with some licences that look permissive at a glance — notably certain
older Apache-1.1-style and CDDL components — and any proprietary SDK is out. This
constrains, for instance, what could be used for optional debrid integrations or
platform SDKs in Phase 20. The audit exists to catch this, but the cheaper moment is
at the point of adding a dependency.

**Harder.** Rust's ecosystem is overwhelmingly MIT/Apache-2.0, which is compatible
in this direction (permissive code can be incorporated into a GPL-3.0 work) but
**not** the reverse. That means `crates/filename-parser`, `crates/subtitle-align`
and `crates/source-protocol` — which §7 extracts precisely because they are
genuinely reusable — are GPL-3.0 as part of this repository, and so are unusable by
most of the ecosystem that would otherwise want them.

**This deserves a later decision rather than a silent acceptance.** Dual-licensing
those three crates permissively, or publishing them from separate repositories under
MIT/Apache-2.0, would make them genuinely reusable and strengthen the portfolio
argument that they are standalone engineering. Logged as **P4** in
`docs/DECISIONS_PENDING.md`, to be decided before those crates are published, not
after.

## Open question flagged in the Phase 0 audit

`SPEC.md` §5 says "source only — no compiled installers published", while Phase 27
lists **packaging** as a deliverable and §2.3 budgets **installed size** — which
implies an installer exists. These are reconcilable (build locally, distribute
nothing), but the spec does not say so explicitly. Logged as **P1** in
`docs/DECISIONS_PENDING.md`; it needs resolving before Phase 27, not during it.

## Alternatives Considered

**MIT or Apache-2.0.** Maximum reuse and the most portfolio-friendly at first
glance. Rejected because it is not honestly available: linking libmpv and a
GPL-configured FFmpeg makes the combined work GPL regardless of what the repository
declares. Declaring MIT over a GPL-derived work is a licence violation, not a
choice.

**LGPL-3.0.** Would permit the extracted crates to be used more widely. Rejected as
the wrong shape: LGPL is designed for libraries linked into larger works, whereas
this repository is an application. The reuse problem it would address is better
solved by P4 — licensing the three genuinely reusable crates separately — than by
weakening the application's licence.

**AGPL-3.0.** Its network-use clause is the distinguishing feature, and this project
has no network service by design (§5 forbids a server component). Rejected as
solving a problem that cannot occur here.

**No licence at all.** Rejected. Absent a licence, default copyright applies and
nobody may legally use, modify or redistribute the work — the opposite of what a
public portfolio repository is for, and a violation of the GPL obligations
inherited from libmpv and FFmpeg.
