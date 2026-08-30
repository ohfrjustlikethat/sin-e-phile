# 0014 — Embedding artefacts ship via GitHub Releases

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0 (implemented in Phase 4)
- **Amends:** `SPEC.md` §2.7, §8, Phase 4, Phase 5
- **Risk:** R3

## Context

`SPEC.md` §8 and Phase 5 both require that Tier 0 machines receive embeddings
"precomputed and shipped/downloaded, never computed on device". R3's fallback, if
ONNX Runtime proves too slow or too painful on Windows, escalates this to *all*
tiers: ship precomputed embeddings for the whole catalogue.

Three constraints appear to block this, and none of them was reconciled in the spec:

- §5 forbids "an external server component of any kind".
- §2.7 forbids anything leaving the machine except explicit user-configured API
  calls.
- §2.3 caps installed size at 120 MB, excluding optional models.

So the spec requires a download, from nowhere, of something nobody was scheduled to
produce. The last part is the most concrete gap: computing embeddings for a
catalogue of hundreds of thousands of titles is hours of compute, and **no phase
owned that work**.

## Decision

### Distribution

Embedding artefacts are published as **versioned static assets on GitHub Releases**
in this repository.

This is not a server component in the spirit of §5. §5's prohibition targets
*runtime dependence on infrastructure the project operates* — a backend the app
talks to in order to function, that costs money and can go down. A static release
asset is the same category as downloading the ONNX model itself, which §5 already
accepts, or as the IMDb and MovieLens dataset downloads §14 already requires. There
is nothing to operate, nothing to pay for, and nothing that can fail in a way that
degrades a running installation.

**§2.7 is amended to say so explicitly**, because the current wording does not
obviously permit it and an unamended spec would make this a silent deviation — the
exact drift §2.8 forbids.

### Consent

The download is **consented to, with the size shown, before it happens**. No
background fetch, no "we'll just grab this". The user sees what it is, what it
enables, and how large it is, and chooses. This is consistent with the §2.7 posture
and with Phase 22's identical treatment of the optional vision models.

### Production — the previously unowned work

**A new Phase 4 subtask** produces and publishes the artefact from a reproducible
script in `tools/ingest/`, run on the author's machine. Requirements:

- Deterministic: same catalogue snapshot plus same model version plus same
  document-builder version yields byte-identical output.
- Versioned and pinned: the artefact records the model identity, its quantisation,
  the embedding dimension, the document-builder version, and the catalogue snapshot
  date. The application refuses to load an artefact whose model identity does not
  match the model it has.
- Checksummed, with the checksum verified after download.
- Resumable, since it is a long-running job over hundreds of thousands of items.

The application must degrade correctly when the artefact is absent: Tier 1 and Tier
2 embed on device; Tier 0 falls back to FTS5/BM25 keyword search alone, which is
diminished but genuinely useful, never broken or empty (§8).

### Budget

INT8, 384 dimensions, roughly **77 MB per 200,000 titles**. §2.3's 120 MB installed
cap already excludes optional downloads, so this sits outside it — but the number is
recorded here and in `docs/PERFORMANCE.md` so that catalogue growth cannot quietly
inflate it unnoticed.

## Consequences

**Easier.** Tier 0 gets real semantic search. R3's fallback becomes a concrete,
already-built path rather than a plan, which is what makes it an actual mitigation.
Zero cost and zero operated infrastructure, so §2.6 holds.

**Harder.** A release process now exists and has to be maintained. Re-ingesting the
catalogue means regenerating and republishing, and a stale artefact paired with a
newer catalogue produces items with no embedding. The version pinning above is what
makes that detectable instead of mysterious.

**Harder.** The author has to actually run a multi-hour job on their own machine,
and re-run it whenever the model or document builder changes. Naming this in the
ADR is deliberate: it was invisible work before, and invisible work is the kind that
does not get done.

**Constraint.** GitHub Releases has a 2 GB per-asset limit. At the stated density
that is comfortable for a catalogue in the low millions, but a much larger catalogue
would require splitting into shards. Noted rather than solved; R4 already scopes the
catalogue by popularity threshold.

## Alternatives Considered

**Bundle embeddings in the installer.** Rejected: blows the §2.3 size budget for
every user including those who never search semantically, and forces a full
reinstall to refresh them.

**Hugging Face Hub.** A good fit, and genuinely close. Rejected only because it adds
a second hosting dependency and a second domain to the allowlist (ADR-0010) for no
capability GitHub Releases lacks — and the release is already versioned in lockstep
with the code that reads it.

**Compute on device for all tiers, dropping the Tier 0 exemption.** Rejected: it is
precisely what §8's Tier 0 rules forbid, and R3 exists because it may be infeasible.
Spike C in Phase 1 measures whether it is even viable on Tier 1.

**A self-hosted static file host.** Rejected: a running cost (§2.6) and something to
operate (§5), for no advantage over a release asset.
