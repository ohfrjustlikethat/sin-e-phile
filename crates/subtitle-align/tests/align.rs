//! The aligner on synthetic dialogue: a speech trace and subtitles made from the same
//! lines, then damaged in known ways. Self-contained, so the crate tests on its own; the
//! harder, audio-derived corpus is `fixtures/subtitles/` (`cargo run -p eval -- subs`).

use subtitle_align::align::{align, Options, Refusal};
use subtitle_align::{align_text, scale_from_duration, Cue};

/// xorshift64*: reproducible dialogue without a dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo) as u64) as i64
    }
}

/// Lines of dialogue over `minutes`: (start, end) in ms.
fn dialogue(seed: u64, minutes: i64) -> Vec<(i64, i64)> {
    let mut rng = Rng(seed);
    let mut t = rng.range(10_000, 40_000);
    let mut out = Vec::new();
    while t < minutes * 60_000 - 10_000 {
        let len = rng.range(700, 4_000);
        out.push((t, t + len));
        t += len
            + if rng.range(0, 10) == 0 {
                rng.range(4_000, 20_000)
            } else {
                rng.range(150, 2_500)
            };
    }
    out
}

/// What a speech detector would make of the lines: mostly right, with misses and noise.
fn trace(lines: &[(i64, i64)], minutes: i64, seed: u64) -> Vec<bool> {
    let mut rng = Rng(seed ^ 0xABCD);
    let mut v = vec![false; (minutes * 6_000) as usize];
    for &(a, b) in lines {
        for f in (a / 10)..(b / 10).min(v.len() as i64) {
            v[f as usize] = rng.range(0, 100) >= 8; // 8% of speech frames missed
        }
    }
    // Doors, footsteps, music: short false bursts.
    for _ in 0..minutes * 6 {
        let at = rng.range(0, v.len() as i64 - 100) as usize;
        let len = rng.range(10, 60) as usize;
        v[at..at + len].iter_mut().for_each(|x| *x = true);
    }
    v
}

/// The lines as a subtitler times them: a little early, lingering after.
fn cues(lines: &[(i64, i64)], seed: u64) -> Vec<Cue> {
    let mut rng = Rng(seed ^ 0x1234);
    lines
        .iter()
        .map(|&(a, b)| Cue {
            start: (a - rng.range(0, 250)).max(0),
            end: b + rng.range(200, 800),
        })
        .collect()
}

fn damage(cues: &[Cue], f: impl Fn(i64) -> i64) -> Vec<Cue> {
    cues.iter()
        .map(|c| Cue {
            start: f(c.start),
            end: f(c.end),
        })
        .collect()
}

/// The share of damaged cues the alignment puts back within 150 ms of the truth.
fn landed(truth: &[Cue], bad: &[Cue], a: &subtitle_align::Alignment) -> f64 {
    let ok = truth
        .iter()
        .zip(bad)
        .filter(|(t, b)| (a.map(b.start) - t.start).abs() <= 150)
        .count();
    ok as f64 / truth.len() as f64
}

const MIN: i64 = 25;

fn film(seed: u64) -> (Vec<bool>, Vec<Cue>) {
    let lines = dialogue(seed, MIN);
    (trace(&lines, MIN, seed), cues(&lines, seed))
}

#[test]
fn an_offset_is_found() {
    let (speech, truth) = film(1);
    for shift in [-73_400, -2_150, 900, 41_000] {
        let bad = damage(&truth, |t| t + shift);
        let a = align(&speech, &bad, &Options::default()).unwrap();
        assert_eq!(a.scale, 1.0);
        assert!(
            (a.offset_ms + shift).abs() <= 150,
            "shift {shift}: found {}",
            a.offset_ms
        );
        assert!(landed(&truth, &bad, &a) > 0.97);
    }
}

