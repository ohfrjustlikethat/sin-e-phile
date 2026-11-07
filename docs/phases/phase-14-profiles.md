# Phase 14 — Profiles and First-Run Onboarding

**Status:** in_progress · **Depends on:** 3, 5 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A three-screen setup that a non-technical person completes in under two minutes, and that gives the recommender everything it needs.

## Deliverables

Profile system: creation, avatar, optional PIN, Netflix-style picker on launch **only when there are two or more profiles** (otherwise straight to Home, ADR-0043), fast switching. Full isolation of taste model, history, watchlist, continue-watching, and playback preferences; shared downloaded files and metadata cache. A kids profile with content-rating limits. **The onboarding wizard**, three steps: (1) create profile; (2) taste — **defaulting to the three methods that need no artwork** (ADR-0013), since the app is fully functional with no TMDB key and a poster grid needs posters: import from Letterboxd CSV, Trakt, IMDb ratings export, or MAL/AniList XML; taste statements about pace, ambiguity, era, subtitles, runtime, and formal experimentation — *not* genre checkboxes; and a free-text "describe what you love" box embedded directly into the taste vector. The **adaptive poster grid** (3–4 rounds, each adapted to previous picks, choices spread to maximise information gain) appears as an additional round **only once artwork is available**. Where artwork is absent the equivalent surface is a **typographic title-card grid** — title, year, director, set in the §9.2 display serif — which for a cinephile app is arguably the better design anyway: it selects on knowledge and taste rather than poster recognition. Offering the optional ~30-second TMDB step sits here, framed as *unlock artwork and rich detail*, with a "later" that is not a dead end; (3) sources — optional, one paste-a-URL field with plain-language help and a "later" option that is not a dead end. **The app must be fully usable if step 3 is skipped**, via local files and the Internet Archive backend. Add-a-local-folder is offered inline in step 3 as the zero-configuration path.

## Exit criteria

- [ ] **E1** A first-time user with no technical knowledge reaches a populated Home screen in under two minutes. Test this on an actual person.
- [ ] **E2** A Letterboxd export of 500+ films imports correctly and is reflected in the taste model.
- [ ] **E3** Skipping the sources step leaves a functional, non-empty app.
- [ ] **E4** Completing onboarding with no TMDB key produces a working, populated, good-looking app (ADR-0013). Every artwork-bearing surface has a designed artwork-free state — a typographic fallback, never a grey rectangle.
- [ ] **E5** Profile switching is instant and leaks no state between profiles.
- [ ] **E6** Every onboarding screen is beautiful — this is the first impression.

## Subtasks

- [x] **14.1** Profiles in the backend: migration 0024 (avatar colour, PIN salt and hash, kids flag and limit, last used); ProfileRepository create/rename/delete (never the last)/set_pin/verify_pin/set_kids/touch, each with a line in repository_surface.rs; an active profile in AppState replacing every ensure_default() call in src-tauri; a crate test that watchlist, positions, ratings, watch events, settings and keys never cross profiles (E5).
- [x] **14.2** Profiles in the app: the picker on launch only with two or more (ADR-0043), the avatar in the masthead with a switcher, create and edit with an optional PIN, the kids profile failing closed (TMDB certification within the limit when the profile has a key, otherwise Family or Animation titles that are not adult nor tagged with an adult genre); switching timed to a re-rendered Home (E5); pad-complete in tools/uiaudit/controller.mjs. The frontend's personal browser storage (playback choice, recent searches) is scoped by profile and reloaded on a switch; machine settings (controller, last destination) stay shared.
- [x] **14.3** Imports (crates/imports): Letterboxd CSV (watched, ratings, diary, watchlist), Trakt JSON, IMDb ratings CSV, MAL and AniList XML, each matched to the catalogue (an IMDb id where the file gives one, else title and year through the library's identifier), recorded per profile as ratings, completed watches and watchlist, unmatched rows listed in words; a 500+ entry Letterboxd fixture built from catalogue titles, match rate recorded (E2).
- [ ] **14.4** Taste input: the six taste statements (pace, ambiguity, era, subtitles, runtime, formal experimentation) stored per profile; the free-text 'what you love' embedded with the query embedder and stored; a seed taste vector from imports and statements, so E2's 'reflected in the taste model' is measurable now (nearest titles to the vector against the imported favourites) and Phase 15 builds on it.
- [ ] **14.5** The onboarding wizard: profile, taste (imports, statements, free text, and the adaptive typographic title-card grid as an extra round: 3-4 rounds, picks spread for information gain), sources (paste a URL, the Internet Archive, add a folder inline), each skippable and never a dead end; shown on first run only; E3 and E4 tested with no key and no sources; pad-complete.
- [ ] **14.6** Author checks batched after Phase 15 (E1 a first-time user timed to a populated Home; E6 sign-off on the onboarding screens) with the earlier ones; docs/specs/profiles.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

Cold-start in recommender systems; why an import beats a questionnaire; information gain in adaptive questioning; why taste statements beat genre checkboxes.

---

<!-- subtask log: appended during the phase -->

## Work log
- **14.1** Profiles in the backend: migration 0024 (avatar colour, PIN salt and hash, kids flag and limit,  · `f671bc2`
- **14.2** Profiles in the app: the picker on launch only with two or more (ADR-0043), the avatar in the ma · `272b54f`
- **14.3** Imports (crates/imports): Letterboxd CSV (watched, ratings, diary, watchlist), Trakt JSON, IMDb  · `cace2eb`
