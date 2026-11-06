# subtitle-align

Part of [sin-e-phile](https://github.com/ohfrjustlikethat/sin-e-phile), but
**independently useful and independently licensed** (ADR-0017).

Aligns a subtitle file to a film's speech by solving for **offset and framerate scale
together** — because a 23.976 vs 25 fps mismatch is a multiplication, not an addition,
and no constant offset can fix a drift — and for an intro added or cut part-way through.
It scores its own confidence and **refuses** to apply an alignment it is not sure of: a
wrong shift is worse than none.

## Use

```rust
use subtitle_align::{align_text, vad, Options};

// Mono PCM of the film's audio, from wherever you get it (this crate never runs FFmpeg).
let samples: Vec<f32> = vec![0.0; 16_000 * 60];
let speech = vad::speech(&samples, 16_000); // one bool per 10 ms

let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";
match align_text(srt, &speech, &Options::default()) {
    Ok((fixed, a)) => println!("scale {} offset {} ms\n{fixed}", a.scale, a.offset_ms),
    Err(why) => println!("left as it was: {why}"), // here: no speech in a minute of silence
}
```

Three modules, each usable on its own:

| Module | Does |
|---|---|
| `cue` | Reads timings from SRT, WebVTT and ASS/SSA and rewrites them **in place**: styling, override tags and line endings come back byte for byte. |
| `vad` | Which 10 ms frames of mono PCM are speech: band energy over a local noise floor, plus a syllable-modulation test that keeps music from reading as dialogue. Pure Rust, no model. |
| `align` | Offset, scale and an optional split by FFT cross-correlation, with a distinctness confidence. |

How it works, and why each choice: [`docs/specs/subtitle-alignment.md`](../../docs/specs/subtitle-alignment.md).

## Measured

On the project's corpus (`fixtures/subtitles/`, ten films, eighty damaged subtitle
files, `cargo run --release -p eval -- subs`): **98.8%** aligned to within 150 ms
(96.9% on films held out from tuning), **0** false accepts against the wrong film.

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
