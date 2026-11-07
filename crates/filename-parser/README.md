# filename-parser

Part of [sin-e-phile](https://github.com/ohfrjustlikethat/sin-e-phile), but
**independently useful and independently licensed** (ADR-0017).

Reads what a media filename says about itself: title, year, season, episodes, an
anime-style episode number with no season, a daily show's date, the episode's own title,
resolution, source, video codec, audio codec and channels, declared languages, edition,
release flags, and the release group. It handles scene names, plain names, anime fansub
names and multi-episode files.

Pure string handling: no I/O, no catalogue, no network. It reports what a name *claims*.
Deciding what a file *is* needs evidence from the file and a catalogue, which is the job
of whatever uses this crate.

```rust
use filename_parser::{parse, Codec, Source};

let p = parse("The.Grand.Budapest.Hotel.2014.1080p.BluRay.x264-GROUP.mkv");
assert_eq!(p.title, "The Grand Budapest Hotel");
assert_eq!(p.year, Some(2014));
assert_eq!(p.resolution, Some(1080));
assert_eq!(p.source, Some(Source::BluRay));
assert_eq!(p.codec, Some(Codec::H264));
assert_eq!(p.group.as_deref(), Some("GROUP"));

let e = parse("Show.Name.S02E01E02.720p.HDTV.x264-GROUP.mkv");
assert_eq!((e.season, e.episodes.clone()), (Some(2), vec![1, 2]));

let a = parse("[Some Subs] Show Name - 059 [1080p].mkv");
assert_eq!(a.absolute_episodes, vec![59]); // no season given: see the field's docs
assert_eq!(a.group.as_deref(), Some("Some Subs"));

// A year inside a title is title: "Wonder Woman 1984" was released in 2020.
let w = parse("Wonder.Woman.1984.2020.2160p.WEB-DL.mkv");
assert_eq!((w.title.as_str(), w.year), ("Wonder Woman 1984", Some(2020)));
```

Try a name from the command line:

```text
cargo run -p filename-parser --example parse -- "Some.Film.2010.720p.WEB-DL.mkv"
```

## How it decides

- Every tag is found by pattern and kept only with a separator (or an end) on both sides,
  so `x264` counts and `fox264` does not. Overlapping finds keep the earliest, then the
  longest (`WEB-DL` over `WEB`).
- The title is the text before the first *strong* tag (episode, year, resolution, source,
  codec). *Weak* tags (edition, language, audio) only count after the title, so
  *The French Connection* keeps its "French".
- Of several year-shaped numbers, a bracketed one wins, then the last before the other
  tags; one that opens the name is never the year (*1917*, *2001: A Space Odyssey*).
- Sources that are also ordinary words ("Web", "Cam", "TS") never end a title
  (*Charlotte's Web*).
- A release group is found by where it sits (a leading `[Group]`, a trailing `-Group`),
  never by a list of names.

## Measured

On `fixtures/filenames/` in the parent repository: 549 hand-labelled names it was tuned
on, and 109 written afterwards and never tuned on, each field compared with what a
careful human reads (`tests/corpus.rs`). Numbers are in the parent repository's
`docs/eval-results.md`.

## Licence

Dual-licensed under **MIT OR Apache-2.0**, at your option — the Rust convention.

The parent application is GPL-3.0, because it links libmpv and FFmpeg. **This crate
links neither**, so it carries the permissive licence that makes it actually usable
by the ecosystem. See [ADR-0017](../../docs/adr/0017-dual-license-extracted-crates.md).
