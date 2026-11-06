# Subtitle alignment

**Status:** built in Phase 10 (subtask 10.1–10.2) · **Code:** `crates/subtitle-align/`
(MIT OR Apache-2.0, ADR-0017) · **Corpus:** `fixtures/subtitles/` · **Harness:**
`cargo run --release -p eval -- subs --report`

Subtitles found anywhere but inside the film are often out of step with it: shifted by a
few seconds, drifting because they were timed for a different framerate, or jumping after
an intro that one release has and another does not. The aligner works out the
correction from the film's audio and the subtitles' timings alone, or says it cannot.

## 1. The model

`new = scale × old + offset`, with optionally a second offset from one cue onwards.

- **Offset** is an addition: the whole file early or late.
- **Scale** is a multiplication. A file timed for 23.976 fps played against a 25 fps
  release (PAL speed-up) is 4.1% too long. It starts right and is two minutes late by the
  end of a film. No offset can fix that.
- **Split**: an intro added or cut moves everything after it by one more offset.

The scales tried are the conversions releases actually go through: 1, 25/23.976,
23.976/25, 25/24, 24/25, 24/23.976 and 23.976/24.

## 2. Speech (`vad.rs`)

Mono PCM in, one bool per 10 ms out. Pure Rust, no model to download:

1. Band-pass 250–3400 Hz (two RBJ biquads), where speech carries its energy.
2. Frame loudness in dB; the noise floor is the quietest nearby second (the 10th
   percentile of each second, then the minimum within ±3 s).
3. A frame is speech when it is **9 dB over the floor** and its loudness **moves** (a
   standard deviation over ±100 ms above 2.5 dB). Syllables rise and fall several times a
   second; a held note or a hum does not, so music does not read as dialogue.
4. Gaps under 200 ms are bridged (breaths), runs under 60 ms dropped (clicks).

## 3. Matching (`align.rs`)

- Speech becomes a zero-mean signal (speech positive, silence negative). A subtitle
  over silence costs as much as one over speech earns, so a shift cannot win just by
  overlapping more.
- For each scale, the subtitles become a 10 ms "a subtitle is showing" signal, and the
  **cross-correlation** with speech, at every shift up to ±10 minutes, comes from one
  FFT. The best per-subtitle-frame score picks the scale.
- **The top is a plateau.** A subtitle appears a little before its line and lingers
  after, so a few hundred milliseconds of shift keep the same speech under it. Taking the
  first maximum put answers up to 200 ms off; the **middle of the plateau** is used.
- **No stretch unless clearly better.** Over a few minutes a 0.1% stretch (24 to 23.976)
  fits noise slightly better than none; a stretched fit must beat the unstretched one by
  2% to be believed. A real conversion (4%, or seconds of drift over a feature) wins by
  far more.
- **Splits.** Candidate offsets come from the six tallest distinct peaks of the whole
  file's correlation, and from short runs of ~12 cues across the first 60% of the file
  (where an intro is), each aligned on its own: a short part's peak can be lower than
  the long part's side-lobes. Each cue's speech-under-it is scored at each candidate
  (prefix sums, so this is cheap), and every pair is tried at every cue. A split is kept
  only if:
  - it beats one offset by more than 10%,
  - each part has at least 8 cues,
  - **film time runs forward**: the first part ends before the second begins. Without
    this, a short run of cues could be thrown minutes away, to a talkier stretch where
    any subtitles find more speech under them, and called confident.
  - the larger part is confident on its own (§4). An intro's minute of dialogue is too
    little to stand out alone, and is accepted because moving it gains clearly.

## 4. Confidence, and refusing

**Distinctness**: the winning peak's height over the best rival's (at least 3 s away),
both measured from the average. Chance produces near-ties (≈1); a real match stands
alone. Below **1.45** the aligner refuses and the subtitles are left as they were. A
wrong shift is worse than none.

The threshold is measured: subtitles run against the wrong film score at most 1.30 on
the corpus, and real matches 1.6 and up; 1.45 is the middle of the gap. It was 1.3 until
the held-out films produced a control at exactly 1.30. A z-score against all shifts was
tried first: on nine-minute films it put real matches (8–12) and chance (up to 7.8) too
close together to choose a threshold.

## 5. Without audio (Tier 0)

SPEC.md §8: no VAD alignment on Tier 0. `scale_from_duration` suggests a scale when the
last subtitle ends after the film does, but a scaled version would end inside its last
eight minutes. 24/25 and 23.976/25 are 0.1% apart, and duration cannot tell them apart;
the nudge covers the rest.

## 6. The corpus (`fixtures/subtitles/`)

Real films' subtitles are neither ours to commit nor timed against a known truth, so the
corpus is built (`tools/subcorpus/build.py`):

- Ten short films (8–12 minutes, ~95–120 subtitles each) of dialogue spoken by
  Windows' own speech synthesiser, two voices at varied rates, placed at known times,
  over a quiet room, street noise with passing traffic, or a swelling music bed.
- The true subtitles are timed as a person would: 0–250 ms early, lingering 200–800 ms,
  close lines sometimes sharing a subtitle, one voice in twenty not subtitled.
- Only the **speech trace** the app's own detector made of each film is kept (run
  lengths, a few KB). The corpus tests the detector as it really behaves, and CI needs no
  voice.
- Eight damaged copies per film: none, small and large offsets, 23.976→25, 25→23.976
  and 24→25, an intro removed (everything after it late, with a gap where it was), and
  an intro added on top of a scale change (everything after it early, by no more than a
  quiet stretch the extra footage filled). A cue moves as a whole, and none moves before
  zero: a file cannot say where a subtitle clamped to 00:00:00 belonged.
- **Held out**: four of the ten films were built after the aligner was tuned on the
  other six and never tuned on. The harness reports both numbers.
- **Negative controls**: each film's subtitles against the next film's speech. Anything
  but a refusal is a false accept.

A copy **aligns** when 95% of its cues land within 150 ms of the true file. The target
is more than 90% of copies (SPEC.md §12.2), with no false accepts.

**Measured** (2026-11-06, `docs/eval-results.md`): 79/80 = **98.8%**; tuned-on 48/48,
**held out 31/32 = 96.9%**; false accepts **0/10**. The miss is a split whose second
part lands 156 ms off: the plateau's slack, just over the line. The speech detector agrees
with when lines were really spoken on 93.8–97.9% of frames (recall 89.5–96.1%,
precision 95.5–99.5%).

## 7. Known limits

- Synthetic speech is cleaner than a film's mix. Real dialogue under effects and score
  will produce a noisier trace; the corpus's street and music beds are the stand-in.
- Nine-minute films are a *harder* case than full films: the correlation grows with
  length, and a feature has ten times the subtitles.
- One split at most. A file cut in several places aligns its largest part.
