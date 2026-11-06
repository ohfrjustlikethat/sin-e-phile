//! Solving for offset *and* framerate scale.
//!
//! A subtitle made for a 23.976 fps release, played against a 25 fps one, starts right and
//! drifts 4% — over two minutes by the end of the film. No offset fixes that: the fix is a
//! **multiplication** (the scale) and an **addition** (the offset), `new = scale × old +
//! offset`. So for each plausible scale the subtitles are stretched, turned into a 10 ms
//! "a subtitle is showing" signal, and slid along the speech trace; the slide with the most
//! speech under the subtitles wins. Sliding is a cross-correlation, done with an FFT so
//! every offset is tried at once.
//!
//! **Splits.** When an intro has been added or cut, the start of the file wants one offset
//! and the rest another. Both show up as peaks of the same correlation, each as tall as
//! its part is long. So the few tallest distinct peaks are taken as candidate offsets,
//! every cue is scored at each, and the cue where switching from one offset to another
//! gains most is the split. It is kept only if both parts are confident on their own.
//!
//! **Confidence** is how far the winning slide stands above the best *different* one (at
//! least 3 s away), each measured from the average slide: 2.0 means twice as high. Chance
//! produces near-ties, so a ratio near 1; a real match stands alone. Below
//! [`Options::min_confidence`] the aligner refuses, because a wrong shift is worse than
//! the subtitles as they were. (A z-score against every slide was tried first: on a
//! nine-minute film it put real matches and chance too close together to choose a
//! threshold.)

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::cue::Cue;
use crate::vad::FRAME_MS;

/// Framerate pairs a release can be converted between: 23.976, 24, 25 (PAL speed-up).
pub const SCALES: [f64; 7] = [
    1.0,
    25.0 / 23.976,
    23.976 / 25.0,
    25.0 / 24.0,
    24.0 / 25.0,
    24.0 / 23.976,
    23.976 / 24.0,
];

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub scales: Vec<f64>,
    /// Refuse below this distinctness (see the module note).
    pub min_confidence: f64,
    /// The largest shift considered, either way.
    pub max_offset_ms: i64,
}

impl Default for Options {
    /// The threshold is measured, not chosen: on `fixtures/subtitles/`, subtitles against
    /// the wrong film score at most 1.30 and every real match at least 1.6. 1.45 is the
    /// middle of that gap. (It was 1.3 until the held-out films produced a control at
    /// exactly 1.30.)
    fn default() -> Self {
        Self {
            scales: SCALES.to_vec(),
            min_confidence: 1.45,
            max_offset_ms: 10 * 60 * 1000,
        }
    }
}

/// Where the second part of a split file starts, and its own offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Split {
    /// In the subtitle file's own time: cues starting here or later take `offset_ms`.
    pub at_ms: i64,
    pub offset_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alignment {
    pub scale: f64,
    pub offset_ms: i64,
    pub split: Option<Split>,
    /// The winner's distinctness (see the module note); for a split, the weaker part's.
    pub confidence: f64,
}

impl Alignment {
    /// Where a time in the subtitle file belongs in the film.
    pub fn map(&self, t: i64) -> i64 {
        let offset = match self.split {
            Some(s) if t >= s.at_ms => s.offset_ms,
            _ => self.offset_ms,
        };
        (self.scale * t as f64).round() as i64 + offset
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The trace has no speech to align to.
    NoSpeech,
    NoCues,
    /// The best match is not clearly better than chance.
    NotConfident(f64),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSpeech => write!(f, "no speech found in the audio"),
            Self::NoCues => write!(f, "the subtitles have no timings"),
            Self::NotConfident(c) => write!(f, "no confident match (confidence {c:.2})"),
        }
    }
}

/// A rival must be this far from the winner's plateau to count as a different answer.
const RIVAL_FRAMES: i64 = 300;
/// How close to the top still counts as the plateau, as a share of the peak's height.
const PLATEAU: f64 = 0.01;
/// Candidate offsets for a split: the tallest distinct peaks.
const CANDIDATES: usize = 6;
/// A split's parts must differ by more than this to be a split rather than noise.
const SPLIT_MIN_FRAMES: i64 = 50;
/// How far (frames) a split's first part may overlap its second: subtitles overlap a
/// little in real files.
const ORDER_SLACK: i64 = 100;

/// How much better a stretched fit must be than an unstretched one to be believed.
const PREFER_UNSCALED: f64 = 0.02;

/// The smallest run of cues that proposes an offset of its own (about 40 s of dialogue).
const WINDOW_CUES: usize = 12;
const MAX_WINDOWS: usize = 16;
/// And each part must have at least this many cues.
const MIN_PART: usize = 8;

