//! Taste (SPEC.md Phase 14, built on in Phase 15).
//!
//! - [`Statements`]: six things a person can say about what they like, none of them a
//!   genre (pace, ambiguity, era, subtitles, runtime, form). Stored per profile.
//! - [`seed`]: the profile's taste as one direction in the catalogue's embedding space: the
//!   weighted mean of the vectors of what they liked, with "what you love" in their own
//!   words embedded and counted alongside. Phase 15 splits this into several modes; one
//!   seed is enough to show that an import is reflected and to start a Home screen.

pub mod home;
pub mod rounds;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sinephile_search_engine::Semantic;

/// One statement: -1, 0 or 1 along its axis; `None` when not answered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statements {
    /// -1 slow and patient · 1 brisk.
    pub pace: Option<i8>,
    /// -1 left open · 1 tied up.
    pub ambiguity: Option<i8>,
    /// -1 older films · 1 recent ones.
    pub era: Option<i8>,
    /// -1 rather not read them · 1 happily.
    pub subtitles: Option<i8>,
    /// -1 short · 1 long is welcome.
    pub runtime: Option<i8>,
    /// -1 classical · 1 experimental.
    pub form: Option<i8>,
}

impl Statements {
    /// From the stored JSON; anything unreadable, or outside -1..=1, is unanswered.
    pub fn parse(json: Option<&str>) -> Self {
        let s: Self = json
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let ok = |v: Option<i8>| v.filter(|x| (-1..=1).contains(x));
        Self {
            pace: ok(s.pace),
            ambiguity: ok(s.ambiguity),
            era: ok(s.era),
            subtitles: ok(s.subtitles),
            runtime: ok(s.runtime),
            form: ok(s.form),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// How many were answered.
    pub fn answered(&self) -> usize {
        [
            self.pace,
            self.ambiguity,
            self.era,
            self.subtitles,
            self.runtime,
            self.form,
        ]
        .iter()
        .filter(|v| v.is_some())
        .count()
    }
}

/// The unit-length weighted mean of `vectors`, `None` if there is nothing to average.
/// Each vector is normalised first, so a long vector does not outvote a short one.
pub fn mean(vectors: &[(Vec<f32>, f64)]) -> Option<Vec<f32>> {
    let dims = vectors.first()?.0.len();
    let mut sum = vec![0f64; dims];
    let mut total = 0.0;
    for (v, w) in vectors {
        if v.len() != dims || *w <= 0.0 {
            continue;
        }
        let norm = v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
        if norm == 0.0 {
            continue;
        }
        for (s, x) in sum.iter_mut().zip(v) {
            *s += w * f64::from(*x) / norm;
        }
        total += w;
    }
    if total == 0.0 {
        return None;
    }
    let norm = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
    (norm > 0.0).then(|| sum.iter().map(|x| (x / norm) as f32).collect())
}

/// How much "what you love" counts against one five-star film: as much as three.
pub const LOVE_WEIGHT: f64 = 3.0;

/// A profile's seed taste vector: the catalogue vectors of what it liked (with their
/// weights from `ProfileRepository::liked`), and "what you love" embedded as a query.
/// `None` when there is nothing to go on, or no vector half (Tier 0 before the artefact).
pub fn seed(semantic: &mut Semantic, liked: &[(i64, f64)], love: Option<&str>) -> Option<Vec<f32>> {
    let mut vectors: Vec<(Vec<f32>, f64)> = liked
        .iter()
        .filter_map(|(id, w)| {
            let v = semantic.index.vector(*id)?;
            Some((v.iter().map(|x| f32::from(*x)).collect(), *w))
        })
        .collect();
    if let Some(text) = love.map(str::trim).filter(|t| !t.is_empty()) {
        if let Ok(v) = semantic.embedder.embed_query(text) {
            vectors.push((v, LOVE_WEIGHT));
        }
    }
    mean(&vectors)
}

/// The `k` catalogue items nearest a seed, leaving out `exclude` (what was already seen).
pub fn nearest(semantic: &Semantic, seed: &[f32], k: usize, exclude: &HashSet<i64>) -> Vec<i64> {
    let q = sinephile_embedding::quantise::quantise(seed);
    semantic
        .index
        .search(&q.values, k + exclude.len().min(2_000))
        .map(|hits| {
            hits.into_iter()
                .map(|n| n.media_item_id)
                .filter(|id| !exclude.contains(id))
                .take(k)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_round_trip_and_nonsense_is_unanswered() {
        let s = Statements {
            pace: Some(-1),
            form: Some(1),
            ..Default::default()
        };
        assert_eq!(Statements::parse(Some(&s.to_json())), s);
        assert_eq!(s.answered(), 2);
        assert_eq!(
            Statements::parse(Some(r#"{"pace": 7, "era": -1}"#)).pace,
            None
        );
        assert_eq!(
            Statements::parse(Some(r#"{"pace": 7, "era": -1}"#)).era,
            Some(-1)
        );
        assert_eq!(Statements::parse(Some("not json")), Statements::default());
        assert_eq!(Statements::parse(None).answered(), 0);
    }

    #[test]
    fn the_mean_points_between_what_was_liked_and_leans_to_the_weightier() {
        let m = mean(&[(vec![1.0, 0.0], 1.0), (vec![0.0, 10.0], 1.0)]).unwrap();
        // Normalised first: the long vector does not outvote the short one.
        assert!((m[0] - m[1]).abs() < 1e-6, "{m:?}");
        assert!(
            ((m[0] * m[0] + m[1] * m[1]).sqrt() - 1.0).abs() < 1e-6,
            "unit length"
        );
        let leaning = mean(&[(vec![1.0, 0.0], 1.0), (vec![0.0, 1.0], 0.3)]).unwrap();
        assert!(leaning[0] > leaning[1]);
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[(vec![0.0, 0.0], 1.0)]), None, "nothing to point at");
    }
}
