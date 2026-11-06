//! Voice activity: which 10 ms frames of audio contain speech.
//!
//! Deliberately simple, and pure Rust: alignment needs *where speech starts and stops*,
//! not a perfect transcript, and cross-correlation forgives a frame wrong here and there.
//! A frame is speech when, in the speech band:
//!
//! 1. it is clearly louder than the local noise floor (9 dB over the quietest second
//!    nearby), and
//! 2. its loudness *moves* — speech rises and falls with syllables, several times a second,
//!    where a held note or a hum stays level. This is what keeps a score from reading as
//!    dialogue.
//!
//! Then short gaps are bridged and blips dropped, as a listener would hear it.

/// One frame of the speech trace.
pub const FRAME_MS: i64 = 10;

/// Over the floor by this much to count as loud.
const OVER_FLOOR_DB: f32 = 9.0;
/// Below this, silence whatever the floor (digital silence would otherwise be "noisy").
const ABSOLUTE_FLOOR_DB: f32 = -60.0;
/// The spread of loudness, over about a fifth of a second, that marks syllables.
const MODULATION_DB: f32 = 2.5;
/// Gaps shorter than this inside speech are a breath, not silence.
const BRIDGE_FRAMES: usize = 20;
/// Speech shorter than this is a click.
const MIN_RUN_FRAMES: usize = 6;

/// The speech trace of mono `samples` at `rate` Hz, one entry per 10 ms.
pub fn speech(samples: &[f32], rate: u32) -> Vec<bool> {
    let band = band_pass(samples, rate as f32);
    let hop = (rate as usize * FRAME_MS as usize / 1000).max(1);
    let db: Vec<f32> = band
        .chunks(hop)
        .map(|f| {
            let power = f.iter().map(|x| x * x).sum::<f32>() / f.len() as f32;
            10.0 * (power + 1e-12).log10()
        })
        .collect();
    let floor = noise_floor(&db);
    let spread = local_spread(&db, 10);
    let raw: Vec<bool> = (0..db.len())
        .map(|i| {
            db[i] > ABSOLUTE_FLOOR_DB
                && db[i] > floor[i] + OVER_FLOOR_DB
                && spread[i] > MODULATION_DB
        })
        .collect();
    smooth(raw)
}

/// 250–3400 Hz, where speech carries its energy: two second-order filters (the RBJ
/// audio-EQ cookbook), high-pass then low-pass.
fn band_pass(x: &[f32], rate: f32) -> Vec<f32> {
    let hp = Biquad::new(rate, 250.0, true);
    let lp = Biquad::new(rate, 3400.0_f32.min(rate * 0.45), false);
    lp.run(&hp.run(x))
}

struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
}

impl Biquad {
    fn new(rate: f32, cutoff: f32, high: bool) -> Self {
        let w = 2.0 * std::f32::consts::PI * cutoff / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let b = if high {
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0]
        } else {
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0]
        };
        Self {
            b: b.map(|v| v / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
        }
    }

    fn run(&self, x: &[f32]) -> Vec<f32> {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        x.iter()
            .map(|&x0| {
                let y0 = self.b[0] * x0 + self.b[1] * x1 + self.b[2] * x2
                    - self.a[0] * y1
                    - self.a[1] * y2;
                (x2, x1, y2, y1) = (x1, x0, y1, y0);
                y0
            })
            .collect()
    }
}

/// For each frame, the quietest nearby second: the 10th percentile of each one-second
/// block, then the lowest of those within three seconds either side.
fn noise_floor(db: &[f32]) -> Vec<f32> {
    let block = 100;
    let lows: Vec<f32> = db
        .chunks(block)
        .map(|b| {
            let mut v = b.to_vec();
            v.sort_by(f32::total_cmp);
            v[v.len() / 10]
        })
        .collect();
    (0..db.len())
        .map(|i| {
            let b = i / block;
            let from = b.saturating_sub(3);
            let to = (b + 4).min(lows.len());
            lows[from..to].iter().copied().fold(f32::INFINITY, f32::min)
        })
        .collect()
}

/// For each frame, the standard deviation of loudness over `half` frames either side.
fn local_spread(db: &[f32], half: usize) -> Vec<f32> {
    (0..db.len())
        .map(|i| {
            let w = &db[i.saturating_sub(half)..(i + half + 1).min(db.len())];
            let mean = w.iter().sum::<f32>() / w.len() as f32;
            (w.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / w.len() as f32).sqrt()
        })
        .collect()
}

/// Bridge breaths, then drop clicks.
fn smooth(mut v: Vec<bool>) -> Vec<bool> {
    let runs = |v: &[bool], want: bool| -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < v.len() {
            if v[i] == want {
                let s = i;
                while i < v.len() && v[i] == want {
                    i += 1;
                }
                out.push((s, i));
            } else {
                i += 1;
            }
        }
        out
    };
    for (s, e) in runs(&v, false) {
        if s > 0 && e < v.len() && e - s < BRIDGE_FRAMES {
            v[s..e].iter_mut().for_each(|x| *x = true);
        }
    }
    for (s, e) in runs(&v, true) {
        if e - s < MIN_RUN_FRAMES {
            v[s..e].iter_mut().for_each(|x| *x = false);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    /// A vowel-like tone, rising and falling four times a second, the way syllables do.
    fn syllables(secs: f32) -> Vec<f32> {
        let n = (secs * RATE as f32) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let envelope = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * 4.0 * t).cos());
                envelope * 0.3 * (2.0 * std::f32::consts::PI * 700.0 * t).sin()
            })
            .collect()
    }

    fn silence(secs: f32) -> Vec<f32> {
        // A little hiss, as a real recording has.
        let n = (secs * RATE as f32) as usize;
        let mut seed = 1u32;
        (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 * 0.002 - 0.001
            })
            .collect()
    }

    fn hum(secs: f32) -> Vec<f32> {
        let n = (secs * RATE as f32) as usize;
        (0..n)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / RATE as f32).sin())
            .collect()
    }

    fn share(v: &[bool]) -> f32 {
        v.iter().filter(|&&s| s).count() as f32 / v.len().max(1) as f32
    }

    #[test]
    fn speech_is_found_where_it_is() {
        let audio = [silence(3.0), syllables(2.0), silence(3.0)].concat();
        let v = speech(&audio, RATE);
        assert_eq!(v.len(), 800);
        assert!(share(&v[..280]) < 0.02, "quiet before");
        assert!(
            share(&v[320..480]) > 0.9,
            "speech found: {}",
            share(&v[320..480])
        );
        assert!(share(&v[520..]) < 0.02, "quiet after");
    }

    #[test]
    fn a_held_note_is_not_speech() {
        let audio = [silence(3.0), hum(4.0), silence(3.0)].concat();
        let v = speech(&audio, RATE);
        assert!(share(&v) < 0.03, "{}", share(&v));
    }

    #[test]
    fn breaths_are_bridged_and_clicks_dropped() {
        let v = smooth(
            [
                vec![false; 10],
                vec![true; 30],
                vec![false; 10],
                vec![true; 30],
                vec![false; 30],
                vec![true; 3],
                vec![false; 10],
            ]
            .concat(),
        );
        assert!(v[10..80].iter().all(|&s| s), "the 100 ms breath is bridged");
        assert!(
            v[110..113].iter().all(|&s| !s),
            "the 30 ms click is dropped"
        );
    }
}
