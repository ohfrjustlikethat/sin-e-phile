# 0033 — Wikipedia abstracts are the embedding text source

- **Status:** Accepted
- **Date:** 2026-09-07
- **Phase:** 5 (subtask 5.4 found the need; the loader lands in Phase 5)
- **Resolves:** P12
- **Amends:** `tools/guard/allowlist.txt` (adds two domains, per ADR-0010)
- **Relates to:** ADR-0018 (swappable text source), ADR-0027 (TMDB is per-user), ADR-0014
- **Risk:** R11

## Context

Phase 5's first real semantic query returned ten films literally *titled* "Grief".

The cause was not the index, the metric or the fusion — all three measured correctly.
It was the documents:

```sql
SELECT COUNT(*) FROM media_items WHERE in_core = 1 AND kind <> 'episode'
   AND synopsis IS NOT NULL AND length(trim(synopsis)) > 0;   -- 0 of 855,703
```

Every embedded document was `Title (Year), genres kind, featuring cast`. Nothing about
what a film is *about*, so the embedding space encoded little more than title words.

`SPEC.md` Phase 5 specifies the document as "synopsis, genres, keywords, director, mood
descriptors, and era". **Three of those six do not exist in this catalogue.** IMDb's free
datasets carry no plot text. Synopses were to come from TMDB — which ADR-0027 makes
optional, per-user and absent by default, and which is exactly the configuration the
catalogue was built in.

ADR-0018 anticipated this precisely, specifying a **swappable text source**
(`tmdb` / `imdb` / `wikidata`) so its contribution could be measured. The seam was
specified and never exercised, and nothing noticed: every artefact check verifies the
file is *correct*, not that the documents are *informative*.

## Decision

**English Wikipedia lead extracts, joined to the catalogue through Wikidata's IMDb ID
property (P345), are the embedding text source.** The author chose this over supplying a
TMDB key.

### Why this over a TMDB key

- **No key, for anyone.** ADR-0013 and ADR-0027 built the entire catalogue around being
  complete and good-looking with no key. A build step that needed one would be the first
  crack in that.
- **It removes the ADR-0018 licensing question from the published artefact entirely.**
  ADR-0018 records a *reading* — that embedding is inference, not training — which is
  probably right and whose confirming enquiry is still unsent. No TMDB text goes into the
  artefact now, so the reading no longer has to hold for the thing we distribute.
- **The join is exact, not a guess.** P345 is IMDb's own id recorded on the Wikidata item.
  Matching by title and year would be the kind of plausible-but-wrong matching the anime
  matcher already refuses to do.

### Measured before deciding

| | |
|---|---|
| Items with both an IMDb id and an English article | **509,464** (WDQS `COUNT`, 59 s) |
| One mapping chunk (`tt004` prefix) | 7,513 rows in **11.6 s** |
| Extract quality | ~1,000–1,200 characters of lead text |

A representative extract:

> *Grave of the Fireflies is a 1988 Japanese animated war film written and directed by
> Isao Takahata … Based on Akiyuki Nosaka's 1967 semi-autobiographical short story of the
> same name, the film is set in Kobe shortly after its bombing by the U.S.*

Period, genre, form, authorship and subject — the things a semantic query asks about.

### Scope and cadence

- The loader walks the core tier **in descending vote order**, so a bounded run covers the
  part of the catalogue anyone actually searches. Same pattern as `ingest anime --pages`.
- Resumable, like every other loader, because a full run is hours.
- The mapping is chunked by IMDb-id prefix and subdivides adaptively when a chunk is too
  large, since WDQS times out at 60 s.

### Licensing, stated rather than assumed

Wikipedia text is **CC BY-SA**. That is materially more permissive than TMDB's terms for
this use — there is no clause restricting machine learning — but it carries an attribution
and share-alike obligation, so:

- **Storing and embedding locally: unencumbered.** The text sits in the user's own
  database and is fed to a model on their machine.
- **The published artefact contains vectors, not text.** It is derived, is not a
  substitute for the articles, and cannot be read back as prose.
- **If the UI ever displays an abstract, it must attribute**: the article title, a link,
  and the licence. That is a requirement on Phases 9 and 11, recorded here because the
  obligation is created by this decision and would otherwise be discovered late.

### Recording which source produced an artefact

ADR-0018 requires the artefact to name its text source, and the format has no such field.
**`FORMAT_VERSION` goes to 2** and the header gains `text_source`. A version bump rather
than a quiet use of the reserved bytes, because the failure it prevents is silent: two
artefacts identical in model, dimension and builder version but built from different text
are otherwise indistinguishable, and comparing queries against the wrong one degrades
search with nothing to point at.

The consequence is accepted deliberately: the `embeddings-v1` release already published
becomes unreadable to this build. It is superseded by the re-embed this ADR requires
anyway.

## Consequences

- Two domains join `tools/guard/allowlist.txt`: `wikidata.org` and `wikipedia.org`. Both
  are metadata sources in the ADR-0010 sense — neither indexes or locates content.
- A new loader, `ingest wikipedia`, of roughly the size of `ingest akas`.
- A full re-embed (~35 min at the measured 2.4 ms per document) and an index rebuild
  (~7 min), then E4's two queries re-measured with `eval search --query`.
- **P12 stays open** until that measurement. Wikipedia's coverage is real but partial and
  skewed to notable titles; if the measurement says it is not enough, the fallback is a
  per-user TMDB key enriching *live*, which is a different mechanism from a rebuilt
  artefact and does not disturb this decision.
- E3 and E4 remain unmet until the re-measurement, and are not claimed on the strength of
  a plan.

## Alternatives Considered

- **The author's TMDB key.** Faster (~5 h of fetching, or ~35 min scoped to the top
  100,000) and better covered, especially for recent and non-English titles. Rejected by
  the author: no key, and it would make the artefact depend on one.
- **Wikipedia abstract dumps** (`enwiki-latest-abstract.xml.gz`, ~1 GB). No API traffic at
  all, but the dump carries no IMDb id, so joining means matching on title and year — a
  guess, and the wrong-but-plausible kind.
- **Accept it and record E3/E4 as failed.** Honest, and available at any time if the
  measurement disappoints. Rejected while a zero-cost source is untried.
