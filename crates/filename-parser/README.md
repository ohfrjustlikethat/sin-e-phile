# filename-parser

Part of [sin-e-phile](https://github.com/ohfrjustlikethat/sin-e-phile), but
**independently useful and independently licensed** (ADR-0017).

Extracts structured metadata from real-world media filenames: title, year,
season, episode, absolute episode number, resolution, source, codec, audio,
language and release group. Handles scene releases, anime fansub conventions
and multi-episode files.

Built in Phase 12. Pure string handling, no I/O.

## Licence

Dual-licensed under **MIT OR Apache-2.0**, at your option — the Rust convention.

The parent application is GPL-3.0, because it links libmpv and FFmpeg. **This crate
links neither**, so it carries the permissive licence that makes it actually usable
by the ecosystem. See [ADR-0017](../../docs/adr/0017-dual-license-extracted-crates.md).
