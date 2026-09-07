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

> **RESOLVED 2026-09-04.** Author answered "your call", which by the §10 protocol takes
> the stated default: **option 1**. `absolute_number` stays NULL, `SPEC.md` §6.2 and §11
> are amended (spec_version 1.8.0, amendments A22-A23), and the gap is recorded against
> Phase 12 as debt D22. See [ADR-0031](adr/0031-absolute-episode-numbering-is-not-published.md).
> Blocker B1 cleared. Kept below because the reasoning is what Phase 12 will need.

**Raised:** 2026-09-04, Phase 4 subtask 4.4. **Blocker B1 — cleared.**

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

---

## P11 — Which HNSW implementation {#p11}

> **RESOLVED 2026-09-06.** Author answered "your call", which takes the stated default:
> **`usearch`**, with recall measured against a brute-force baseline before any
> relevance number is reported. Confirmed by probe before committing to it: it builds on
> this machine in 52 s, accepts `ScalarKind::I8`, and `Index::view()` memory-maps a saved
> index rather than loading it — which is the property the whole decision rested on.

**Raised:** 2026-09-06, Phase 5 subtask 5.3. **Resolved same day.**

### The decision, in one sentence

`SPEC.md` §2 locks "HNSW, persisted" as the vector index and Phase 5 adds "memory-mapped";
which library provides it, over 855,703 vectors of 384 int8 dimensions.

### The options

1. **`usearch`** — C++ bindings. **The only one that memory-maps a persisted index**,
   which is what §15 Phase 5 actually asks for and what keeps Tier 0's idle RAM inside
   the §2.3 250 MB budget: a 313 MB index loaded into memory does not fit there.
   **Cost:** a third C/C++ dependency after ONNX Runtime and libwebp — all three already
   need the MSVC toolchain, so it adds no new build requirement, but it is more foreign
   code the author must be able to speak about.

2. **`instant-distance`** — pure Rust, serde persistence. **Cost:** loads the whole
   index into memory. At 313 MB of vectors plus graph overhead that breaks the Tier 0
   RAM budget outright, which is the tier this artefact exists to serve (ADR-0015).

3. **`hnsw_rs` / `fast-hnsw`** — pure Rust with file dump and load. **Cost:** same
   memory question as (2) unless they mmap; needs checking rather than assuming, which
   is a half-day before any code is written.

4. **Write it.** HNSW is a skip-list of proximity graphs — explainable, and the
   portfolio value is real. **Cost:** it is the one algorithm here where a subtle bug
   produces *plausible but worse* results rather than a failure, and Phase 5's exit
   criteria are measured numbers. Recall would need its own eval before the search eval
   could be trusted.

### Recommendation

**(1) `usearch`**, because the memory-mapping requirement is not decoration: it is what
lets Tier 0 hold a 313 MB index inside a 250 MB RAM budget, and options 2 and 3 do not
obviously satisfy it. I would rather take a third C++ dependency than quietly miss a
§2.3 budget.

**(4) is the one I would enjoy most and would advise against**, for the same reason the
AniList matcher refuses to guess: a wrong-but-plausible answer is the expensive kind.

### The default if you say "your call"

**Option 1**, with the recall measured against a brute-force baseline on the fixture
corpus before any relevance number is reported — so that if the index is the reason a
number is bad, that is visible rather than inferred.

---

## P12 — The catalogue has no synopses, so semantic search has nothing to be semantic about

**Raised:** 2026-09-07 (Phase 5, subtask 5.4) · **Blocks:** E3, E4 · **Decide by:** Phase 5
**DECIDED 2026-09-07 — option 2, Wikidata.** The author has no TMDB key and does not want
one as a build dependency. That also removes ADR-0018's licensing question from the
published artefact completely, since no TMDB-derived text goes into it. P12 stays open
only until the re-measurement below says whether Wikidata's coverage is enough; if it is
not, the fallback is a per-user key enriching *live*, not a rebuilt artefact.

### The decision

Where the text that describes what a film is *about* comes from — because there is
currently none, and E3 and E4 cannot be met without it.

### What was found

`eval search --query "films about grief that aren't depressing"` — E4's own example —
returns ten films literally **titled** "Grief". The vector half is working correctly over
documents that cannot support the question:

```
Placebo (2002), animation comedy short film, featuring Jim Carrey…
```

That is the whole document. Title, year, genres, kind, cast.

```sql
SELECT COUNT(*) FROM media_items WHERE in_core = 1 AND kind <> 'episode'
   AND synopsis IS NOT NULL AND length(trim(synopsis)) > 0;   -- 0 of 855,703
```

`SPEC.md` Phase 5 specifies the document as "synopsis, genres, keywords, director, mood
descriptors, and era". **Three of those six do not exist in this catalogue.** IMDb's free
datasets carry no plot text; synopses were to come from TMDB, which ADR-0027 makes
optional and absent by default — the configuration the artefact was built in.

ADR-0018 already anticipated exactly this, specifying a **swappable text source**
(`tmdb` / `imdb` / `wikidata`) so its contribution could be measured. The seam was
specified and never exercised, and nothing noticed because every artefact check verifies
the file is *correct*, not that the documents are *informative*.

### The options

