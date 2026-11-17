# 0048 — Posters: a keyless image service, opted into, and TMDB with the profile's own key

- **Status:** Accepted 2026-11-17. The author chose it ("Artwork: both"), round 7 of the fix-list questions
  (`docs/design/author-brief-2026-11-17.md`).
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** ADR-0013 (artwork-free by default) and ADR-0027 (TMDB per profile, optional): neither is reversed.
- **Allowlist:** `metahub.space` added to `tools/guard/allowlist.txt` (ADR-0010).

## Context

A default install has no artwork: ADR-0013 made TMDB optional, and the designed typographic card is what most
people see. The author asked for posters anyway, "the poster images Stremio itself uses, looked up by IMDb id",
as an opt-in, plus full artwork once the profile's own TMDB key is added. The app's content security policy loads
no remote images (`img-src 'self' asset: data:`), and that stays: a page that fetched images from anywhere would
send every title it showed to whoever served them, unasked.

## Decision

- **Two sources, both by IMDb id, both chosen by the person.** With the profile's TMDB key, TMDB's image server
  (`image.tmdb.org`, already allowed) through TMDB's `find` endpoint. With "Show posters" switched on (onboarding
  and Settings → Library, off unless chosen), Stremio's poster images at
  `https://images.metahub.space/poster/medium/<imdb id>/img`. The switch says plainly that it sends the IMDb id of
  each title shown to that service, and nothing else.
- **The backend fetches, never the page** (`sinephile_artwork::posters`). Only an IMDb id (`tt` and digits) goes
  into an address; only an `image/*` answer under 4 MB is kept; each is re-encoded to WebP in the artwork cache
  (500 MB budget) and read from disk after that; a title with no poster is not asked again in the session; four
  fetches at most at once. The page receives a `data:` URL, which the policy already allows.
- **Lazily.** A card asks for its poster only when it is near the screen, and only if a source is on.
- **Without either source nothing changes**: no request is made, and the designed text card stays.

## Consequences

- `metahub.space` is a metadata image host, like `image.tmdb.org`: it serves posters, not films, and the app names
  it only as an image source. §2.1 is unchanged: the app still ships, suggests and names no content source.
- A person who switches posters on tells that service which titles they look at. The switch says so, it is off by
  default, and it is per profile.
- If the service changes its address scheme, posters quietly stop (the cards fall back to text) until the
  address is updated here; nothing breaks.