/// Align `cues` to the speech trace (one entry per [`FRAME_MS`]).
pub fn align(speech: &[bool], cues: &[Cue], opts: &Options) -> Result<Alignment, Refusal> {
    if cues.is_empty() {
        return Err(Refusal::NoCues);
    }
    let talk = speech.iter().filter(|&&s| s).count();
    if talk == 0 {
        return Err(Refusal::NoSpeech);
    }
    // Zero-mean speech: a subtitle over silence costs as much as one over speech earns, so
    // a long shift that simply overlaps more cannot win by size.
    let mean = talk as f64 / speech.len() as f64;
    let s: Vec<f64> = speech
        .iter()
        .map(|&v| f64::from(u8::from(v)) - mean)
        .collect();
    let prefix: Vec<f64> = std::iter::once(0.0)
        .chain(s.iter().scan(0.0, |acc, &x| {
            *acc += x;
            Some(*acc)
        }))
        .collect();
    let mut planner = FftPlanner::new();
    let max_lag = opts.max_offset_ms / FRAME_MS;

    // Each scale's best fit, with or without a split; scales compared per subtitle frame,
    // since stretching the file lengthens its signal.
    let mut best: Option<Fit> = None;
    let mut unscaled: Option<Fit> = None;
    for &scale in &opts.scales {
        let signal = render(cues, scale, 0);
        let on = signal.iter().filter(|&&v| v > 0.0).count().max(1) as f64;
        let values = correlation(&mut planner, &s, &signal, -max_lag, max_lag);
        // A short part's peak can be lower than the long part's side-lobes, so short runs
        // of cues also propose the offset they would take on their own.
        let mut offsets = candidates(&values);
        for part in windows(cues) {
            let p = peak(&correlation(
                &mut planner,
                &s,
                &render(part, scale, 0),
                -max_lag,
                max_lag,
            ));
            // No distinctness bar here: forty seconds of dialogue is weak evidence on its
            // own. The split must still beat one offset clearly (best_fit) and its larger
            // part must be confident (below).
            if offsets.iter().all(|&o| (o - p.lag).abs() > RIVAL_FRAMES) {
                offsets.push(p.lag);
            }
        }
        let fit = best_fit(&values, &offsets, &prefix, cues, scale, on);
        if scale == 1.0 {
            unscaled = Some(fit);
        }
        if best.as_ref().is_none_or(|b| fit.per_frame > b.per_frame) {
            best = Some(fit);
        }
    }
    // No stretch unless a stretch is clearly better: over a few minutes a 0.1% one (24 to
    // 23.976) fits noise a little better, while a real conversion wins by far more.
    let best = best.expect("at least one scale");
    let fit = match unscaled {
        Some(u) if u.per_frame >= best.per_frame * (1.0 - PREFER_UNSCALED) => u,
        _ => best,
    };
    let scale = fit.scale;

    let single = Alignment {
        scale,
        offset_ms: fit.single.lag * FRAME_MS,
        split: None,
        confidence: fit.single.distinct,
    };
    let result = match fit.split {
        Some((k, left, right)) => {
            // Each part, refined and judged on its own.
            let first = &cues[..k];
            let second = &cues[k..];
            let a = refine(&mut planner, &s, first, scale, left, max_lag);
            let b = refine(&mut planner, &s, second, scale, right, max_lag);
            Alignment {
                scale,
                offset_ms: a.lag * FRAME_MS,
                split: Some(Split {
                    at_ms: cues[k].start,
                    offset_ms: b.lag * FRAME_MS,
                }),
                // The larger part carries the evidence: a short part (an intro is a minute
                // or two) is too little dialogue to stand out on its own, and is accepted
                // because moving it gains clearly over one offset.
                confidence: if k * 2 >= cues.len() {
                    a.distinct
                } else {
                    b.distinct
                },
            }
        }
        None => single,
    };
    // A split whose parts are not each confident is not trusted; one offset may still be.
    let result =
        if result.confidence < opts.min_confidence && single.confidence >= opts.min_confidence {
            single
        } else {
            result
        };
    if result.confidence < opts.min_confidence {
        return Err(Refusal::NotConfident(result.confidence));
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy)]
struct Peak {
    lag: i64,
    value: f64,
    distinct: f64,
}

/// One scale's answer: its single best offset, and a split if one fits much better.
#[derive(Clone, Copy)]
struct Fit {
    scale: f64,
    single: Peak,
    /// (first cue of the second part, its offset before, its offset from there), frames.
    split: Option<(usize, i64, i64)>,
    per_frame: f64,
}

