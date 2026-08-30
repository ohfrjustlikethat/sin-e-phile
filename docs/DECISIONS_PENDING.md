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
| [P2 — Do embeddings count as "AI/ML training" under TMDB's terms?](#p2) | **Phase 5** | 2026-08-31 |
| [P3 — Can a MovieLens-derived similarity matrix be redistributed?](#p3) | **Phase 16** | 2026-08-31 |
| [P4 — Licensing the three extracted crates for actual reuse](#p4) | Phase 27 | 2026-08-31 |
| [P5 — Where the review queue's confidence threshold is set](#p5) | Phase 12 | 2026-08-31 |

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

## P2 — Do embeddings count as "AI/ML training" under TMDB's terms? {#p2}

**Decide by:** Phase 5 · **Raised:** 2026-08-31, live verification of §14 terms
· **Risk:** R11

TMDB's current API terms **prohibit use of the data for AI/ML training**. Phase 5's
document text builder composes each item's embedding input partly from TMDB
synopses, then runs a sentence-transformer over it.

Computing an embedding is **inference, not training** — no model weights are
updated, and the output is a representation of the text rather than a derived model.
That reading is almost certainly correct. But "almost certainly" is not the standard
for a term that could invalidate the project's use of its primary enrichment source,
and the distinction deserves a deliberate ruling rather than an assumption nobody
wrote down.

**What makes this low-stakes in practice:** ADR-0013 already made TMDB optional, and
the offline IMDb + MovieLens catalogue is the base. If the conservative reading is
taken, Phase 5 builds embedding documents from IMDb/MovieLens fields plus AniList,
and TMDB contributes artwork only — a real but survivable loss of synopsis text.

**Options.** (a) Proceed on the inference-not-training reading, and record it
explicitly in the case study. (b) Exclude TMDB text from embedding input; use it for
display only. (c) Ask TMDB directly — they are responsive, and a written answer
settles it permanently.

**Leaning:** (c), then (a). Asking costs an email and removes the ambiguity
completely.

---

## P3 — Can a MovieLens-derived similarity matrix be redistributed? {#p3}

**Decide by:** Phase 16 · **Raised:** 2026-08-31, live verification of §14 terms
· **Risk:** R11

GroupLens states it **does not generally permit public redistribution** of the
MovieLens datasets, and requires a usage form to be completed.

Phase 16 precomputes an **item-item similarity matrix** from MovieLens and ships it
— either bundled or, per ADR-0014, as a versioned release asset. A similarity matrix
is a *derived statistical artefact*, not the ratings data: it contains no user
identifiers and no individual ratings, only aggregate item-to-item relationships.
Whether that counts as redistribution is a real question and not one to answer by
assumption.

**Options.** (a) Ship the derived matrix, on the reading that an aggregate statistic
is not the dataset. (b) Do not ship it; have the user's machine build it during
first-run ingestion from a dataset they download themselves — slower first run, no
redistribution question at all, and it fits the existing ingestion pipeline. (c) Ask
GroupLens.

**Leaning:** (b) is the clean answer and needs no permission from anyone. It costs
first-run time on Tier 0 hardware, which is exactly where that hurts most — so
measure it in Phase 4 before committing.

---

## P4 — Licensing the three extracted crates for actual reuse {#p4}

**Decide by:** Phase 27, and before any of them is published separately
· **Raised:** 2026-08-31, ADR-0007

`SPEC.md` §7 extracts `filename-parser`, `subtitle-align` and `source-protocol` into
standalone crates deliberately, because they are self-contained, genuinely reusable,
and make the repository read as engineering rather than one application blob.

But as part of a GPL-3.0 repository they are GPL-3.0 — and the Rust ecosystem is
overwhelmingly MIT/Apache-2.0, which is compatible in one direction only. So the
three crates specifically chosen for reuse are, as licensed, unusable by most of the
ecosystem that would want them.

Note that none of the three links libmpv or FFmpeg. `filename-parser` is pure string
handling; `source-protocol` is types and a schema. `subtitle-align` uses FFmpeg for
VAD extraction, but by invoking the binary rather than linking it, which is a
materially different licensing situation and needs checking rather than assuming.

**Options.** (a) Leave them GPL-3.0 — simplest, and the reuse claim becomes
rhetorical. (b) Dual-license them MIT/Apache-2.0 within this repository, which is
possible for code we wholly own. (c) Publish them from separate repositories under
MIT/Apache-2.0 and depend on them here, which is also the strongest portfolio
story — three published crates.

**Leaning:** (c) for `filename-parser` and `source-protocol`, which are clean;
check `subtitle-align`'s FFmpeg relationship before deciding it.

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

## Resolved

*(none yet — resolved entries move here with the ADR that settled them)*
