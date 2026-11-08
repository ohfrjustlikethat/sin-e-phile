# The local library (Phase 12)

Point the app at a folder: it finds the films and episodes, says what each one is, asks only
when it is not sure, and keeps up as files are added, renamed and removed. Measured numbers
are in `docs/eval-results.md` (Phase 12).

## 1. Reading a name — `crates/filename-parser`

MIT/Apache, no I/O. `parse(name)` reads title, year, season, episodes, an episode number
with no season, a daily show's date, the episode's title, resolution, source, codec, audio,
languages, edition, flags and the release group.

- Tags are found by pattern and kept only with a separator or an end on both sides;
  overlaps keep the earliest, then the longest.
- The title is the text before the first **strong** tag (episode, year, resolution, source,
  codec). **Weak** tags (edition, language, audio) count only after it, and trailing ones are
  trimmed from it unless a year ends the title.
- The year: never one that opens the name; a bracketed one first; else the last before the
  other strong tags. A source that is also a word ("Web", "Cam", "TS") never ends a title.
- A release group is found by position (`[Group]` first, `-Group` last, ` - Group` after a
  technical tag), never by name. The fixtures' groups are invented (§2.1).

Corpus: `fixtures/filenames/names.psv` (549, tuned on) and `heldout.psv` (109, never tuned
on); `tests/corpus.rs` holds every field to 97% and whole names to 95% (90% held out).

## 2. Saying what a file is — `crates/library/src/identify.rs`

`read(evidence)` turns the name and folders into a query: the name's title, or the
nearest naming folder's when the name says nothing ("S01E02.mkv" in "Breaking Bad (2008)/
Season 1"); the folder's year and season fill gaps.

Candidates come from the catalogue's exact title (any variant), its word index, and, only
when nothing close was found, its trigram index (100× the cost). A trailing country ("The
Office (UK)") is searched without it. Individual episodes are never candidates; a series is,
and its episode is found afterwards.

Each candidate is scored on independent signals in [0, 1] (0.5 is "no evidence"):

| Signal | Weight | Reading |
|---|---|---|
| title | 0.40 | word overlap with the best of its titles, 1 when equal |
| year | 0.22 | 1 equal, 0.6 one apart, 0 otherwise |
| kind | 0.18 | film or series against how much the name says "episode" |
| runtime | 0.12 | the probed duration against the catalogue's (loosely for a series) |
| popularity | 0.08 | log of votes |

A series that has the claimed episode gains 0.08; one that lacks it is scaled by 0.92.
Fansub naming makes a live-action namesake less likely (×0.6 on kind).

**Confidence** = lead × plausibility: the lead is (best − next) / 0.12, capped at 1; the
plausibility is (best − 0.50) / 0.30, capped at 1. A runaway winner that barely fits is not
confident, and neither is a perfect fit with a twin. **Accepted at 0.75**
(`ACCEPT_AT`); below, the file is asked about. A remembered answer is confidence 1.

## 3. Scanning and watching — `scan.rs`, `watch.rs`

- **Walk** (no probing, no hashing): video extensions, release samples (the word "sample"
  and under 300 MB) and Windows' hidden and system folders skipped, links not followed.
- **Reconcile**: same path, size and time is unchanged; a vanished file is marked missing;
  a new path with a missing file's size **and** modification time is that file moved, and
  keeps its row and match. Still missing at the end: removed.
- **Identify** what is new or changed, 7 at a time, a series' searches shared; FFprobe (no
  console window) runs only for a file the name did not settle.
- **Watch**: `notify` (ReadDirectoryChangesW). A path's events wait until it has been quiet
  for 2 s, removals before arrivals, so a rename delivered as remove + create is a move.
- Paths are stored in one spelling, without Windows' `\\?\` prefix.
- At launch every folder is rescanned, catching what changed while the app was closed.

## 4. Asking — `review.rs`

The queue is every present, un-ignored file matched below 0.75 and unconfirmed, in path
order. An answer is a confirmed manual match, **remembered under the name's pattern**
(`comparable title|year|movie-or-series`), and every other waiting file with the same
pattern is identified again at once: one answer settles a series. "Not a film or episode"
sets `ignored_at` (migration 0022) and survives rescans. The screen: the file and how it was
read on the left, typographic candidate cards on the right; 1–5 / A choose, S / Y search, N /
X not a film, L / RB later, Esc / B back.

## 5. Playing — `LocalFileBackend`

The local source turns a query back into its catalogue item (IMDb id, else title, year and
kind; an episode through its series' season and episode) and offers the library's files
matched to it that are still on disk, with resolution, codec and audio languages from the
probe. Phase 8's "search, click and watch a local file" is this path.

## 6. Schema

Migration 0021: `library_folders` (the user's own paths, allowed by the portability test
with `local_files.path`), `local_files.folder_id` (a trigger, not a foreign key, so the down
migration can drop it), `missing_since`, probe columns, `local_file_matches.covers`,
`review_candidates`, `match_memory`. Migration 0022: `local_files.ignored_at`.

## TV layouts and extras (Phase 16)

A release's extras are not library items. A file under an extras folder (Featurettes, Extras, Behind the
Scenes, Deleted Scenes, Interviews, Trailers, Menu Art, Bonus, Special Features …) or named as one (gag reel,
NCOP, NCED, creditless, featurette) is left out by `scan::walk`, so it is never searched for as a film
(`folders::is_extra`). The show is named by its nearest naming folder, the season by a `Season N` folder, the
episode by the file. A fansub absolute number is placed as season 1's episode only when the chosen series has
exactly one season, and never counts while scoring candidates. Measured by `eval tvlayouts` over
`fixtures/tv-layouts/` (E4); a real folder can be checked read-only with `eval tvlayouts --folder <path>`.

