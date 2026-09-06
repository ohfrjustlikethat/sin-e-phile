//! Text to vector: one implementation, shared by the producer and by every query.
//!
//! ADR-0015 splits the work by tier: **catalogue documents** are embedded once on a
//! capable machine and shipped as an artefact, but a **query** is embedded on device on
//! every tier, because it is one forward pass over about thirty tokens. Both halves are
//! this file.
//!
//! # Why one implementation is a correctness requirement, not tidiness
//!
//! A query is compared against document vectors by cosine. That comparison is only
//! meaningful if both were produced the same way — same tokenizer, same truncation,
//! same pooling, same normalisation. Two implementations that agree today drift the
//! first time one is "improved", and the symptom is not an error: it is search quietly
//! getting worse, with nothing to point at, exactly as ADR-0014 describes for a
//! mismatched artefact.
//!
//! So the producer in `tools/ingest` and the application's query path call the same
//! [`Embedder::embed`]. `cargo run -p eval --release -- embed --report` re-derives a
//! document, embeds it here, and compares the result against that title's vector *in
//! the artefact* — byte-identical, since ADR-0014 requires the artefact to be
//! deterministic. It is the only check that can catch the two halves disagreeing, and
//! it lives in the harness rather than in a unit test because it needs the real model
//! and the real artefact to mean anything.
//!
//! # What this crate deliberately does not do
//!
//! It does not know what an artefact is, what a catalogue is, or which model is the
//! right one. It loads the model it is given and returns vectors.
//! [`sinephile_embedding::MODEL`] names the model the artefact was built with, and the
//! header check is what refuses a mismatch.

use std::path::Path;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::TensorRef;

/// The model's limit, and where a long document is cut.
///
/// Truncation happens **here rather than in the tokenizer's own configuration**,
/// because the artefact must be byte-reproducible: leaving the cut to whatever a
/// tokenizer version happens to default to would make a rebuild depend on a dependency
/// bump.
const MAX_TOKENS: usize = 256;

/// MiniLM's output width. Asserted against the artefact header rather than assumed.
pub const DIMENSION: u16 = 384;

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("{path}: {message}")]
    Load { path: String, message: String },
    #[error("tokenize: {0}")]
    Tokenize(String),
    #[error("inference: {0}")]
    Inference(String),
    #[error("the model returned {found}-dimensional vectors, this build expects {expected}")]
    Dimension { found: usize, expected: u16 },
}

/// A loaded sentence-transformer.
pub struct Embedder {
    session: Session,
    tokenizer: tokenizers::Tokenizer,
    identity: String,
    dimension: u16,
}

impl Embedder {
    /// Load a model and its tokenizer.
    ///
    /// `identity` is what gets written into an artefact and compared on load, so it
    /// must name the model **and its quantisation**: `all-MiniLM-L6-v2-int8` and
    /// `all-MiniLM-L6-v2-fp32` produce different vectors and must never be
    /// interchangeable.
    pub fn load(model: &Path, tokenizer: &Path, identity: &str) -> Result<Self, EmbedError> {
        let fail = |path: &Path, e: &dyn std::fmt::Display| EmbedError::Load {
            path: path.display().to_string(),
            message: e.to_string(),
        };

        let session = Session::builder()
            .map_err(|e| fail(model, &e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| fail(model, &e))?
            .commit_from_file(model)
            .map_err(|e| fail(model, &e))?;

        let mut tok =
            tokenizers::Tokenizer::from_file(tokenizer).map_err(|e| fail(tokenizer, &e))?;
        // This tokenizer.json pads to a fixed 128 tokens. Spike C found that makes the
        // model do roughly four times the work a short input needs; over 855,703
        // documents that was the difference between twenty minutes and an hour, and on
        // a query it is latency spent on padding.
        tok.with_padding(None);
        tok.with_truncation(Some(tokenizers::TruncationParams {
            max_length: MAX_TOKENS,
            ..Default::default()
        }))
        .map_err(|e| fail(tokenizer, &e))?;

        Ok(Self {
            session,
            tokenizer: tok,
            identity: identity.to_string(),
            dimension: DIMENSION,
        })
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn dimension(&self) -> u16 {
        self.dimension
    }

    /// One forward pass: tokenize, run, mean-pool over real tokens, L2-normalise.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let encoded = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbedError::Tokenize(e.to_string()))?;
        let len = encoded.len().max(1);

        let ids: Vec<i64> = encoded.get_ids().iter().map(|i| *i as i64).collect();
        let mask: Vec<i64> = encoded
            .get_attention_mask()
            .iter()
            .map(|i| *i as i64)
            .collect();
        let types: Vec<i64> = vec![0; len];
        let shape = [1_i64, len as i64];

        let map = |e: ort::Error| EmbedError::Inference(e.to_string());
        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => TensorRef::from_array_view((shape, ids.as_slice())).map_err(map)?,
                "attention_mask" => TensorRef::from_array_view((shape, mask.as_slice())).map_err(map)?,
                "token_type_ids" => TensorRef::from_array_view((shape, types.as_slice())).map_err(map)?,
            ])
            .map_err(map)?;

