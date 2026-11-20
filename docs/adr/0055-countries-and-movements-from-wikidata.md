# 0055 — Countries and movements from Wikidata, for the discovery engine

- **Status:** Accepted 2026-11-19 (ADR-0040: decided and recorded, not a reserved decision).
- **Date:** 2026-11-19
- **Phase:** 18
- **Amends:** ADR-0033 (Wikidata, asked by IMDb id, now for two more facts as well as articles), ADR-0054 (one more
  source kept current by the hourly check). `SPEC.md` §8 (schema: migration 0033), §15 Phase 18; spec_version 4.8.0.

## Context

Phase 18's blind-spot arm is "a well-regarded region of the catalogue the user has never touched: a country, a decade,
a movement", and the spec's own example label is *"Your blind spot: Iranian New Wave"*. A stretch is "one axis away from
a taste mode", and country is the axis a viewer notices most.

Checked on a copy of the development catalogue before planning: **no title has a country or an original language.**
The IMDb datasets carry neither; `media_items.countries` (migration 0001, ISO codes for a filter chip) was written only
by a TMDB path the author does not use (no key, P12). Decades and genres are there for every film. Countries and
movements are not there at all, so without a source the blind spot can only ever say "the 1950s" or "westerns".

The app already asks Wikidata at launch: ADR-0054's meaning step maps new titles to their Wikipedia articles through
WDQS, by IMDb id (property P345), under ADR-0033's rate limit and identifying user agent.

## Decision

**The hourly check also asks Wikidata where each title comes from (P495, country of origin) and which film movement it
belongs to (P135)**, by IMDb id, each as its English label.

- **What is asked, in order**: every title some profile has met and that was never asked (an episode as its series),
  whatever its votes, then films and series with at least 1,000 votes, most voted first. 6,000 titles a run, 100 ids a
  query, each allowed WDQS's own 60 s (amended 2026-11-20: 200 a query timed out at the transport's 30 s on a slow day,
  39 s a query, in the release build's first launches). Measured on a copy of the development catalogue: every eligible title, 69,124, in 2,150 s (about 6 s a query
  with WDQS's own time), 64,560 with a country (93%) and 404 with a movement; so a run is about six minutes of an hour's
  background, and the first dozen hours the app is open ask everything (at the amended rate).
- **Stored** in a new table (migration 0033, `title_regions`: title, axis, name), with `regions_asked` recording every
  title asked, answered or not, so a title Wikidata does not know is not asked again each hour.
- **English names, not ISO codes.** The Soviet Union, West Germany, Czechoslovakia and Yugoslavia have no ISO 3166-1
  code and are where much of cinema came from. `media_items.countries` is left as it was.
- **An honest "never".** "You have never watched anything from Iran" may be said only when every title the profile has
  met was asked. Until then country and movement are *unknown* axes for that profile and no arm claims anything on them
  (`Profile::unknown`, `strategy::holds`). Met titles are asked first for exactly this reason.
- **Settings** lists the source with the others (ADR-0054 step 7): titles asked, with a country, with a movement.
- **`ingest regions`** runs the same step to the end on a data folder, for a development copy.

## Consequences

- The blind spot and the stretch gain the two axes that make them interesting; "Iranian New Wave" becomes a label the
  app can truthfully show. Movements are sparse (24 of the 994 most voted), which is right: Wikidata tags a film with a
  movement when the movement is the point of it.
- One more outbound request type, to a service the app already uses, carrying only IMDb ids. The ids of titles a
  profile met are among them (usually beside the most-voted titles, sometimes alone once those are all asked), which is
  the kind of request ADR-0054's live neighbours already make to AniList and TMDB for what a profile loved.
- A profile whose history includes a title with no IMDb id (an anime known only to AniList) never has its countries
  known, so it never gets a country blind spot. Strict, and honest; the decade and genre blind spots still work.
- Rejected: **TMDB's production countries** (needs a key the author does not have, ADR-0013); **IMDb's `title.akas`
  regions** (where a title was *released*, not where it was made: every Hollywood film has dozens).
