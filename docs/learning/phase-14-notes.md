# Phase 14 — Learning notes

**Everyone in the house gets their own profile; a first run asks three questions; and a history
from Letterboxd, Trakt, IMDb or MyAnimeList comes across in seconds.** Four sections per ADR-0016 (A4).

---

## 1. What we built

- **Profiles** — names, colours, PINs, kids profiles; a picker when there are two or more; switching in
  about 11 ms (112 ms the first time after launch, from disk); nothing one profile does is visible to
  another, tested table by table.
- **Imports** — `crates/imports` reads four services' exports and places each title on the catalogue:
  98.83% right and none wrong on 600 films chosen after the last change.
- **Taste input** — six statements that are not genres, "what you love" in your own words, and a seed
  taste vector built from what you liked.
- **First run** — three steps, each skippable: who, what you like (title cards across a century,
  statements, or an import), where to watch.
- **Home, first version** — rails that are never empty, typographic cards with no artwork needed.
- **Kids profiles** that fail closed, built on a category the free data actually has.
- **Two navigation fixes** a pad user would have hit on any long page or any row of buttons.

## 2. Why

- **Fail closed for children.** With no certifications, the only safe rule is "shown when positively
  known to suit them". IMDb's genres cannot tell Toy Story from Sausage Party (identical); MovieLens's
  hand-filed "Children" can. Choosing the data that separates the cases beat tuning a rule that cannot.
- **Ask rather than guess.** A stranger's film in your history is worse than one question. So two titles
  of one name and year are only told apart when one is plainly the one people mean (ten times the votes).
- **Judge on fresh data.** The first run of the import harness found a wrong match; the rule that fixed
  it was judged on 600 films it had never seen.
- **Measure what you are told, and do not move the bar.** The taste seed reflects a history (27 times
  chance) but loses to "show the most popular". That bar was set before running, so the claim is carried
  to Phase 15 rather than redefined.
- **Data that may not be shared is built at run time.** The harnesses make their exports from the local
  MovieLens files; nothing from them is committed.
- **A PIN is a household lock**, wrapped like a key, and the notes say what it does not protect against.
- **Look at the real thing.** The title cards were correct in every test and took seconds to appear on
  the real catalogue; an index made it 0.067 s.

## 3. New concepts

- **ON DELETE CASCADE as the isolation guarantee** — `crates/persistence/migrations/0004_user_and_config.up.sql:29`;
  tested in `crates/persistence/tests/profiles.rs`.
- **Never delete the last, hand on the default, in one transaction** — `crates/persistence/src/repositories/profiles.rs:144` `delete`.
- **Idempotent import by a natural key** — `profiles.rs:278` `record_watch_at` (INSERT … WHERE NOT EXISTS).
- **A constant-time comparison** — `crates/persistence/src/repositories/credentials.rs:315` `verify_pin`.
- **Weighted signals into one list** — `profiles.rs:381` `liked` (UNION ALL of ratings and watches).
- **Failing closed in SQL** — `crates/persistence/src/repositories/media.rs:296` `suitable_for_children`.
- **A hand-written CSV reader (RFC 4180)** — `crates/imports/src/csv.rs:6` `records`.
- **Streaming XML with entities** — `crates/imports/src/mal.rs` `read` (quick-xml events; collect, then assign at the end tag).
- **Decisions as a pure function** — `crates/imports/src/lib.rs:212` `choose`; normalising names, `:188` `normalised`.
- **A centroid in embedding space** — `crates/taste/src/lib.rs:74` `mean` (normalise, weight, renormalise).
- **Interleaving for spread** — `crates/taste/src/rounds.rs:25` `interleave`.
- **Steering SQLite's planner** — `media.rs` `most_voted_films_between`: `+column` keeps a column out of the index choice.
- **A partial index** — `crates/persistence/migrations/0025_vote_indexes.up.sql`.
- **Per-user state in a shared store** — `src-tauri/src/state.rs:132` `AppState::profile`.
- **Scoped browser storage, and why reset-then-read lost data** — `src/lib/profileStorage.ts:67` `switchStores`.
- **A beam in spatial navigation** — `src/lib/gamepad.ts:169`; **regions first** — `gamepad.ts:217` `nearestInRegion`.
- **A guarded event listener** — `src/lib/events.ts:11` `onEvent`.

## 4. The questions

Written, not asked — Phase 14 is not a tier boundary (§10.10).

1. A kids profile cannot use certifications here. What does it use instead, why is "Children or Family,
   and none of six genres" safer than "Animation", and what does a child lose by it?
2. "300" (2007 in one source, 2006 in the other) was once placed on the wrong film. What rule fixed it,
   and why was that rule judged on a different set of films from the one that found the bug?
3. Why does importing the same Letterboxd file twice add no watches, and which column makes that so?
4. The taste seed is 27 times better than chance and worse than "most popular". What is it a mean of,
   why does a mean blur a person who likes two very different things, and what does Phase 15 do instead?
5. Switching profiles takes 11 ms but 112 ms the first time after launch. Where does each go?
