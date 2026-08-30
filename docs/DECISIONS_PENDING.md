# Decisions pending

Things deliberately deferred, so that *"we'll decide later"* never quietly becomes
*"we forgot"*.

Every entry names the phase by which it must be decided. A decision reached here
becomes an ADR and, if it changes `SPEC.md`, an amendment under §2.8. Entries are
removed only when resolved — with a line saying which ADR resolved them.

**Reviewed at the start of every phase named in the "Decide by" column.**

| | Decide by | Raised |
|---|---|---|
| [P1 — Source-only distribution versus Phase 27 packaging](#p1) | Phase 27 | 2026-08-30 |
| [P5 — Where the review queue's confidence threshold is set](#p5) | Phase 12 | 2026-08-31 |
| [P6 — Windows Sandbox pass on a genuinely bare machine](#p6) | Phase 27 | 2026-08-31 |

**Resolved:** P2 (ADR-0018), P3 (ADR-0019), P4 (ADR-0017) — see [Resolved](#resolved).

---

## P1 — Source-only distribution versus Phase 27 packaging {#p1}

**Decide by:** Phase 27 · **Raised:** 2026-08-30, session 0a audit (item 12)

`SPEC.md` §5 says GPL-3.0, "source only — no compiled installers published".
Phase 27 lists **packaging** among its deliverables, and §2.3 budgets **installed
size < 120 MB** — which implies an installer exists. Phase 20 adds a further wrinkle:
Windows 11 context-menu entries require a packaged (sparse MSIX) application, so
"never package" and "integrate with the shell properly" are in tension.

These are reconcilable — build locally, package for shell registration, distribute
no binaries — but the spec does not say so, and an unstated reconciliation is the
kind of thing that gets resolved differently by two different sessions.

**Options.** (a) Packaging means local build artefacts only; publish nothing —
keeps §5 literal, and the installed-size budget becomes a measurement of a local
build. (b) Publish a signed installer as a GitHub Release, accepting the GPL
distribution obligations, which are satisfiable since the source is already public.
(c) Publish a portable ZIP rather than an installer, which fits §2.5's
portable-by-default posture better than an installer does.

**Leaning:** (a) for Tier B, revisit at Phase 27 if anyone actually wants to run it
without building. Not urgent, but must not be decided implicitly.

---




## P5 — Where the review queue's confidence threshold is set {#p5}

**Decide by:** Phase 12 · **Raised:** 2026-08-31

Phase 12 must hit **> 95% top-1 accuracy** and **< 1% false-confident** on the
filename corpus, with a "tunable confidence threshold" deciding what reaches the
review queue.

Those two targets pull against each other, and the threshold is the dial between
them: raise it and false-confidence falls while the review queue grows; lower it and
the queue empties while wrong-but-confident matches slip through.

The spec does not say which side to err on. It should, because the asymmetry is
real: a wrong-but-confident match silently mislabels a user's library and is
discovered much later, whereas an unnecessary review-queue item costs one click.

**Leaning, to be confirmed with data rather than intuition:** err toward the queue.
Choose the threshold *after* the corpus exists, by plotting both metrics against it
and picking the knee — and record the plot in the Phase 12 case study. This is a
"measure before deciding" item, which is why it is logged rather than guessed now.

---

## P6 — Windows Sandbox pass on a genuinely bare machine {#p6}

**Decide by:** Phase 27 · **Raised:** 2026-08-31

Exit criterion **E1** is now evidenced by a CI job that clones fresh on a clean
`windows-latest` runner and runs `doctor` plus the build. That is repeatable and
catches `docs/SETUP.md` rotting later, which a one-time manual check would not.

**Its limitation is real and is stated in the evidence string:** the runner ships
Rust, Node, MSVC and the Windows SDK preinstalled. So the job proves a clean
*checkout* builds; it does not prove the install instructions are complete for a
genuinely bare machine.

**Outstanding:** one pass in Windows Sandbox or a bare VM, following `SETUP.md`
literally from nothing, confirming every prerequisite link and step is correct.
Worth doing once before Phase 27, and again at Phase 27 when it becomes part of the
portfolio claim that a stranger can build this.

---

## Resolved

| | Resolved by | Decision |
|---|---|---|
| **P2** — do embeddings count as AI/ML training under TMDB's terms? | [ADR-0018](adr/0018-tmdb-embedding-text-and-swappable-source.md) | Inference, not training — recorded as a decision rather than an assumption. Enquiry drafted at `docs/correspondence/tmdb-ai-clause.md` for the author to send. Hedged structurally: the Phase 5 document builder takes a swappable text source via config, so an unfavourable answer is a config change and a re-embed, not a rewrite. |
| **P3** — can a MovieLens-derived matrix be redistributed? | [ADR-0019](adr/0019-movielens-matrix-computed-on-device.md) | Never ship one. Ingestion downloads MovieLens and computes the item-item matrix on the user's machine, so the question never arises. Deliberately a different answer from §8's shipped embeddings: embeddings computed from metadata we assemble are ours to publish; a matrix derived from GroupLens' ratings is not clearly ours. |
| **P4** — licensing the extracted crates for actual reuse | [ADR-0017](adr/0017-dual-license-extracted-crates.md) | `filename-parser`, `subtitle-align` and `source-protocol` are **MIT OR Apache-2.0**; the app stays GPL-3.0 from libmpv and FFmpeg linkage. Binding design constraint: **`subtitle-align` must not depend on FFmpeg** — it takes PCM samples or a precomputed VAD signal, and extraction lives in the app. Licence-clean, and testable without spawning a process. |