fn best_fit(
    values: &[(i64, f64)],
    offsets: &[i64],
    prefix: &[f64],
    cues: &[Cue],
    scale: f64,
    on: f64,
) -> Fit {
    let single = peak(values);
    // Speech under each cue at each candidate offset, from the prefix sums.
    let frames: Vec<(i64, i64)> = cues
        .iter()
        .map(|c| (frame(c.start, scale), frame(c.end, scale)))
        .collect();
    let under = |o: i64| -> Vec<f64> {
        frames
            .iter()
            .map(|&(a, b)| {
                let at = |f: i64| prefix[(f + o).clamp(0, prefix.len() as i64 - 1) as usize];
                at(b) - at(a)
            })
            .collect()
    };
    let scores: Vec<Vec<f64>> = offsets.iter().map(|&o| under(o)).collect();
    let whole = |i: usize| scores[i].iter().sum::<f64>();
    let one = (0..offsets.len()).map(whole).fold(single.value, f64::max);

    let mut split: Option<(usize, i64, i64, f64)> = None;
    for (i, &left) in offsets.iter().enumerate() {
        for (j, &right) in offsets.iter().enumerate() {
            if (left - right).abs() < SPLIT_MIN_FRAMES {
                continue;
            }
            // Σ before k at `left` + Σ from k at `right`, for every k at once.
            let mut before = 0.0;
            let mut after: f64 = scores[j].iter().sum();
            for k in 0..cues.len() {
                // Film time runs forward: the first part must end before the second
                // begins. Without this, a short run of cues can be thrown minutes away to
                // a talkier stretch, where any subtitles find more speech under them.
                let in_order =
                    k == 0 || frames[k - 1].1 + left <= frames[k].0 + right + ORDER_SLACK;
                if in_order && k >= MIN_PART && cues.len() - k >= MIN_PART {
                    let total = before + after;
                    if split.is_none_or(|s| total > s.3) {
                        split = Some((k, left, right, total));
                    }
                }
                before += scores[i][k];
                after -= scores[j][k];
            }
        }
    }
    // A split must earn its keep: a clearly better fit than one offset can manage. Noise
    // alone buys a split a percent or two on a clean file.
    let split = split.filter(|s| s.3 > one * 1.10 && s.3 - one > 0.03 * on);
    let total = split.map_or(one, |s| s.3);
    Fit {
        scale,
        single,
        split: split.map(|(k, l, r, _)| (k, l, r)),
        per_frame: total / on,
    }
}

/// Short, half-overlapping runs of cues across the first 60% of the file, where an intro
/// is by definition; at most [`MAX_WINDOWS`], so a feature costs no more than a short.
fn windows(cues: &[Cue]) -> Vec<&[Cue]> {
    let reach = cues.len() * 6 / 10;
    let size = (reach / 20).max(WINDOW_CUES).min(cues.len());
    let step = (size / 2).max(reach / MAX_WINDOWS).max(1);
    (0..reach.saturating_sub(size / 2))
        .step_by(step)
        .map(|i| &cues[i..(i + size).min(cues.len())])
        .take(MAX_WINDOWS)
        .collect()
}

fn frame(t: i64, scale: f64) -> i64 {
    ((t as f64 * scale) / FRAME_MS as f64).round() as i64
}

/// The cues, scaled, as a 10 ms signal: 1 while a subtitle shows. `from` is the frame the
/// signal starts at, so a run of cues can be rendered on its own.
fn render(cues: &[Cue], scale: f64, from: i64) -> Vec<f64> {
    let len = cues
        .iter()
        .map(|c| frame(c.end, scale) - from)
        .max()
        .unwrap_or(0)
        .max(0) as usize
        + 1;
    let mut v = vec![0.0; len];
    for c in cues {
        let a = (frame(c.start, scale) - from).max(0) as usize;
        let b = (frame(c.end, scale) - from).max(0) as usize;
        for x in v.iter_mut().take(b.min(len)).skip(a) {
            *x = 1.0;
        }
    }
    v
}

