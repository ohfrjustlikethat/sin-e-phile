# 0052 — A title's backdrop, from the poster sources the person chose

- **Status:** Accepted 2026-11-17. The author chose it ("Backdrop and poster"), round 9 of the fix-list questions
  (`docs/design/author-brief-2026-11-17.md`).
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** ADR-0048 (posters) and `SPEC.md` Appendix F, I80 (backdrops were Phase 19's, with a TMDB key only).
- **Allowlist:** unchanged. The keyless service is the one ADR-0048 already admits.

## Context

A title's page had empty space on its right where the author expected the poster, which every card already shows
once posters are switched on. Asked how it should look, the author chose a wide, faded still behind the top of the
page with the poster on the right. Appendix F had backdrops in Phase 19, from TMDB only.

## Decision

- **The same two sources as posters, under the same switch, both by IMDb id.** With the profile's TMDB key, the
  `backdrop_path` of TMDB's `find` answer at `w1280`. With "Show posters" on, the keyless service's wide still at
  `https://images.metahub.space/background/medium/<imdb id>/img` (1920×1080, about 350 KB). Nothing is asked when
  neither is on.
- **Fetched, kept and refused exactly as a poster is** (`sinephile_artwork::posters`, `Art::Backdrop`): only an
  IMDb id in the address, only an image under 4 MB, re-encoded to WebP in the artwork cache, a miss remembered for
  the session. The page gets a `data:` URL from the `backdrop` command, beside `poster`.
- **On the page** (`src/features/detail/DetailScreen.tsx`): the still at 45% behind the top 640 px, faded into the
  page at its top, bottom and behind the text on the left, which keeps the text legible (§9.0 bans shadow for depth,
  not a scrim for legibility). The poster, at 300 px, stands on the right. Without a poster, the lettered card the rails use. Both are hidden
  from assistive technology and from the D-pad: the heading names the title.

## Consequences

- A person with posters on now also sends the IMDb id of each title page they open to the service that serves the
  stills, which is the same service and the same id the cards already send. The switch's wording covers it ("the
  IMDb id of each title shown").
- Appendix F's I80 is half done in Phase 16: backdrops. Cast photos stay in Phase 19.
- A title with no still shows the page as before, with the poster on the right.