        let (out_shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(map)?;
        let hidden = *out_shape.last().unwrap_or(&(DIMENSION as i64)) as usize;
        if hidden != self.dimension as usize {
            return Err(EmbedError::Dimension {
                found: hidden,
                expected: self.dimension,
            });
        }

        Ok(pool(data, &mask, len, hidden))
    }
}

/// Mean-pool over non-padding tokens, then L2-normalise.
///
/// **Padding tokens carry real activations**, so averaging them in drags every long
/// document toward the same point — the mask is not a formality. Extracted as a free
/// function so it can be tested without a 22 MB model, which is the only part of this
/// crate that can be.
fn pool(data: &[f32], mask: &[i64], len: usize, hidden: usize) -> Vec<f32> {
    let mut pooled = vec![0f32; hidden];
    let mut counted = 0f32;
    for t in 0..len {
        if mask.get(t).copied().unwrap_or(0) == 0 {
            continue;
        }
        counted += 1.0;
        for h in 0..hidden {
            pooled[h] += data[t * hidden + h];
        }
    }
    for v in pooled.iter_mut() {
        *v /= counted.max(1.0);
    }
    let norm = pooled.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
    for v in pooled.iter_mut() {
        *v /= norm;
    }
    pooled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_tokens_are_excluded_from_the_mean() {
        // Two tokens of real signal and one of padding that points somewhere else. If
        // the mask were ignored, the result would tilt toward the third row.
        let data = vec![
            1.0, 0.0, // token 0
            1.0, 0.0, // token 1
            0.0, 9.0, // token 2, masked out
        ];
        let pooled = pool(&data, &[1, 1, 0], 3, 2);
        assert!((pooled[0] - 1.0).abs() < 1e-6, "{pooled:?}");
        assert!(
            pooled[1].abs() < 1e-6,
            "padding must not leak in: {pooled:?}"
        );
    }

    #[test]
    fn the_result_is_a_unit_vector() {
        // Cosine over these vectors is computed without re-normalising, and the
        // artefact's quantiser assumes components sit in roughly [-1, 1].
        let pooled = pool(&[3.0, 4.0, 0.0, 0.0], &[1, 1], 2, 2);
        let norm: f32 = pooled.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "{norm}");
    }

    #[test]
    fn an_all_padding_input_does_not_divide_by_zero() {
        // A query of nothing but stop-words the tokenizer drops is not an error, and it
        // must not be NaN either — NaN propagates into every distance it touches.
        let pooled = pool(&[1.0, 2.0], &[0], 1, 2);
        assert!(pooled.iter().all(|v| v.is_finite()), "{pooled:?}");
    }
}
