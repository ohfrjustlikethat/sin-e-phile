# subtitle-align

Part of [sin-e-phile](https://github.com/ohfrjustlikethat/sin-e-phile), but
**independently useful and independently licensed** (ADR-0017).

Aligns a subtitle track to audio by solving for **offset and framerate scale**
together via cross-correlation — because a 23.976 vs 25 fps mismatch is a
multiplication, not an addition, and no constant offset can fix a drift.

Scores its own confidence and refuses to apply a low-confidence alignment.

Built in Phase 10.

## Licence

Dual-licensed under **MIT OR Apache-2.0**, at your option — the Rust convention.

The parent application is GPL-3.0, because it links libmpv and FFmpeg. **This crate
links neither**, so it carries the permissive licence that makes it actually usable
by the ecosystem. See [ADR-0017](../../docs/adr/0017-dual-license-extracted-crates.md).

## Design constraint

**This crate must never depend on FFmpeg** (ADR-0017). It takes **PCM samples or a
precomputed VAD signal** as input; audio extraction lives in the application.

That keeps the licence clean, and it makes the crate testable with synthetic
fixtures — no subprocess, no temp files, no FFmpeg on the test machine.
