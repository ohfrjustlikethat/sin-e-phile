# 0054 — Current on every launch: the catalogue, what it means, and what to recommend

- **Status:** Accepted 2026-11-18. The author: "will it stay constantly updated? i need it to stay updated when it
  comes to movies, shows, anime, anything … THIS APP NEEDS TO BE UPDATED ALL THE TIME ON EVERY LAUNCH."
- **Date:** 2026-11-18
- **Phase:** 17
- **Amends:** ADR-0030 (freshness in layers: extended from the catalogue to everything derived from it), ADR-0015
  (Tier 0 and document embedding: a launch's new titles are not the catalogue), ADR-0019 (which MovieLens release the
  app builds from). `SPEC.md` §3.3, §8, §15 Phase 17; spec_version 4.7.0.

## Context

**What is current today.** Phase 16 made ADR-0030 automatic: at every launch, then hourly, the app checks whether IMDb
has republished its files and adds the new titles, episodes and ratings; AniList's airing schedules every six hours;
TMDB's "now playing" and "airing" for a profile with a key. Search finds a new film, show or anime by name the day the
catalogue gains it.

**What is not.** Everything *derived* from the catalogue is a snapshot:

- **The meaning index** comes from the published file (`embeddings-v3`, 119,874 titles of the catalogue it was made
  from). Nothing gives a title added since then a vector, although the risk register (R3) says new items get one "on
  next catalogue refresh". A new title is invisible to search by meaning and to the content half of the recommender.
- **The item-item matrix** (Phase 17) is built from MovieLens' ml-25m, whose ratings end in 2019. A film released
  last month has no neighbours.

So a recommender built only on these could reach a new release through popularity alone, and never with a reason.

## Decision

Everything the app knows is brought current **on every launch**, in the background, after the catalogue check and in
this order, never holding the window back (§2.3's cold start):

1. **The catalogue** (ADR-0030, unchanged): IMDb, AniList airing, TMDB now playing and airing. It is the trigger for
   the rest: each step below starts from what this one added.
2. **Meaning for what is new.** Each title the catalogue gained since the last launch that is worth indexing gets a
   synopsis — the profile's TMDB overview when it has a key, AniList's description for anime (no key), else its
   Wikipedia lead found through Wikidata (ADR-0033's two steps, asked for these titles only) — then a vector from the
   model the installer carries (ADR-0053), inserted into the meaning index. **On every tier**: ADR-0015 protects a
   weak machine from embedding the catalogue, hundreds of thousands of documents; a launch's new titles are tens to a
   few thousand, so the batch is capped (2,000 a launch, the rest the next) and runs while idle. Tier 0 still never
   embeds the catalogue.
3. **"People who loved this also loved", live.** MovieLens is a snapshot that GroupLens updates rarely, so two sources
   that change every day stand beside it in the neighbour graph, each row marked with where it came from: AniList's
   community recommendations for anime (no key) and TMDB's recommendations for films and series (the profile's own
   key, ADR-0013). They are asked for the titles the profile liked or watched recently and for new titles, kept for a
   week, and asked again at the first launch after that.
4. **MovieLens' newest release.** The app builds its matrix from the newest full release GroupLens publishes (its
   rolling `ml-latest`), not the fixed ml-25m; at launch it compares the release's `Last-Modified` with the one it
   built from and rebuilds when it changed (about 10 s). ml-25m stays the fixed set the evaluation is measured on, so
   `docs/eval-results.md` can be reproduced. When GroupLens cannot be reached (its certificate has lapsed before), the
   last matrix stays.
5. **The day a title arrives**, before anyone has rated it, it gets neighbours by meaning (step 2) and by shared people
   (its director, writers and leads), so the ranker can reach it at once with a reason that is true.
6. **Rising now.** Each refresh keeps the previous refresh's IMDb vote counts, so the ranker's freshness term knows what
   is gaining viewers this week, not only what is new.
7. **Recommendations follow.** Home's rails are ranked again after each step lands, and Settings says, per source, when
   it last updated and what it added.

## Amendment, 2026-11-18 (A44, spec_version 4.7.1): every title without meaning

Step 2 said "each title the catalogue gained since the last launch". Run on a copy of the development catalogue, those
were 19,052 core titles past the published file, the most voted with 4,741 votes, and Wikipedia described 23 of the first
2,000; meanwhile 1,331 core titles with 10,000 votes or more had no vector, because they had no synopsis long enough
when the file was made (Doctor Who, One Punch Man, Solo Leveling, Delicatessen). The wording would never have reached
them: the snapshot this ADR exists to end. So step 2 covers **every core title the published file does not hold**,
gained since or left out, most-voted first, under the same cap of 2,000 a launch. A title nobody could describe is asked
again after a week when it came out in the last year (its article is often written in the weeks after), otherwise
after 90 days, so old gaps do not take every launch's turn.

## Consequences

- A title the catalogue gains can be recommended on the launch that gains it, with a reason: Phase 17 exit criterion
  E7, and subtask 17.6.
- Each launch costs a few small checks (IMDb's files, MovieLens' `Last-Modified`), and a background batch sized by what
  changed: requests within AniList's and TMDB's rate limits, a few milliseconds of embedding per new title.
- Without a TMDB key, films' and series' live neighbours come from MovieLens, meaning and people; anime always has
  AniList's. The degradation is graceful, as ADR-0013 requires.
- No telemetry (§2.7): what leaves the machine is title ids, to the public metadata services the app already uses
  (AniList, Wikidata and Wikipedia, GroupLens) and to TMDB under the profile's own key.

## Alternatives rejected

- **Republish the meaning file every day.** It needs a pipeline that someone runs for every install: a server
  component in all but name, which `SPEC.md` §5 rules out.
- **Leave MovieLens as it is.** Recommendations would stop at 2019 for good.
- **Read ratings or reviews from websites** (Letterboxd, IMDb's pages). Their terms forbid it, and §2.1's posture holds.
- **Trakt's related titles.** They need a registered client id, which would mean shipping a key (ADR-0013, ADR-0027).