#[test]
fn a_framerate_mismatch_is_a_scale_not_an_offset() {
    // Subtitles timed for a 23.976 fps release, played against the 25 fps one: every
    // time is 4.1% short, so a pure offset can fix a moment, never the whole film.
    let (speech, truth) = film(2);
    let bad = damage(&truth, |t| (t as f64 * 23.976 / 25.0) as i64 + 3_000);
    let a = align(&speech, &bad, &Options::default()).unwrap();
    assert!((a.scale - 25.0 / 23.976).abs() < 1e-9, "scale {}", a.scale);
    assert!(
        landed(&truth, &bad, &a) > 0.97,
        "{}",
        landed(&truth, &bad, &a)
    );

    // An offset alone could not do it.
    let offset_only = Options {
        scales: vec![1.0],
        ..Options::default()
    };
    // (It either refuses or lands few cues.)
    if let Ok(a) = align(&speech, &bad, &offset_only) {
        assert!(
            landed(&truth, &bad, &a) < 0.5,
            "an offset alone landed {}",
            landed(&truth, &bad, &a)
        );
    }
}

#[test]
fn a_removed_intro_splits_the_file() {
    let (speech, truth) = film(3);
    let cut = truth[truth.len() / 3].start;
    let bad = damage(&truth, |t| if t >= cut { t - 47_000 } else { t } + 1_200);
    let a = align(&speech, &bad, &Options::default()).unwrap();
    let split = a.split.unwrap_or_else(|| panic!("a split: {a:?}"));
    assert!(
        landed(&truth, &bad, &a) > 0.95,
        "{} (split at {} → {})",
        landed(&truth, &bad, &a),
        split.at_ms,
        split.offset_ms
    );
}

#[test]
fn an_aligned_file_stays_aligned() {
    let (speech, truth) = film(4);
    let a = align(&speech, &truth, &Options::default()).unwrap();
    assert_eq!((a.scale, a.split), (1.0, None));
    assert!(a.offset_ms.abs() <= 150, "{}", a.offset_ms);
}

#[test]
fn it_refuses_rather_than_guess() {
    // Subtitles of one film against the speech of another: nothing to find.
    let (speech, _) = film(5);
    let (_, other) = film(6);
    match align(&speech, &other, &Options::default()) {
        Err(Refusal::NotConfident(c)) => assert!(c < 8.0),
        r => panic!("should refuse, got {r:?}"),
    }
    assert_eq!(
        align(&vec![false; 1000], &other, &Options::default()),
        Err(Refusal::NoSpeech)
    );
    assert_eq!(
        align(&speech, &[], &Options::default()),
        Err(Refusal::NoCues)
    );
}

#[test]
fn the_text_comes_back_retimed() {
    let (speech, truth) = film(7);
    let bad = damage(&truth, |t| t + 5_000);
    let srt: String = bad
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let f = |ms: i64| {
                format!(
                    "{:02}:{:02}:{:02},{:03}",
                    ms / 3_600_000,
                    ms / 60_000 % 60,
                    ms / 1000 % 60,
                    ms % 1000
                )
            };
            format!("{}\n{} --> {}\nline {i}\n\n", i + 1, f(c.start), f(c.end))
        })
        .collect();
    let (fixed, a) = align_text(&srt, &speech, &Options::default()).unwrap();
    assert!((a.offset_ms + 5_000).abs() <= 150, "{a:?}");
    assert!(fixed.contains("line 0") && fixed.lines().count() == srt.lines().count());
}

#[test]
fn without_audio_the_duration_suggests_a_scale() {
    // Tier 0: a file whose last subtitle ends 4% after a 100-minute film cannot be right
    // at 1.0; 23.976/25 brings it back inside.
    let cues = [
        Cue {
            start: 0,
            end: 1_000,
        },
        Cue {
            start: 6_100_000,
            end: 6_200_000,
        },
    ];
    // (24/25 and 23.976/25 are 0.1% apart; a duration cannot tell them apart, and the
    // nudge covers the difference.)
    let s = scale_from_duration(&cues, 6_000_000).unwrap();
    assert!((s - 0.96).abs() < 0.002, "{s}");
    // One that already fits is left alone.
    let fits = [Cue {
        start: 0,
        end: 5_700_000,
    }];
    assert_eq!(scale_from_duration(&fits, 6_000_000), None);
}
