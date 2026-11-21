//! Mood shelves (SPEC.md Phase 18, ADR-0043): Home rows by mood ("Slow and beautiful", "Tense,
//! under 100 minutes") found by meaning, not by genre.
//!
//! Each mood is a sentence asked of the meaning index as a query is. Its shelf is the nearest
//! titles to that sentence, ranked by [`score`]: closeness to the mood first, then to the
//! profile's own tastes (so two people's "Slow and beautiful" differ), and a small pull towards
//! the widely seen, since plot-first documents let obscure plots crowd out the films a mood
//! means (docs/eval-results.md, v3). Two moods a day, in turn ([`today`]), so Home does not grow
//! without end. What the profile has met, and what another rail shows, is left out.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use sinephile_persistence::repositories::{MatchReason, MediaRepository, SearchRepository};
use sinephile_persistence::{Db, DbError};

use crate::hybrid::Meaning;
use crate::modes::Mode;

/// One mood: its shelf's name, the sentence asked of the meaning index, and a length limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mood {
    pub key: &'static str,
    pub title: &'static str,
    pub sentence: &'static str,
    pub max_minutes: Option<i64>,
}

/// The sentences describe and avoid words a title might carry: "epic" put *Epic Movie* on
/// "Big and sweeping", and "quiet" put *A Quiet Place* on "Quietly devastating" (the real
/// window, 2026-11-20), since a document begins with its title.
pub const MOODS: [Mood; 6] = [
    Mood {
        key: "mood_slow",
        title: "Slow and beautiful",
        sentence: "a contemplative, visually beautiful film of long, unhurried takes",
        max_minutes: None,
    },
    Mood {
        key: "mood_tense",
        title: "Tense, under 100 minutes",
        sentence: "a tense, suspenseful thriller of mounting dread",
        max_minutes: Some(100),
    },
    Mood {
        key: "mood_kind",
        title: "Funny and kind",
        sentence: "a warm, gentle, kind-hearted comedy",
        max_minutes: None,
    },
    Mood {
        key: "mood_strange",
        title: "Strange and dreamlike",
        sentence: "a surreal, dreamlike and mysterious film",
        max_minutes: None,
    },
    Mood {
        key: "mood_sweeping",
        title: "Big and sweeping",
        sentence: "a grand adventure across vast landscapes and many years",
        max_minutes: None,
    },
    Mood {
        key: "mood_devastating",
        title: "Quietly devastating",
        sentence: "an understated, heartbreaking drama of loss and longing",
        max_minutes: None,
    },
];

/// Shelves a Home shows.
pub const PER_DAY: usize = 2;
/// Nearest titles a shelf chooses from.
const POOL: usize = 300;
/// How much closeness to the profile's nearest taste counts beside closeness to the mood: a
/// quarter, so the shelf stays the mood's and two people's still differ.
const TASTE: f32 = 0.25;
/// How much being widely seen counts (`ln(1 + votes)`, over the most among the pool): search by
/// meaning's own weight, chosen on its tuning set (`POPULARITY_PRIOR`), since a mood is a
/// meaning query. (Scaled closeness and a fifth for popularity, the first version, put a
/// 1912 short at the top of "Big and sweeping" in the real window, 2026-11-20.)
const POPULAR: f32 = sinephile_search_engine::POPULARITY_PRIOR as f32;
/// The fewest votes a title on a shelf has.
const MIN_VOTES: i64 = 1_000;

/// Today's moods: [`PER_DAY`] of [`MOODS`] in turn, `day` days since 1970.
pub fn today(day: i64) -> Vec<Mood> {
    let at = day.rem_euclid(MOODS.len() as i64) as usize;
    (0..PER_DAY)
        .map(|i| MOODS[(at + i) % MOODS.len()])
        .collect()
}

/// A title's place on a mood's shelf, from its cosine to the mood and to the profile's nearest
/// taste (none without one), and how widely seen it is, 0 to 1.
pub fn score(to_mood: f32, to_taste: Option<f32>, seen: f32) -> f32 {
    to_mood + TASTE * to_taste.unwrap_or(0.0) + POPULAR * seen
}

/// Each mood's sentence as the meaning index's query, embedded once per run of the app.
#[derive(Default)]
pub struct MoodVectors {
    inner: Mutex<HashMap<&'static str, Vec<f32>>>,
}

