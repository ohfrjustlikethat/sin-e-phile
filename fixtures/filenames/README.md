# fixtures/filenames — the identification corpus (Phase 12)

Hand-labelled filenames: what each name says, and what the file truly is. Two harnesses
read it:

- `crates/filename-parser/tests/corpus.rs` — per-field parsing accuracy (Phase 12 E6).
- `cargo run --release -p eval -- identify` — top-1 identification against the real
  catalogue and the false-confident rate (Phase 12 E1, E2).

## Format

`names.psv`, one name per line, fields separated by `|` (a character Windows forbids in
filenames, so no name can contain it). Spaces around a field are trimmed. `#` starts a
comment line. An empty field means "not in the name".

```
name | title | year | season | episodes | absolute | resolution | source | codec | group | id_kind | id_title | id_year | folder | minutes
```

- `title` … `group`: what a careful human reads in the name. `episodes` and `absolute`
  are comma-separated (`1,2,3`). `source` and `codec` use the parser's names (`BluRay`,
  `WEB-DL`, `H.264` …).
- `id_kind`, `id_title`, `id_year`: what the file IS — `movie` or `series`, the
  catalogue's title and year (a series' first year). Empty means "the same as the
  parsed title and year", with the kind following from whether the name is an episode.
  `?` in `id_kind` marks a name that cannot honestly be identified from what is given;
  the right answer for it is the review queue, not a guess.
- `folder`, `minutes`: evidence beside the name — the folders it sits in, and the
  file's real duration. Empty when the row is about the name alone.

## Rules

- **Every release group is invented.** Real group names would name content sources by
  the back door (SPEC.md §2.1). The parser recognises a group by where it sits, never by
  its name, so an invented name tests exactly the same thing.
- **No site names, no addresses.** The site-prefix rows use RFC 2606 domains (ADR-0009).
- Titles and years are of real films and series: they are catalogue facts, the same
  facts the IMDb datasets publish.
