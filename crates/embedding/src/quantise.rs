//! Float vectors to int8, and the scale that makes them comparable again.
//!
//! ADR-0014 budgets **INT8, 384 dimensions, ~77 MB per 200,000 titles**. Float32 would
//! be four times that — 1.3 GB for this catalogue, against R4's 4 GB total — so the
//! quantisation is not an optimisation, it is what makes the artefact exist.
//!
//! # Why symmetric, per-vector scaling
//!
//! Sentence-transformer outputs are L2-normalised, so every component sits in roughly
//! [-1, 1] and the distribution is similar across vectors. A single global scale would
//! therefore be defensible — and would be wrong the first time a vector had an unusual
//! magnitude, clipping its largest components flat. Per-vector costs four bytes each
//! and cannot clip, because the scale is derived from the vector's own maximum.
//!
//! Symmetric (rather than an asymmetric zero-point) because the values are centred on
//! zero already. An asymmetric scheme buys a fraction of a bit here and costs a
//! subtraction on every comparison in the hot path.

/// The int8 range, less one so that `-128` never appears — its negation does not fit in
/// an i8, and a dot product that negates a component would overflow on exactly one value.
const MAX_MAGNITUDE: f32 = 127.0;

/// A quantised vector and the scale needed to read it back.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantised {
    pub values: Vec<i8>,
    /// Multiply an int8 component by this to recover the float.
    pub scale: f32,
}

/// Quantise one vector.
pub fn quantise(vector: &[f32]) -> Quantised {
    let peak = vector.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    // An all-zero vector has no scale to derive. Returning 0.0 would make every later
    // dequantisation produce NaN through 0/0, so the scale stays 1.0 and the values
    // stay zero — which dequantises back to exactly the vector we were given.
    let scale = if peak == 0.0 {
        1.0
    } else {
        peak / MAX_MAGNITUDE
    };

    Quantised {
        values: vector
            .iter()
            .map(|v| {
                let scaled = (v / scale).round();
                scaled.clamp(-MAX_MAGNITUDE, MAX_MAGNITUDE) as i8
            })
            .collect(),
        scale,
    }
}

/// Recover the float vector, approximately.
pub fn dequantise(quantised: &Quantised) -> Vec<f32> {
    quantised
        .values
        .iter()
        .map(|v| *v as f32 * quantised.scale)
        .collect()
}

/// Cosine similarity between two float vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalised(seed: u64, dimension: usize) -> Vec<f32> {
        // A deterministic pseudo-random unit vector, like a real model's output.
        let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut raw: Vec<f32> = (0..dimension)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 33) as f32 / (1u64 << 31) as f32) - 1.0
            })
            .collect();
        let norm: f32 = raw.iter().map(|v| v * v).sum::<f32>().sqrt();
        for v in &mut raw {
            *v /= norm;
        }
        raw
    }

    #[test]
    fn quantising_preserves_similarity_closely_enough_to_rank_with() {
        // The number that matters. If int8 changed the ORDER of results, the artefact
        // would be smaller and useless. 384 dimensions, as the real model produces.
        let a = normalised(1, 384);
        let b = normalised(2, 384);

        let exact = cosine(&a, &b);
        let approximate = cosine(&dequantise(&quantise(&a)), &dequantise(&quantise(&b)));

        assert!(
            (exact - approximate).abs() < 0.005,
            "cosine moved from {exact} to {approximate} — too much to rank with"
        );
    }

    #[test]
    fn a_vector_is_closest_to_itself_after_quantisation() {
        let v = normalised(7, 384);
        let self_similarity = cosine(&v, &dequantise(&quantise(&v)));
        assert!(
            self_similarity > 0.9999,
            "a vector should survive its own round trip: {self_similarity}"
        );
    }

    #[test]
    fn nothing_clips_because_the_scale_comes_from_the_vector_itself() {
        // A single global scale would flatten this vector's largest component. Per
        // vector, the peak maps exactly onto 127 and nothing is lost to clipping.
        let spiky = vec![0.001, 0.002, 9.5, -0.003];
        let q = quantise(&spiky);
        assert_eq!(q.values[2], 127);
        assert!(q.values.iter().all(|v| *v != i8::MIN));

        let back = dequantise(&q);
        assert!(
            (back[2] - 9.5).abs() < 0.05,
            "peak came back as {}",
            back[2]
        );
    }

    #[test]
    fn minus_128_is_never_produced() {
        // Its negation does not fit in an i8, so a dot product that negates a component
        // overflows on exactly one value out of 256 — the kind of bug that shows up
        // once in a million comparisons and is never reproducible.
        for seed in 0..200 {
            let q = quantise(&normalised(seed, 64));
            assert!(
                q.values.iter().all(|v| *v > i8::MIN),
                "seed {seed} produced -128"
            );
        }
        assert!(quantise(&[-1.0, 1.0]).values.iter().all(|v| *v > i8::MIN));
    }

    #[test]
    fn an_all_zero_vector_does_not_produce_nan() {
        // 0/0 in the scale would poison every later comparison with NaN, and NaN sorts
        // unpredictably rather than failing.
        let q = quantise(&[0.0; 8]);
        assert_eq!(q.scale, 1.0);
        let back = dequantise(&q);
        assert!(back.iter().all(|v| *v == 0.0 && !v.is_nan()));
        assert_eq!(cosine(&back, &back), 0.0, "no NaN escapes");
    }

    #[test]
    fn quantisation_is_deterministic() {
        let v = normalised(42, 384);
        let once = quantise(&v);
        for _ in 0..20 {
            assert_eq!(quantise(&v), once);
        }
    }
}