impl MoodVectors {
    /// The mood's vector, embedding it the first time with `embed`.
    pub fn get(
        &self,
        mood: &Mood,
        embed: impl FnOnce(&str) -> Option<Vec<f32>>,
    ) -> Option<Vec<f32>> {
        if let Some(v) = self.inner.lock().ok()?.get(mood.key) {
            return Some(v.clone());
        }
        let v = embed(mood.sentence)?;
        if let Ok(mut m) = self.inner.lock() {
            m.insert(mood.key, v.clone());
        }
        Some(v)
    }
}

fn unit(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n == 0.0 {
        Vec::new()
    } else {
        v.iter().map(|x| x / n).collect()
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// One mood's shelf of up to `k`: films, within its length, none of `taken`.
pub async fn shelf(
    db: &Db,
    semantic: &mut (dyn Meaning + Send),
    mood: &Mood,
    vector: &[f32],
    modes: &[Mode],
    taken: &HashSet<i64>,
    k: usize,
) -> Result<Vec<i64>, DbError> {
    let direction = unit(vector);
    let near = semantic.nearest(&direction, POOL, taken);
    // Films only: a mood is an evening's.
    let films: HashSet<i64> = SearchRepository::new(db)
        .describe(&near, MatchReason::Semantic)
        .await?
        .into_iter()
        .filter(|h| h.kind == "film" || h.kind == "anime_film")
        .map(|h| h.media_item_id)
        .collect();
    let near: Vec<i64> = near.into_iter().filter(|id| films.contains(id)).collect();
    let facts = MediaRepository::new(db).facts(&near).await?;
    let facts: HashMap<i64, _> = facts.into_iter().map(|f| (f.id, f)).collect();
    let most = near
        .iter()
        .filter_map(|id| facts.get(id).and_then(|f| f.votes))
        .max()
        .unwrap_or(0);
    let scale = ((most.max(0) + 1) as f32).ln().max(1e-6);
    // Each title's closeness to the mood and to the nearest taste, and how widely seen.
    let raw: Vec<(i64, f32, Option<f32>, f32)> = near
        .iter()
        .filter_map(|id| {
            let f = facts.get(id)?;
            if f.votes.unwrap_or(0) < MIN_VOTES {
                return None;
            }
            if let Some(max) = mood.max_minutes {
                if !f.runtime_minutes.is_some_and(|m| m <= max) {
                    return None;
                }
            }
            let v = unit(&semantic.vector(*id)?);
            let to_taste = modes
                .iter()
                .map(|m| dot(&m.direction, &v))
                .fold(None, |best: Option<f32>, c| {
                    Some(best.map_or(c, |b| b.max(c)))
                });
            let seen = ((f.votes.unwrap_or(0).max(0) + 1) as f32).ln() / scale;
            Some((*id, dot(&direction, &v), to_taste, seen))
        })
        .collect();
    let mut scored: Vec<(f32, i64)> = raw
        .iter()
        .map(|(id, to_mood, to_taste, seen)| (score(*to_mood, *to_taste, *seen), *id))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    Ok(scored.into_iter().take(k).map(|(_, id)| id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_moods_a_day_in_turn_every_one_in_a_week() {
        let mut seen = HashSet::new();
        for day in 0..MOODS.len() as i64 {
            let t = today(day);
            assert_eq!(t.len(), PER_DAY);
            assert_ne!(t[0].key, t[1].key);
            seen.insert(t[0].key);
        }
        assert_eq!(seen.len(), MOODS.len());
        assert_eq!(
            today(-1)[0].key,
            MOODS[MOODS.len() - 1].key,
            "days before 1970 too"
        );
        let keys: HashSet<&str> = MOODS.iter().map(|m| m.key).collect();
        assert_eq!(keys.len(), MOODS.len(), "every mood its own rail key");
    }

    #[test]
    fn the_mood_leads_and_taste_and_popularity_break_its_ties() {
        // Clearly nearer the mood beats nearer the taste at equal fame; at equal closeness the
        // taste and then the widely seen decide.
        assert!(score(0.9, Some(0.3), 0.5) > score(0.6, Some(0.8), 0.5));
        assert!(score(0.7, Some(0.6), 0.0) > score(0.7, Some(0.2), 0.0));
        assert!(score(0.7, None, 1.0) > score(0.7, None, 0.1));
    }

    #[test]
    fn a_mood_is_embedded_once() {
        let cache = MoodVectors::default();
        let mut asked = 0;
        for _ in 0..3 {
            let v = cache.get(&MOODS[0], |_| {
                asked += 1;
                Some(vec![1.0, 0.0])
            });
            assert_eq!(v, Some(vec![1.0, 0.0]));
        }
        assert_eq!(asked, 1);
    }
}
