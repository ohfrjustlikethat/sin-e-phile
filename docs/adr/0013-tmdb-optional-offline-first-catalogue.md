# 0013 — TMDB is optional; the catalogue is offline-first

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0 (implemented in Phases 4 and 14)
- **Amends:** `SPEC.md` §14, Phase 4, Phase 14
- **Risk:** R4

## Context

`SPEC.md` §14 marks TMDB as **required** — the primary source of films, TV,
artwork and cast. Meanwhile §3.3 and Phase 14 promise that a non-technical person
installs the app and is watching something in under two minutes, through a
three-screen wizard, without reading documentation. None of those three screens is
"register at a metadata provider and paste an API key".

Both cannot hold. The two obvious resolutions are both blocked:

- **Ship a key in the build.** Violates §12.6 (no secret in a public repository),
  violates TMDB's terms, and the key would be extracted and rate-limited to death
  within days of the repository being public.
- **Require every user to obtain their own key.** Breaks the two-minute promise for
  every user, and makes the app non-functional out of the box — which for a
  portfolio project means non-functional for the person evaluating it.

This surfaces in Phase 4 and again in Phase 14, and the schema and onboarding
consequences differ enough that it cannot be deferred.

## Decision

**The application is completely functional with no key of any kind.**

The offline catalogue — IMDb datasets plus MovieLens, both free downloads requiring
no account — provides titles, years, runtimes, genres, cast, crew, and ratings. On
top of that base, everything the project considers its actual engineering runs
unchanged: semantic search (Phase 5), the taste model (Phase 15), the recommender
(Phase 16), and the discovery engine (Phase 17). None of them require TMDB.

**TMDB becomes an optional enrichment step**, offered inside the onboarding wizard
as roughly thirty seconds of work, framed as what it actually delivers — *unlock
artwork and rich detail* — with a "later" option that is not a dead end and can be
completed from Settings at any time. **No key ever ships.**

### The consequence that needed solving

The Phase 14 onboarding includes an adaptive **poster grid** for cold-start taste
elicitation. A poster grid needs posters, and posters come from TMDB. Without a key
the signature onboarding step is an empty scaffold — the exact "degraded to
something broken or empty" failure §8 forbids.

So the cold-start taste step **defaults to the three methods that need no artwork**:

1. **Library import** — Letterboxd CSV, Trakt, IMDb ratings export, MAL/AniList XML.
   The strongest signal available, and it requires no artwork at all.
2. **Taste statements** — pace, ambiguity, era, subtitles, runtime, formal
   experimentation. Already specified in Phase 14, already artwork-free.
3. **Free-text** — "describe what you love", embedded directly into the taste vector.

The **poster grid appears only once artwork is available**, as an additional round
for users who completed the TMDB step.

This is not a consolation prize. For a cinephile application a **typographic
title-card grid** — title, year, director, set in the display serif from §9.2 — is
arguably the better design: it selects on knowledge and taste rather than on poster
recognition, which is exactly the discrimination the taste model wants. It also
looks more distinctive than another poster wall.

## Consequences

**Easier.** The two-minute promise survives, and survives for *every* user rather
than only for users who already have a key. §12.6 is preserved with no tension.
R4 (catalogue ingestion risk) is hardened, because the critical path no longer
depends on a third-party API's continued free tier — §14 itself warns that terms
change. A reviewer cloning the repository gets a working application immediately,
which for a portfolio project is close to the whole point.

**Easier.** Offline-first is now structural rather than aspirational. Phase 5's
"works fully offline" exit criterion becomes a property of the design instead of a
feature needing separate defence.

**Harder.** Two visual states for every surface that displays artwork — with and
without. Phase 2's design system must produce a genuinely good artwork-free
`PosterCard` (typographic fallback), not a grey rectangle, and Phase 18 must look
correct in both. This is real additional design work and it must not be deferred to
Phase 27.

**Harder.** Phase 4's ingestion must produce a genuinely useful catalogue from IMDb
and MovieLens alone. Synopses in particular are thinner without TMDB, which affects
the Phase 5 embedding document builder — the composed text will lean harder on
genres, keywords, crew and era. The Phase 5 eval harness should record nDCG both
with and without TMDB enrichment, so the cost of running key-free is measured rather
than assumed.

**Honest limitation.** Without TMDB the app has no artwork. That is a real
experience gap, not a neutral trade. The mitigation is that acquiring a key is
thirty seconds and is offered at the moment the user first meets a surface that
would benefit from it.

## Alternatives Considered

**A fourth onboarding screen with a guided key flow.** The straightforward fix, and
rejected only narrowly. It still makes every user do setup work before a working
app, and it makes the very first impression a registration form. The chosen option
strictly dominates: the guided flow still exists, but it is optional and it appears
after the user already has something working.

**Proxy TMDB through a small hosted service holding the key.** Rejected: introduces
a server component, which §5 forbids outright and §2.7 makes untenable. Also a
running cost, which §2.6 forbids.

**Bundle a prebuilt artwork pack.** Rejected: distribution size, unclear licensing
of the images, and it goes stale. Artwork is exactly the kind of asset that should
be fetched on demand and cached (Phase 4 already specifies the cache).

**Drop TMDB entirely.** Rejected: artwork and rich detail are a large part of what
makes the browsing surfaces (Phase 18) and the pause overlay (Phase 11) good. The
goal is that TMDB is not *required*, not that it is unused.
