# Phase 10 — Learning notes

**Subtitles that are in step on the first frame, or the honest reason they are not.** Four
sections per ADR-0016 (A4).

---

## 1. What we built

- **The aligner** — `crates/subtitle-align/` (its own MIT/Apache crate): reads and rewrites
  SRT, WebVTT and ASS timings in place; hears speech in audio; finds the offset, the
  framerate scale and an intro split that put the subtitles over the speech; refuses when
  unsure.
- **A corpus that measures it** — `fixtures/subtitles/`, built by `tools/subcorpus/`: ten
  short films of synthesised dialogue, eighty damaged subtitle files, wrong-film controls,
  four films held out from tuning. `cargo run --release -p eval -- subs`: 80/80, 0 false
  accepts.
- **The chain** — `crates/subtitles/`: embedded tracks, files beside the film, then
  OpenSubtitles with the user's own key and a daily budget that cannot be overspent.
- **In the app** — subtitles chosen, aligned and loaded once the first frame is up;
  G/H and the bumpers nudge them, remembered per file; Settings → Subtitles.

## 2. Why

- **Scale and offset together.** A file for 23.976 fps played at 25 drifts 4%: a
  multiplication. An offset fixes one moment of it, never the film.
- **Refuse rather than guess.** A wrong shift makes good subtitles bad; the threshold is
  set from wrong-film controls, not chosen.
- **Measured on files held back from tuning.** A number tuned on its own test set
  flatters; the held-out films are the honest one.
- **Embedded first, network last.** Embedded tracks are in step by construction and free;
  the free download quota is a handful a day (§14).
- **The chain owns sidecars.** mpv loaded a subtitle beside the file by itself, unaligned,
  and it looked embedded; `sub-auto=no` makes the chain the only door.

## 3. New concepts

- **Cross-correlation by FFT** — `crates/subtitle-align/src/align.rs` `correlation`: every
  shift at once as IFFT(A·conj(B)), O(n log n) instead of O(n²).
- **A plateau, and which edge is true** — `align.rs` `peak`: subtitles lead a little and
  linger, so the top is flat; the truth is its late edge.
- **Distinctness as confidence** — `align.rs` `distinctness`: the winner over the best
  rival 3 s away; chance gives ≈1.
- **Monotonic time as a constraint** — `align.rs` `best_fit` (`in_order`): a split's parts
  cannot overlap, which rules out throwing a short run of cues minutes away.
- **Biquad filters** — `crates/subtitle-align/src/vad.rs` `Biquad`: two poles and two
  zeros, the RBJ cookbook's band-pass.
- **Modulation as a speech cue** — `vad.rs` `local_spread`: speech rises and falls with
  syllables; music holds.
- **A conditional UPDATE as a lock** — `crates/persistence/src/repositories/subtitles.rs`
  `reserve`: `used = used + 1 WHERE used < budget` cannot overspend, even twice at once.
- **The OpenSubtitles hash** — `crates/subtitles/src/hash.rs`: size plus the first and last
  64 KiB as wrapping u64 sums; identifies bytes, so a nudge follows a renamed file.
- **Inner doc attribute from a file** — `crates/subtitle-align/src/lib.rs`
  `#![doc = include_str!("../README.md")]`: the README's example runs as a test.
- **Per-package optimisation in dev** — `Cargo.toml` `[profile.dev.package.subtitle-align]`:
  FFT tests went from 117 s to 7 s without slowing other builds.

## 4. The questions

Written, not asked — Phase 10 is not a tier boundary (§10.10).

1. Why can no constant offset fix subtitles timed for 23.976 fps on a 25 fps film, and
   how much are they off after an hour?
2. The correlation's top is flat. Why is the truth at its late edge and not its middle?
3. Why is confidence "winner over the best rival 3 s away" rather than a z-score, and
   where did 1.45 come from?
4. Why does reserving a download before asking for it, in one SQL statement, mean the
   quota is never exceeded even if two films open at once?
5. What did "held out" protect against, and what would 100% on the tuned films alone
   have hidden?
