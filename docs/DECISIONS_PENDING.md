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
| **P7** — the visual direction, and how player chrome is composited | [ADR-0023](adr/0023-visual-direction.md), [ADR-0021](adr/0021-player-composition-architecture.md) | The compositing half was resolved in Phase 1: still-frame substitution on pause, region cutouts during playback, no patched dependency. The design half is resolved by the author's brief — MUBI-led editorial direction, warm near-black palette, an explicit banned list, and a falsifiable test ("would this pass as a MUBI or Criterion screen?"). ADR-0020's solid opaque player panel is treated as suiting the direction rather than as a limitation. |
| **P4** — licensing the extracted crates for actual reuse | [ADR-0017](adr/0017-dual-license-extracted-crates.md) | `filename-parser`, `subtitle-align` and `source-protocol` are **MIT OR Apache-2.0**; the app stays GPL-3.0 from libmpv and FFmpeg linkage. Binding design constraint: **`subtitle-align` must not depend on FFmpeg** — it takes PCM samples or a precomputed VAD signal, and extraction lives in the app. Licence-clean, and testable without spawning a process. |


---

## P9 — compile-time-checked SQL, or runtime-checked?

**Raised:** 2026-09-01 (Phase 3) · **Decide by:** before Phase 4 · **Owner:** the author

`SPEC.md` §2's technology table says: *"SQLite via `sqlx` (compile-time checked
queries)… `sqlx` catches SQL errors at compile time — valuable for a learner."*

Phase 3 shipped the data layer using **runtime-checked** `sqlx::query()`, not the
compile-time-checked `query!` / `query_as!` macros. That is a deviation from the
stated rationale for the dependency, so it is being put in front of the author
rather than quietly kept.

### What compile-time checking would buy

A typo'd column name, a wrong table, or a query that does not match the schema fails
the **build** instead of a test. For a learner that is genuinely valuable — it is the
same argument as Rust's type system, applied to SQL.

### What it costs, honestly

- **Dynamic SQL cannot use it at all.** `query!` needs a literal string. The
  archive's source-preference query is built with `format!`, so it stays
  runtime-checked regardless. The rule would have an exception from day one.
- **SQLite nullability inference is weak.** sqlx cannot tell whether a column in a
  join is nullable, so most columns need `as "column!"` annotations. This is churn
  across every query, and the annotations are themselves unchecked assertions —
  getting one wrong turns a compile-time guarantee into a runtime panic.
- **It adds a build prerequisite and a ritual.** `cargo sqlx prepare` must be re-run
  after every schema change, `sqlx-cli` must be installed (so `tools/doctor` grows a
  check), and the `.sqlx/` cache must be committed or CI needs a live database.

### The options

1. **Convert now.** Highest cost, but Phase 3 is the smallest the data layer will
   ever be — Phase 4's ingestion pipeline writes far more SQL.
2. **Accept runtime-checked, and amend `SPEC.md` §2** to say sqlx was chosen for
   async SQLite with a good migration story, which is true and is what is actually
   being used.
3. **Convert selectively** — macros for static queries, runtime for dynamic — and
   document the boundary.

**Recommendation: (1) or (2), not (3).** A rule with a documented exception is a rule
people follow; a rule applied case by case is one that erodes. If the compile-time
guarantee is worth having it is worth having everywhere it can apply, and if it is
not, the spec should stop claiming it. What should *not* happen is the spec saying one
thing and the code doing another, which is the situation today.

The timing matters: converting is cheapest now and gets steadily more expensive.

---

## P10 — Absolute episode numbering, when no source publishes one {#p10}

**Raised:** 2026-09-04, Phase 4 subtask 4.4. **Blocker B1.**

### The decision, in one sentence

`SPEC.md` §6.2 requires absolute-versus-seasonal episode reconciliation and assumes
AniList supplies the absolute number — **it does not**, so we either derive it, source
it elsewhere, or stop claiming to have it.

### What is actually true

Seasonal numbering is loaded and correct: 539,817 episodes from IMDb `title.episode`,
with `episode_numbering.source = 'imdb'`. `absolute_number` is NULL on every row.

AniList publishes an episode **count** and an airing schedule per *entry*, and an
AniList entry is one cour. Its numbering therefore restarts at 1 each season, in
exactly the same place IMDb's does. There is no absolute number to read.

An absolute number can only be **derived** — order a series' AniList entries and
cumulate their episode counts. That is precisely what migration 0003 warns against, in
its own words: *"the conversions are not arithmetic, because cours split unevenly,
recaps are numbered by some sources and not others, and specials interleave"*. The
schema was built to store what each source said and reconcile by lookup, specifically
so that nobody would compute this.

There is a second problem. We map **one** AniList entry per catalogue series (first
claim wins, by year then id), so seasons 2+ resolve to `already_claimed` and carry no
AniList id at all — 409 of them. Even a derivation has nothing to iterate over.

### The options

1. **Leave `absolute_number` NULL, and amend §6.2** to say the schema *supports*
   absolute numbering and that it is populated when a source that publishes one is
   added. **Cost:** Phase 12's filename matcher meets `Series - 59` and cannot resolve
   it from the catalogue; it must fall back to asking the user or to a fuzzy guess,
   for anime specifically. That is a real gap in the feature the spec calls out.

2. **Store per-season AniList numbering properly first** — stop collapsing seasons onto
   one catalogue item, give each AniList entry its own `seasons` row and its own
   `episode_numbering` rows. **Cost:** a redesign of the claim rule, roughly a session,
   and it changes data already written. It also makes the absolute number *derivable*
   rather than derived, which is the honest version of option 3.

3. **Derive and store it now** — cumulate episode counts across ordered seasons.
   **Cost:** one line of arithmetic that is silently wrong for every series with a
   recap episode, a split cour, or an interleaved special — which is most long-running
   anime, i.e. exactly the ones this feature exists for. Migration 0003 exists because
   of this.

4. **TVDB** publishes absolute numbering directly and is the only source that does.
   **Cost:** an API key per user, a fifth external dependency, and §2.4's zero-cost
   rule to check against their current terms.

### Recommendation

**(1) now, (2) when Phase 12 needs it.** The gap is real but it is Phase 12's gap, not
Phase 4's, and option 2 is a redesign that should be driven by the code that needs it
rather than guessed at eight phases early. Option 3 is the one to refuse outright: a
number that is confidently wrong is worse than a NULL, and this schema was designed
specifically to avoid it.

### The default if you say "your call"

**Option 1.** Leave `absolute_number` NULL, amend §6.2 to describe what is actually
stored, and record the derivation problem against Phase 12 so it arrives with the
context rather than rediscovering it.
