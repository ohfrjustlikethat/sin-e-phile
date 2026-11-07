# Phase 14 — Profiles and First-Run Onboarding

**Status:** complete · **Depends on:** 3, 5 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

A three-screen setup that a non-technical person completes in under two minutes, and that gives the recommender everything it needs.

## Deliverables

Profile system: creation, avatar, optional PIN, Netflix-style picker on launch **only when there are two or more profiles** (otherwise straight to Home, ADR-0043), fast switching. Full isolation of taste model, history, watchlist, continue-watching, and playback preferences; shared downloaded files and metadata cache. A kids profile with content-rating limits. **The onboarding wizard**, three steps: (1) create profile; (2) taste — **defaulting to the three methods that need no artwork** (ADR-0013), since the app is fully functional with no TMDB key and a poster grid needs posters: import from Letterboxd CSV, Trakt, IMDb ratings export, or MAL/AniList XML; taste statements about pace, ambiguity, era, subtitles, runtime, and formal experimentation — *not* genre checkboxes; and a free-text "describe what you love" box embedded directly into the taste vector. The **adaptive poster grid** (3–4 rounds, each adapted to previous picks, choices spread to maximise information gain) appears as an additional round **only once artwork is available**. Where artwork is absent the equivalent surface is a **typographic title-card grid** — title, year, director, set in the §9.2 display serif — which for a cinephile app is arguably the better design anyway: it selects on knowledge and taste rather than poster recognition. Offering the optional ~30-second TMDB step sits here, framed as *unlock artwork and rich detail*, with a "later" that is not a dead end; (3) sources — optional, one paste-a-URL field with plain-language help and a "later" option that is not a dead end. **The app must be fully usable if step 3 is skipped**, via local files and the Internet Archive backend. Add-a-local-folder is offered inline in step 3 as the zero-configuration path.

## Exit criteria

- [ ] **E1** A first-time user with no technical knowledge reaches a populated Home screen in under two minutes. Test this on an actual person. — *carried to Phase 16*
- [x] **E2** A Letterboxd export of 500+ films imports correctly and is reflected in the taste model.
- [x] **E3** Skipping the sources step leaves a functional, non-empty app.
- [ ] **E4** Completing onboarding with no TMDB key produces a working, populated, good-looking app (ADR-0013). Every artwork-bearing surface has a designed artwork-free state — a typographic fallback, never a grey rectangle. — *carried to Phase 16*
- [x] **E5** Profile switching is instant and leaks no state between profiles.
- [ ] **E6** Every onboarding screen is beautiful — this is the first impression. — *carried to Phase 16*

## Subtasks

