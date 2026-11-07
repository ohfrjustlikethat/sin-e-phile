# Phase 12 — Learning notes

**Point it at a folder; it says what everything is, asks only when it is not sure, and keeps
up as files move.** Four sections per ADR-0016 (A4).

---

## 1. What we built

- **A filename parser** — `crates/filename-parser` (its own MIT/Apache crate): title, year,
  season, episodes, group and a dozen tags from scene, plain and fansub names. 549 labelled
  names it was tuned on and 109 it never saw, every field right.
- **Identification** — `crates/library/src/identify.rs`: the name, the folders and the
  file's length weighed against the 2.7M-title catalogue. Right first 99.05% of the time;
  0.39% of what it accepts without asking is wrong.
- **A scanner and a watcher** — 10,000 files in about 36 s with the UI unaffected; a file
  renamed or moved, while the app runs or while it is closed, keeps its row and its match.
- **A review queue** — one file at a time, typographic cards, one key or button; one answer
  settles every file named the same way and is remembered.
- **Local playback** — the local source answers from the library: search, click, play
  (388 ms to the first frame), discharging Phase 8's last criterion.

## 2. Why

- **Many weak signals beat one strong one.** A name says "Heat"; a year, a runtime and a
  series' episode list each rule out a different wrong answer. None is trusted alone.
- **Wrong is worse than unsure.** A confident wrong match hides a film; an unsure one costs a
  key press. So confidence measures the lead over the runner-up, and the threshold sits where
  fewer than 1% of acceptances are wrong, with a margin.
- **Measure the threshold, do not choose it.** The sweep found 0.615 at 0.97%: legal, but on
  the line. 0.75 measured 0.39%.
- **Find tags by where they sit, never by name.** A list of release-group names would name
  content sources (§2.1); position works for invented names exactly as for real ones.
- **No hashing in a scan.** Size plus modification time recognises a move and costs nothing;
  reading 10,000 files' contents would cost minutes.
- **Remember answers by what the name says**, not by file: the next episode of the same
  series is then settled before anyone sees it.
- **Check by hand what the tests cannot reach.** The real window found the `\\?\` path
  spelling and the walker that skipped films starting with "$"; unit tests passed for both.

## 3. New concepts

- **Pattern matching with explicit boundaries** — `crates/filename-parser/src/tags.rs:465`
  `find`: match anywhere, keep only with a separator either side, step one character on.
- **Choosing among year-shaped numbers** — `tags.rs:570` `choose_year`.
- **A continuation read before the boundary** — `tags.rs:541` `episode_tail` (S01E01E02).
- **Ambiguous words** — `tags.rs:135` `AMBIGUOUS_WORDS`: "Web" counts after a title, never
  ends one.
- **Normalised comparison** — `crates/filename-parser/src/lib.rs:370` `comparable`.
- **Word overlap (Dice)** — `crates/library/src/identify.rs:146` `title_similarity`.
- **Confidence as lead × plausibility** — `identify.rs:228` `confidence`.
- **Reading before searching** — `identify.rs:255` `read`, so the review queue can compute
  a pattern without asking the catalogue.
- **A shared cache behind a mutex** — `identify.rs:236` `Cache`; never held across an await.
- **Bounded concurrency** — `crates/library/src/scan.rs:362` `buffer_unordered(PARALLEL)`.
- **Blocking work off the async threads** — `scan.rs:277` `spawn_blocking` for FFprobe.
- **A function pointer as a seam** — `scan.rs:33` `Prober`: FFprobe in the app, a counter in
  tests, and `'static` so it can move to the blocking pool.
- **File attributes, not names** — `scan.rs:114` `skipped`.
- **Move detection by size and time** — `scan.rs:176` `record`.
- **Debouncing a burst of events** — `crates/library/src/watch.rs:73` `Quieten`, and
  `watch.rs:41` `order` (removals first).
- **Filesystem events to intents** — `watch.rs:51` `changes_from`.
- **An upsert that respects a human** — `crates/persistence/src/repositories/library.rs:455`:
  `ON CONFLICT … WHERE is_confirmed = 0 OR excluded.is_confirmed = 1`.
- **A trigger instead of a foreign key** — `crates/persistence/migrations/0021_library.up.sql:18`,
  so the down migration can drop the column.
- **COM from Rust** — `src-tauri/src/commands/library.rs:314` `picker`: `IFileOpenDialog` on
  its own single-threaded apartment.
- **No console window for a child process** — `crates/library/src/probe.rs:62` `no_window`
  (`CREATE_NO_WINDOW`).
- **Remembering a decision by pattern** — `crates/library/src/review.rs:125` `resolve`.

## 4. The questions

Written, not asked — Phase 12 is not a tier boundary (§10.10).

1. "Wonder.Woman.1984.2020.2160p" has two years. Which is the year, and what rule says so?
   What rule stops "1917.mkv" losing its title?
2. Confidence is the lead over the runner-up times how plausible the winner is. Why both?
   Give a file where each half alone would accept something wrong.
3. Why was the threshold set to 0.75 when 0.615 already met "under 1% wrong"?
4. A file is renamed while the app is closed. Without reading its contents, how does the
   next scan know it is the same file, and when could that go wrong?
5. You answer "Monster (2004, anime)" for one episode. Why are the other four settled at
   once, and why will episode 6, added next week, never be asked about?
