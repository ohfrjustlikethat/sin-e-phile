> **CLOSED, UNSENT — 2026-09-01 (ADR-0027).** The author closed this: no key ever
> ships, each user supplies their own under their own acceptance of TMDB's terms,
> and nothing derived is redistributed. The posture stands without a ruling, so the
> enquiry is not sent and is no longer tracked. Kept because the analysis below is
> the useful part and the Phase 27 case study may want it.

---

# Draft enquiry to TMDB — the AI/ML training clause

**Status:** drafted 2026-08-31, **not yet sent**. For the author to send.
**Context:** ADR-0018, pending decision P2, risk R11.

Send via the TMDB support form or `travis@themoviedb.org`. Record the reply in this
file and update ADR-0018 when it arrives.

---

**Subject:** Clarification on the AI/ML clause — text embeddings for local search

Hello,

I am building a free, open-source, non-commercial desktop application (GPL-3.0) that
helps a user search and organise their own film library, and I would like to check
one point in the API terms before I rely on my own reading of it.

The terms prohibit using TMDB data for AI/ML training. I want to confirm how that
applies to **text embeddings used for local search**.

Specifically, the application would:

1. Take a film's synopsis, genres and keywords from TMDB.
2. Run that text through an existing, pre-trained, open-source sentence-transformer
   model to produce a vector representation.
3. Store that vector locally on the user's own machine, and use it so the user can
   search their library by meaning — for example "slow films about grief" — rather
   than by exact title.

To be explicit about what it does **not** do: no model is trained, fine-tuned, or
modified in any way. No model weights are updated. The model is used only for
inference, exactly as it ships. The vectors are stored on the end user's own device
and are never redistributed, published, or used to train anything. Nothing is sent
to any server — the application has no backend.

My reading is that this is inference rather than training, and therefore permitted.
I would rather confirm that with you than assume it.

Two smaller questions while I am asking:

- The terms limit non-commercial caching to six months. Does that six-month limit
  apply to a **derived** artefact such as an embedding vector, or only to the cached
  TMDB response itself?
- Is there a published numeric rate limit you would like clients to respect? I could
  not find one in the terms, and I would prefer to configure a conservative limit
  deliberately rather than guess.

The application displays the required attribution and the "not endorsed, certified,
or otherwise approved by TMDB" notice, and TMDB is an optional enrichment layer — it
works without an API key at all.

Thank you for maintaining a genuinely free and well-documented API.

Best regards,
Jonathan Majors