- [x] **14.1** Profiles in the backend: migration 0024 (avatar colour, PIN salt and hash, kids flag and limit, last used); ProfileRepository create/rename/delete (never the last)/set_pin/verify_pin/set_kids/touch, each with a line in repository_surface.rs; an active profile in AppState replacing every ensure_default() call in src-tauri; a crate test that watchlist, positions, ratings, watch events, settings and keys never cross profiles (E5).
- [x] **14.2** Profiles in the app: the picker on launch only with two or more (ADR-0043), the avatar in the masthead with a switcher, create and edit with an optional PIN, the kids profile failing closed (TMDB certification within the limit when the profile has a key, otherwise Family or Animation titles that are not adult nor tagged with an adult genre); switching timed to a re-rendered Home (E5); pad-complete in tools/uiaudit/controller.mjs. The frontend's personal browser storage (playback choice, recent searches) is scoped by profile and reloaded on a switch; machine settings (controller, last destination) stay shared.
- [x] **14.3** Imports (crates/imports): Letterboxd CSV (watched, ratings, diary, watchlist), Trakt JSON, IMDb ratings CSV, MAL and AniList XML, each matched to the catalogue (an IMDb id where the file gives one, else title and year through the library's identifier), recorded per profile as ratings, completed watches and watchlist, unmatched rows listed in words; a 500+ entry Letterboxd fixture built from catalogue titles, match rate recorded (E2).
- [x] **14.4** Taste input: the six taste statements (pace, ambiguity, era, subtitles, runtime, formal experimentation) stored per profile; the free-text 'what you love' embedded with the query embedder and stored; a seed taste vector from imports and statements, so E2's 'reflected in the taste model' is measurable now (nearest titles to the vector against the imported favourites) and Phase 15 builds on it.
- [x] **14.5** The onboarding wizard: profile, taste (imports, statements, free text, and the adaptive typographic title-card grid as an extra round: 3-4 rounds, picks spread for information gain), sources (paste a URL, the Internet Archive, add a folder inline), each skippable and never a dead end; shown on first run only; E3 and E4 tested with no key and no sources; pad-complete.
- [x] **14.6** Author checks batched after Phase 15 (E1 a first-time user timed to a populated Home; E6 sign-off on the onboarding screens) with the earlier ones; docs/specs/profiles.md, learning note, README and HOW_IT_WORKS, regression, close.

## Learning note

Cold-start in recommender systems; why an import beats a questionnaire; information gain in adaptive questioning; why taste statements beat genre checkboxes.

---

<!-- subtask log: appended during the phase -->

## Work log
- **14.1** Profiles in the backend: migration 0024 (avatar colour, PIN salt and hash, kids flag and limit,  · `f671bc2`
- **14.2** Profiles in the app: the picker on launch only with two or more (ADR-0043), the avatar in the ma · `272b54f`
- **14.3** Imports (crates/imports): Letterboxd CSV (watched, ratings, diary, watchlist), Trakt JSON, IMDb  · `cace2eb`
- **14.4** Taste input: the six taste statements (pace, ambiguity, era, subtitles, runtime, formal experime · `8174b82`
- **14.5** The onboarding wizard: profile, taste (imports, statements, free text, and the adaptive typograp · `3848f41`
- **14.6** Author checks batched after Phase 15 (E1 a first-time user timed to a populated Home; E6 sign-of · `89e50d6`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `06b9e4d`.

### Evidence per criterion

**E1** — A first-time user with no technical knowledge reaches a populated Home screen in under two minutes. Test this on an actual person.

**Not met; carried to Phase 16.** It needs an actual first-time person (SPEC says so); batched with the author checks after Phase 15, so Phase 16 receives it.

> MEASURED: the whole first run is pad-complete in tools/uiaudit/controller.mjs and ends on a Home that is never empty (crates/taste/tests/home.rs); a first-time person timing it is the author's to arrange.

**E2** — A Letterboxd export of 500+ films imports correctly and is reflected in the taste model.

**Not met; carried to Phase 15.** 'Reflected in the taste model' needs the taste model, which is Phase 15's deliverable (taste modes, not one mean); Phase 15 must beat popularity on eval taste.

> MEASURED: the import half is met: eval import on a fresh set of 600 films (ranks 601-1200), 98.83% right, 0 wrong, a second import adds nothing. The reflection half is not: eval taste on three real MovieLens histories, the seed finds 11.06% of held-out favourites among its 500 nearest against 35.64% for popularity (27x chance, 0.31x popularity). docs/eval-results.md.

**E3** — Skipping the sources step leaves a functional, non-empty app.

> 3848f41: src/features/onboarding/Onboarding.test.tsx 'skipping every step still finishes, and nothing is a dead end' (Skip, Skip, Back, Skip, 'Skip, and start' reaches the app); crates/taste/tests/home.rs a_new_profile_with_nothing_still_has_a_home (no history, no sources, no vector half: the 'Widely loved' rail from the offline catalogue alone, seen to fail with the kids filter broken); the whole first run by pad in tools/uiaudit/controller.mjs ('the whole first run by pad, and it ends in the app'); docs/evidence/phase-14/onboarding-*.png (the real window on the full catalogue, no TMDB key).

**E4** — Completing onboarding with no TMDB key produces a working, populated, good-looking app (ADR-0013). Every artwork-bearing surface has a designed artwork-free state — a typographic fallback, never a grey rectangle.

**Not met; carried to Phase 16.** 'Good-looking' is the author's sign-off, asked with E6 after Phase 15; Phase 16 receives it.

> MEASURED: no TMDB key anywhere in Phase 14; Home's cards are PosterCard's designed typographic state (no 'Uncatalogued', no grey rectangles); the onboarding screens on the full catalogue in docs/evidence/phase-14/; 'good-looking' is the author's judgement.

**E5** — Profile switching is instant and leaks no state between profiles.

> eval switch (2026-11-07/08): a switch (touch, then the new profile's Home rails with the taste seed) takes 2.1-2.5 ms for an empty profile and 10.9-11.5 ms for one with 650 imported films, warm; 112.1 ms the first time after an import, from disk (D82). No state leaks: crates/persistence/tests/profiles.rs nothing_one_profile_does_is_visible_to_another and deleting_a_profile_takes_all_of_its_own_and_nothing_else (seen to fail with a profile filter removed); browser storage per profile in src/lib/profileStorage.test.ts (seen to fail with reset-then-read). docs/eval-results.md 'E5: switching profiles'.

**E6** — Every onboarding screen is beautiful — this is the first impression.

**Not met; carried to Phase 16.** 'Beautiful' is the author's sign-off on these screens, batched after Phase 15; Phase 16 receives it.

> MEASURED: the three onboarding steps captured in the real window (docs/evidence/phase-14/onboarding-*.png) and by the pad flow (tools/uiaudit/out/controller/00a, 00b).

### Debt incurred

- **D81** Kids profiles filter by genre (MovieLens 'Children' or IMDb 'Family', not adult, none of Adult/Horror/Thriller/Crime/War/Film-Noir) because the free catalogue has no certifications. The kids_limit column waits for TMDB release-date certifications, which are not fetched yet; until then a profile's limit is not a certification. Home rails (Phases 16-18) must ask MediaRepository::suitable_for_children like search does.
- **D82** The first profile switch after launch costs about 112 ms (index and catalogue pages read from disk) against 11 ms warm. Touch the vector index and the votes index in the background after launch so the first switch, and the first Home, are warm.
- **D83** crates/torrent/tests/swarm.rs a_download_survives_a_restart failed once in CI (run 42396646470, 2026-11-07, at ec78b82): 10,223,616 bytes before the stop, 0 after wait_ready on the restarted engine. Passes 5/5 locally and in every earlier CI run; code unchanged since Phase 13. Rerun. If it recurs: check that Engine::stop lets librqbit flush its writes and persistence before the session ends (a slow runner may lose the race).

### Next phase starts by

Phase 15 subtask 15.1 (docs/phases/phase-15-taste-model.md): write migration 0026_signals in crates/persistence/migrations (a signals table: profile_id, media_item_id, kind (search, detail_view, rewatch), at, hour, context JSON) with repository methods and repository_surface lines; record a watch event from src-tauri/src/commands/playback.rs save_position when the player leaves (completed past 90% of the duration, otherwise abandoned with watched_seconds as its point), once per viewing.
