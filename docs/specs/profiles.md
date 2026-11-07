# Profiles, imports, taste input and first run (Phase 14)

Measured numbers are in `docs/eval-results.md` (Phase 14).

## 1. Profiles — `crates/persistence` (migration 0024), `src-tauri/src/commands/profiles.rs`

A profile has a name, an avatar colour (six token grounds, `--avatar-*`), a kids flag, a last-used
time, and optionally a PIN. Everything personal is keyed on it and cascades on delete: watch events,
positions, watchlist, ratings, profile settings (keys, the qBittorrent login, taste input).

- **The active profile** is `AppState::profile()`: the one switched to, else the default. Every command
  that reads or writes something personal asks it; nothing calls `ensure_default` any more.
- **Never the last.** `delete` refuses the last profile; deleting the default hands it to the most
  recently used; deleting the one watching switches to the default.
- **PINs** are four digits, kept wrapped (DPAPI) in `profile_settings` like the keys. A PIN keeps a child
  out of a parent's profile; it is not security against the machine's owner and is not claimed to be.
  One that will not unwrap (the folder moved to another Windows account) opens, and is set again.
- **The picker** opens on launch only with two or more profiles (ADR-0043), and from the masthead avatar.
- **Browser storage that is personal** (the playback choice, recent searches) is kept per profile
  (`src/lib/profileStorage.ts`); the first profile to load claims what was saved before profiles.
- **Kids profiles fail closed** (`MediaRepository::suitable_for_children`): only titles filed as
  "Children" (MovieLens, ingested as a genre by `ingest movielens`) or "Family" (IMDb), not adult, and in
  none of Adult, Horror, Thriller, Crime, War, Film-Noir. Applied to search, the detail page, play, Home
  and the onboarding rounds. Certifications (`kids_limit`) wait for TMDB release dates (D81).

## 2. Imports — `crates/imports`

`read(path)` knows the export by its contents: a Letterboxd `.zip` (or one of its CSVs), Trakt JSON (or a
`.zip` of them), IMDb's ratings CSV, MyAnimeList XML (which AniList also writes). Each becomes `Entry`s:
title, year, an IMDb or MAL id where given, a rating out of ten, dated watches, watchlist.

`place(entry)` matches, never guesses:
1. the IMDb or MAL id;
2. the exact name (any variant), then by year: the exact year unless a title a year either side has a
   hundred times its votes; two in one year only when one has ten times the next's votes; else a year
   either side on the same terms;
3. when the exact name finds nothing, the name normalised ("&"/"and", punctuation, a leading "the") among
   films and series a year either side, on the same terms.
What is not placed is listed, with why.

`import` records ratings (halves round up to the app's 1–5 stars), every dated watch
(`record_watch_at`, once per moment, so a second import adds nothing), and the watchlist.

## 3. Taste input — `crates/taste`

- **Statements**: six axes, each -1/0/1 or unanswered: pace, endings, era, subtitles, length, form. Not
  genres. Stored per profile (`taste.statements`); Phase 15 uses them.
- **What you love**: free text (`taste.love`), embedded with the query embedder.
- **The seed**: the unit-length weighted mean of the liked titles' stored vectors (five stars 1.0, four
  0.7, completed and unrated 0.3; `ProfileRepository::liked`), with the free text counting as three
  five-star films. `nearest` finds the titles closest to it. Phase 15 replaces the one mean with modes.

## 4. First run and Home

- **The wizard** (`src/features/onboarding/Onboarding.tsx`) on first run only (`onboarding.done`): who
  this is for; what you like (title cards, statements and words, or an import); where to watch (a folder,
  a source address; optional). Every step can be skipped; none is a dead end. No launch intro.
- **Title cards** (`rounds.rs`): a round is the most-voted films decade by decade, interleaved newest
  first, so twelve cards span a century; after a pick, half the next round is nearest the taste so far.
  A pick is a four-star "liked".
- **Home, first version** (`home.rs`): Continue watching, Where your taste points, On your watchlist,
  Widely loved. The last needs only the offline catalogue, so Home is never empty. Typographic title cards
  (`PosterCard`'s designed artwork-free state). Phase 18 overhauls Home.
- **Speed**: migration 0025 indexes votes; the decade query walks it in vote order.

## 5. Navigation, two fixes found here

`src/lib/gamepad.ts`: moving within a region (the page, the masthead, a dialog) before leaving it, so a
long page can be climbed; and the beam, so Right along a row reaches the row's next button before one
above or below.
