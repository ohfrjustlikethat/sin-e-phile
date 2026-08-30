# 0001 — Record architecture decisions

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0

## Context

This project runs for months across dozens of sessions, with long gaps between
them (`SPEC.md` §10.11 exists precisely because the author will forget). Its
author is new to Rust and React and has committed to being able to explain every
part of it in an interview (`SPEC.md` §2.2).

Both facts create the same need. A decision made in Phase 6 will be questioned in
Phase 16, by an author who no longer remembers the reasoning and by an assistant
whose context has been compacted. Without a record, one of two bad things happens:
the decision is silently reversed because nobody remembers why it was made, or it
is preserved out of superstition because nobody dares touch it.

Code shows *what* was decided. Commit messages show *when*. Neither reliably
captures *why*, or — more valuable in an interview — what was rejected and on what
grounds.

## Decision

Use Architecture Decision Records, in the form popularised by Michael Nygard,
stored as sequentially numbered Markdown files in `docs/adr/`.

Every ADR has four sections: **Context** (the forces in play, written so it makes
sense to someone who wasn't there), **Decision** (what was chosen, in the active
voice), **Consequences** (what this makes easier and what it makes harder — both,
honestly), and **Alternatives Considered** (what was rejected and why).

Rules:

- ADRs are written **at the moment of deciding**, never reconstructed afterwards.
  A reconstructed ADR records the justification, not the reasoning, and the
  difference is exactly what makes it worthless.
- ADRs are immutable once Accepted. A reversed decision gets a *new* ADR that
  supersedes the old one, and the old one is marked `Superseded by NNNN`. The
  wrong turns stay in the record; they are part of the story.
- Numbers are never reused. Reserved numbers are marked as such in the index.
- Any change to `SPEC.md` requires an ADR first (`SPEC.md` §2.8).

## Consequences

**Easier.** Cold resumes after a long gap. Answering "why did you do it that way?"
in an interview with a real answer rather than a shrug. Distinguishing a decision
that was carefully made from one that was accidental — which is the distinction
that determines whether it is safe to change.

**Harder.** Every non-obvious decision now costs fifteen minutes of writing. This
is a real tax and it will be tempting to skip it on a session that feels
productive. The mitigation is that `SPEC.md` §11.2 makes ADRs a hard per-phase
deliverable, not a discretionary one.

**Risk.** ADRs can rot into ceremony — files written to satisfy the rule, saying
nothing. The test for whether an ADR is worth its cost is the Alternatives section:
if nothing was genuinely considered and rejected, there was no decision, and the
ADR should not exist.

## Alternatives Considered

**A wiki or a design-doc folder.** Rejected: not version-controlled alongside the
code, so it drifts. The value of an ADR is that it sits in the same commit as the
change it explains and travels with a `git clone`.

**Long-form comments in the code.** Rejected: the natural home for "why this
function is written this way", but the wrong home for "why this library at all",
because the code that would carry the comment is precisely the code that gets
deleted when the decision is reversed.

**Commit messages only.** Rejected: commit messages are optimised for reviewing a
change, not for being found six months later by someone who doesn't know which
commit to look in. Conventional commits are used as well (`SPEC.md` §10.5); the
two solve different problems.

**Nothing — rely on `SPEC.md`.** Rejected: `SPEC.md` records the *current* state of
the decisions. It deliberately carries no history, because §2.8 requires stale text
to be replaced rather than patched around. ADRs are where the history lives, which
is what makes it safe for `SPEC.md` to have none.