1. **Your TMDB key, used once, by you, to build the artefact.** ADR-0014 already has the
   author producing the artefact and publishing it, so users still need no key — they
   download vectors, not text.
   **Cost:** you need a TMDB key. ~855,000 requests at TMDB's ~50/s is about 5 hours of
   wall clock, plus a re-embed (~35 min) and an index rebuild (~7 min). Scoped to the
   most-voted 100,000 titles it is ~35 minutes of fetching instead, and covers the part
   of the catalogue anyone actually searches. Also leans on ADR-0018's reading that
   embedding is inference, not training — a reading we recorded as probably-right and
   whose enquiry letter (`docs/correspondence/tmdb-ai-clause.md`) is still unsent.

2. **Wikidata/Wikipedia abstracts — no key, no rate limit, bulk download.** Matches to us
   by IMDb id (Wikidata property P345), which is an exact join rather than a guess.
   **Cost:** a new ingestion loader of roughly the size of `ingest imdb` — a day or two,
   not an afternoon. Coverage is partial and skewed to notable films; obscure titles stay
   empty. Fits the zero-key posture perfectly and needs nothing from you.

3. **Accept it, and record E3/E4 as not met.** Semantic search stays a title, genre, cast
   and era matcher, which is genuinely useful for "Kurosawa samurai films" and useless
   for "films about grief".
   **Cost:** two exit criteria fail, in the phase that is the centrepiece of the whole
   project. `SPEC.md` §10.11 forbids redefining them, so this means carrying them as
   failed — honest, and a bad look in a portfolio whose selling point is semantic search.

### Recommendation

**Option 2, then option 1 if it is not enough.** Wikidata costs my time rather than your
money, keeps the zero-key posture that ADR-0013 and ADR-0027 built the whole catalogue
around, and — the part that decides it — the resulting artefact carries no TMDB-derived
text at all, so the ADR-0018 licensing question stops mattering for the thing we publish.

Option 1 is faster and I would take it if this were a deadline. It is not.

### The default if you say "your call"

**Option 2, scoped to the core tier**, measured before and after with `eval search
--query` on E4's two queries, and P12 stays open until that measurement says whether
option 1 is still needed.

---

## P13 — The embedding model cannot do what E3 and E4 ask of it

**Raised:** 2026-09-07 (Phase 5, blocker B4, §10.9 escalation) · **Blocks:** E3, E4

### The decision

Whether to swap `all-MiniLM-L6-v2` for a retrieval-trained model, accept E3/E4 as not
met, or add a re-ranking stage.

### What was tried, and what each cost

Three genuinely distinct approaches, each measured (`docs/eval-results.md`):

1. **Enrich the documents.** 233,114 Wikipedia synopses where there were **zero**.
2. **Index only items with descriptive text.** 855,703 → 189,470; index 435 MB → 96 MB.
3. **Remove the title from the document** (`VERSION` 2 — the spec never listed it), plus
   a 300-character minimum to drop trivia-length hubs. → 119,874.

Each helped. The result that ends the sequence:

| item | cosine to a plot query | synopsis |
|---|---|---|
| **Manchester by the Sea** — synopsis contains the query almost verbatim | **0.1944** | 1,847 chars |
| **Fish Hooky** — "the 120th Our Gang short to be released" | **0.4194** | 126 chars |

Giving Manchester its *whole* synopsis reaches 0.2689 — still far behind trivia.

**This is hubness.** Short generic documents sit near the centre of the embedding space
and are close to everything. It cannot be fixed with more text, because those documents
have no more text.

**The diagnosis:** `all-MiniLM-L6-v2` is a **symmetric similarity** model — trained to
say whether two sentences mean the same thing. E3 and E4 need **asymmetric retrieval** —
a short query against a long document. They are different tasks.

### The options

1. **Swap to `bge-small-en-v1.5`.** Trained for retrieval, MIT licensed, **384
   dimensions** — so the artefact format, the index, the quantiser and the whole
   pipeline are unchanged. ~33 M parameters, comparable download to what ships now.
   **Cost:** an ADR (the model is checksum-pinned), a re-embed (75 min), an index
   rebuild (28 s). It requires query/passage **prefixes** — `"Represent this sentence
   for searching relevant passages: "` on the query side — which are not optional and
   are most of where the benefit comes from. **Risk, stated honestly: I cannot promise
   it fixes E4.** It is the established fix for this exact failure, not a guarantee.

2. **`e5-small-v2`.** Same size and dimension, same prefix requirement (`query:` /
   `passage:`), MIT. Equivalent choice; `bge` benchmarks slightly better on retrieval.

3. **Add a re-ranker.** A cross-encoder over the top 50 candidates would fix ordering
   properly and is what production systems do. **Cost:** a second model, and it runs
   *per query* rather than once — straight into E1's 80 ms Tier 0 budget. Wrong shape
   for this project.

4. **Accept it.** Semantic search understands era, genre, nationality and creator — "like
   Wong Kar-wai but Korean" now returns *My Mister* — but not plot. Record E3 and E4 as
   not met. **Cost:** the two criteria fail in the phase that is the project's
   centrepiece, and §10.11 forbids redefining them.

### Recommendation

**Option 1.** It is one pinned constant, one prefix, and 75 minutes of compute, against
two exit criteria in the phase this project is built around. Nothing else in the
pipeline changes, and if it disappoints, option 4 is still there — measured rather than
assumed, which is the whole point.

### The default if you say "your call"

**Option 1**, with the three queries in `docs/eval-results.md` re-run before and after,
and E3/E4 still not claimed unless the numbers earn it.

### What is unaffected

E1 (p95 22.2 ms), E2 (43/43 = 100%) and recall@10 (0.9810) are all healthy. This blocks
E3 and E4 and nothing else.