/// Σ_j signal[j] · speech[j + k] for every lag `k` in `lo..=hi`.
fn correlation(
    planner: &mut FftPlanner<f64>,
    speech: &[f64],
    signal: &[f64],
    lo: i64,
    hi: i64,
) -> Vec<(i64, f64)> {
    let n = (speech.len() + signal.len()).next_power_of_two();
    let fwd: Arc<dyn Fft<f64>> = planner.plan_fft_forward(n);
    let inv: Arc<dyn Fft<f64>> = planner.plan_fft_inverse(n);
    let mut a: Vec<Complex<f64>> = speech.iter().map(|&x| Complex::new(x, 0.0)).collect();
    a.resize(n, Complex::default());
    let mut b: Vec<Complex<f64>> = signal.iter().map(|&x| Complex::new(x, 0.0)).collect();
    b.resize(n, Complex::default());
    fwd.process(&mut a);
    fwd.process(&mut b);
    // corr[k] = Σ_j b[j] a[j + k]  ⇔  IFFT(A · conj(B)).
    let mut c: Vec<Complex<f64>> = a.iter().zip(&b).map(|(x, y)| x * y.conj()).collect();
    inv.process(&mut c);
    let lo = lo.max(-(signal.len() as i64));
    let hi = hi.min(speech.len() as i64);
    (lo..=hi)
        .map(|k| (k, c[k.rem_euclid(n as i64) as usize].re / n as f64))
        .collect()
}

/// The best lag, as the middle of its plateau, with its distinctness.
///
/// The top is a plateau, not a point: a subtitle shows a little before its line and
/// lingers after it, so sliding it by less than that slack keeps the same speech under it.
/// Any lag on the plateau scores the same, and the first one found is biased to an edge —
/// up to 200 ms off. Its middle is the honest answer.
fn peak(values: &[(i64, f64)]) -> Peak {
    let Some(i) = (0..values.len()).max_by(|&x, &y| values[x].1.total_cmp(&values[y].1)) else {
        return Peak {
            lag: 0,
            value: 0.0,
            distinct: 0.0,
        };
    };
    let mean = values.iter().map(|v| v.1).sum::<f64>() / values.len() as f64;
    let (a, b) = plateau(values, i, mean);
    Peak {
        lag: (values[a].0 + values[b].0).div_euclid(2),
        value: values[i].1,
        distinct: distinctness(values, (values[a].0, values[b].0), values[i].1, mean),
    }
}

fn plateau(values: &[(i64, f64)], i: usize, mean: f64) -> (usize, usize) {
    let near = (values[i].1 - mean) * PLATEAU;
    let floor = values[i].1 - near;
    let (mut a, mut b) = (i, i);
    while a > 0 && values[a - 1].1 >= floor {
        a -= 1;
    }
    while b + 1 < values.len() && values[b + 1].1 >= floor {
        b += 1;
    }
    (a, b)
}

/// The winner's height over the best rival's, both above the mean; capped at 10.
fn distinctness(values: &[(i64, f64)], plateau: (i64, i64), top: f64, mean: f64) -> f64 {
    let rival = values
        .iter()
        .filter(|v| v.0 < plateau.0 - RIVAL_FRAMES || v.0 > plateau.1 + RIVAL_FRAMES)
        .map(|v| v.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if !rival.is_finite() || rival <= mean {
        return 10.0;
    }
    ((top - mean) / (rival - mean)).min(10.0)
}

/// The tallest distinct peaks' lags (plateau centres), tallest first.
fn candidates(values: &[(i64, f64)]) -> Vec<i64> {
    if values.is_empty() {
        return vec![];
    }
    let mean = values.iter().map(|v| v.1).sum::<f64>() / values.len() as f64;
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&x, &y| values[y].1.total_cmp(&values[x].1));
    let mut out: Vec<i64> = Vec::new();
    for i in order {
        let lag = values[i].0;
        if out.iter().any(|&o| (o - lag).abs() <= RIVAL_FRAMES) {
            continue;
        }
        let (a, b) = plateau(values, i, mean);
        out.push((values[a].0 + values[b].0).div_euclid(2));
        if out.len() == CANDIDATES {
            break;
        }
    }
    out
}

/// One part of a split, aligned on its own near where the split put it; its distinctness
/// is judged over every lag, so a part that only fits by luck is caught.
fn refine(
    planner: &mut FftPlanner<f64>,
    s: &[f64],
    cues: &[Cue],
    scale: f64,
    near: i64,
    max_lag: i64,
) -> Peak {
    let signal = render(cues, scale, 0);
    let all = correlation(planner, s, &signal, -max_lag, max_lag);
    let whole = peak(&all);
    let local: Vec<(i64, f64)> = all
        .iter()
        .copied()
        .filter(|v| (v.0 - near).abs() <= 100)
        .collect();
    let here = peak(&local);
    Peak {
        lag: here.lag,
        value: here.value,
        distinct: if (whole.lag - here.lag).abs() <= 100 {
            whole.distinct
        } else {
            0.0
        },
    }
}
