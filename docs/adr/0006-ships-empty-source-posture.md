# 0006 — The ships-empty source posture

- **Status:** Accepted
- **Date:** 2026-08-31
- **Phase:** 0 (recording the decision locked in `SPEC.md` §2.1)
- **Risk:** R10

## Context

This application resolves media from multiple backends, one of which is BitTorrent.
That places it in a category where the difference between projects that survive and
projects that are removed from GitHub is **not** the technology. It is whether the
project ships pointers to infringing content.

Popcorn Time bundled sources. Its forks were taken down. Stremio ships an addon
protocol and no sources, and Stremio still exists.

The distinction is not cosmetic and it is not a disclaimer. It is architectural: a
tool that resolves what a user points it at is a general-purpose tool, whereas a
tool that arrives pre-pointed at infringing sources is a distribution mechanism for
them, whatever its README says.

This is also a portfolio project intended to be shown to employers. A repository
that has been taken down cannot be shown to anyone.

## Decision

**The application ships zero content sources**, and the repository contains none —
not in code, config, test fixtures, documentation, or **git history**.

What ships instead is the machinery:

- A documented, versioned **`SourceBackend` protocol** (Phase 6) that any backend
  can implement.
- A **declarative addon manifest format** — TOML/JSON, no executable code — so a
  source is described as data rather than shipped as an implementation. Declarative
  matters: parsing untrusted data is a bounded problem, executing untrusted code is
  not.
- A **catalogue browser** that fetches a catalogue from a URL **the user supplies**.
  No default URL ships.
- Working reference backends against **unambiguously legal sources only**: the local
  filesystem, the Internet Archive's public-domain collection, and user-supplied
  M3U or direct HTTP.

**When a phase is tempted to add a convenient default, the answer is no.** If a
feature cannot be demonstrated without a source, it is demonstrated against the
local-filesystem or Internet Archive backend.

## Consequences

**Easier.** The repository can be public from commit one, which is what makes the
honest commit history usable as portfolio evidence. The legal posture becomes
something to *explain in an interview* rather than something to be evasive about.

**Easier, unexpectedly.** Forbidding defaults forces the source layer to be a real
abstraction rather than a wrapper around one assumed provider. Phase 9's scoring
function ranks candidates across all backends with one function precisely because no
backend is privileged, and adding debrid later is ~200 lines rather than a rewrite.
The constraint improved the architecture.

**Harder.** Every user configures their own sources. This is genuinely ~30 seconds
of work, but it is not zero, and Phase 14's onboarding must make it painless and
must leave a fully functional app when it is skipped.

**Harder.** Testing anything source-dependent needs legal fixtures. The Internet
Archive backend carries real weight here: it is not a token gesture but the thing
Phases 6, 7 and 8 actually demo and measure against.

**Harder — and the real long-term cost.** A human rule cannot survive 28 phases and
dozens of sessions. It will be broken by accident: a URL in a fixture, a doc
example, a "temporary" default. That is **R10**, rated Severe, and it is why the
rule is enforced by `tools/guard/` in CI and pre-commit, scanning history as well as
the working tree, and **verified** in Phase 0 rather than assumed.

Two consequences flow from that and are recorded separately: ADR-0009 (the guard
cannot use a plaintext denylist without violating this very posture) and ADR-0011
(Phase 12's realistic-filename corpus collides with it, and needs a redaction policy
decided in advance).

## Alternatives Considered

**Ship a small default source list, "just to make it work out of the box".** The
tempting option, and rejected absolutely. It is precisely what distinguishes the
projects that were removed from the ones that were not. §2.1 anticipates this
temptation by name and pre-answers it, which is the correct way to handle a decision
that will feel wrong in the moment.

**Ship sources but gate them behind a disclaimer or an "I understand" checkbox.**
Rejected: a disclaimer does not change what the software does, and shipping the
pointers is the act that matters.

**Executable addons — a plugin API where an addon supplies code.** Rejected on two
grounds. Security: executing untrusted code from a user-supplied URL is a remote
code execution vector by design. And posture: an executable plugin ecosystem makes
the project the distributor of a runtime for scrapers. Declarative manifests keep
the attack surface to parsing, which Phase 6 must harden anyway.

**No BitTorrent backend at all.** Would remove the question entirely, and would also
remove the hardest and most interesting engineering in the project — the Phase 7
streaming scheduler. Rejected: the protocol is not the problem, and a general
BitTorrent engine is a lawful thing to build.
