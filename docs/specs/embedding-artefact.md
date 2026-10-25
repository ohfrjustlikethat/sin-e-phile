# The embedding artefact

**Status:** implemented (Phase 4, subtask 4.10) · **Decides:** [ADR-0014](../adr/0014-embedding-artefact-distribution.md), [ADR-0015](../adr/0015-tier-0-query-embedding.md)
**Code:** `crates/embedding/` (format, shared) · `tools/ingest/src/embed.rs` (producer)

A single file holding one vector per core-tier title, published as a GitHub Release
asset and downloaded with consent. This document is what a second implementation would
need to read it — and, more importantly, the reasoning behind the parts that look
arbitrary.

---

## 1. Why the file exists at all

Tier 0 hardware can embed a *query* in about 1 ms (Phase 1, Spike C) but cannot embed
855,703 *documents* in any reasonable time. ADR-0015 splits those: queries are embedded
on every tier, documents are not. So the documents are embedded once, on a capable
machine, and shipped.

The alternative — Tier 0 falling back to FTS5 keyword search alone — still works and is
deliberately kept as the degraded path. It is diminished, never broken.

## 2. Layout

All integers little-endian. Offsets in bytes.

```
0      8    magic          "SINEEMB\0"
8      2    format_version u16, currently 1
10     2    dimension      u16, 384 for all-MiniLM-L6-v2
12     1    quantisation   u8, 1 = int8
13     4    doc_builder    u32, the document-builder version
17     8    count          u64, number of vectors
25    64    model          length-prefixed UTF-8, e.g. "all-MiniLM-L6-v2-int8"
89    16    snapshot_date  length-prefixed UTF-8, "YYYY-MM-DD"
105  151    reserved       zero
256   ...   vectors        count * dimension bytes, int8
       32   checksum       SHA-256 of every byte before it
```

**The header is a fixed 256 bytes** so the vectors begin at a constant offset. A reader
can memory-map the file and index into it arithmetically — `offset = 256 + n *
dimension` — without parsing anything first.

**Vector order is the index.** Vectors are written in ascending `media_items.id` over
the core tier, and the *n*th vector belongs to the *n*th core title in that order.
Nothing inside the file records which id that is: storing 855,703 ids would cost 6.8 MB
to say something the caller already knows, and the caller must run the same ordered
query anyway to use the result.

> **The consequence, stated because it is a real hazard.** An artefact and a catalogue
> that disagree about which titles are core will silently misalign. The `snapshot_date`
> exists to make that detectable; a reader that ignores it will produce results that
> are wrong rather than absent.

## 3. What is checked, and in what order

1. **Magic and format version.** A file that is not an artefact, or is a newer format.
2. **Length.** `count * dimension` must match the bytes actually present.
3. **Checksum.** SHA-256 over the header and vectors, verified **at load**, not lazily
   — a corrupt file discovered on the four-hundred-thousandth lookup has already been
   trusted for a long time.
4. **Model identity**, compared verbatim against the model the application holds.
5. **Document-builder version**, compared against the running build's.

**Steps 4 and 5 matter more than step 3.** A corrupt download fails loudly. A
*mismatched* artefact fails silently: vectors produced by one model, compared against
queries produced by another, land in different regions of a space that has no idea
anything is wrong. Search simply gets worse, gradually, with nothing to point at.

The identity therefore names the **quantisation** as well as the model.
`all-MiniLM-L6-v2-int8` and `all-MiniLM-L6-v2-fp32` are different models for this
purpose, because they produce different vectors.

## 4. Quantisation

int8, symmetric, **scaled per vector**.

Sentence-transformer output is L2-normalised, so a single global scale would be
defensible — and would clip the largest components of any vector with an unusual
magnitude. Per-vector costs nothing to store (the scale is recoverable from the data,
since the input was normalised) and cannot clip.

`-128` is **never emitted**. Its negation does not fit in an `i8`, so a dot product that
negates a component overflows on exactly one value in 256 — a bug that appears once in
a million comparisons and never reproduces.

**The property that must survive is order, not accuracy.** Cosine similarity between
384-dimension unit vectors moves by less than 0.005 under quantisation, and a vector's
similarity to its own round trip exceeds 0.9999. Ranking is unchanged, which is the
only thing search asks of it.

## 5. Determinism

> Same catalogue snapshot + same model + same document-builder version ⇒ byte-identical
> file.

This is a requirement from ADR-0014 and it constrains the format more than anything
else in this document. It means:

- **No build timestamp anywhere.** `snapshot_date` describes the *catalogue*, taken from
  `MAX(date(updated_at))`, not the moment of writing. A file that records when it was
  built cannot be diffed against a rebuild, and "deterministic" becomes a claim nobody
  can check.
- **Fixed-width fields**, no variable-length encoding, no maps.
- **Every producer query ordered.** Alternative titles, genres and billing are read with
  an explicit `ORDER BY`; an unordered read produces a different *sentence*, and
  therefore a different vector, for the same catalogue.
- **Reserved bytes are zero**, so a future reader that uses them and a current one that
  ignores them still produce identical files for identical content today.

## 6. The document

The sentence fed to the model, built by `crates/embedding/src/document.rs`:

```
Title (Year), also known as A and B, genre1 genre2 kind, featuring P1, P2 and P3.
Synopsis, truncated at 400 characters on a word boundary.
```

Prose, not a bag of fields, because the model was trained on prose. The kind is
*described* (`anime_series` → "anime series"), never named as an identifier.
Alternative titles are de-duplicated case-insensitively against the primary title and
each other — AniList's romaji and English forms are identical for a great many titles,
and repeating a name skews its own embedding.

**`document::VERSION` must be bumped for any change to this, including a cosmetic one.**
A different separator is a different string is a different vector. The version is
recorded in the header and checked at load; it is the only thing that makes a
document-builder change detectable rather than a slow, invisible degradation.

## 7. Producing it

`ingest embed`. Resumable, because 855,703 inferences is a long run.

Vectors are appended to a `.part` file and the job cursor is simply how many have been
written. A resume truncates to the last **whole** vector and continues — a process
killed mid-write leaves a few stray bytes, and without discarding them every subsequent
vector is misaligned by that many bytes while the file remains perfectly well-formed.

**The checkpoint guarantee here is weaker than elsewhere in the pipeline, and the code
says so.** Every other resumable job commits its cursor in the same transaction as the
work it describes. No transaction spans a database and a file, so this one upholds a
weaker invariant instead: *flush before checkpointing*, so the cursor can only lag what
is durably on disk, never lead it. A resume then redoes a little work rather than
skipping some — and skipping would lose vectors from the middle of an artefact, which
nothing downstream could detect.

The final file is assembled at the end, which is also when the checksum is computed, so
an interrupted run never leaves something that looks complete.

## 8. Size

| | |
|---|---|
| Core-tier titles | 855,703 |
| Dimensions | 384 |
| **Artefact** | **313 MB** |

Arithmetic over the layout above, not an estimate. ADR-0014 budgets ~77 MB per 200,000
titles, which this matches. §2.3's 120 MB installed cap excludes optional downloads, so
the artefact sits outside it — but the number is recorded here and in
`docs/eval-results.md` so catalogue growth cannot inflate it unnoticed.
