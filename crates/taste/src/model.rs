//! A profile's taste model, from its database rows (Phase 15): its signals read, weighed
//! and decayed, its modes built from the liked titles' vectors, and recommendations taken
//! across the modes.

use std::collections::HashSet;

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_search_engine::Semantic;

use crate::modes::{interleave_by_weight, modes, weigh, Mode, Signal};

/// Days since 1970 for "2026-11-07 12:00:00" or "2026-11-07T12:00:00Z".
pub fn day_of(text: &str) -> Option<f64> {
    let d = text.get(..10)?;
    let mut p = d.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (p.next()??, p.next()??, p.next()??);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) as f64)
}

/// Every signal of a profile with its age in days at `today` (days since 1970).
pub async fn signals(
    db: &Db,
    profile: i64,
    today: f64,
) -> Result<Vec<(i64, Signal, f64)>, DbError> {
    let profiles = ProfileRepository::new(db);
    let media = MediaRepository::new(db);
    let age = |t: &str| day_of(t).map_or(0.0, |d| (today - d).max(0.0));
    let mut out = Vec::new();
    for v in profiles.viewings(profile).await? {
        let s = if v.completed {
            Signal::Completed
        } else {
            let runtime = media
                .by_id(v.media_item_id)
                .await?
                .and_then(|m| m.runtime_minutes)
                .map(|r| r as f64 * 60.0);
            Signal::Abandoned {
                at: v.stopped_at_seconds.unwrap_or(v.watched_seconds) as f64,
                runtime,
            }
        };
        out.push((v.media_item_id, s, age(&v.started_at)));
    }
    for (id, stars, at) in profiles.ratings(profile).await? {
        out.push((id, Signal::Rated(stars), age(&at)));
    }
    for id in profiles.watchlist(profile).await? {
        out.push((id, Signal::Watchlisted, 0.0));
    }
    Ok(out)
}

/// The modes of a profile's taste at `today`, and every title it has signals for (so a
/// recommendation never offers what it has already met).
pub async fn build(
    db: &Db,
    semantic: &Semantic,
    profile: i64,
    today: f64,
) -> Result<(Vec<Mode>, HashSet<i64>), DbError> {
    let weighed = weigh(&signals(db, profile, today).await?);
    let seen: HashSet<i64> = weighed.iter().map(|(id, _)| *id).collect();
    let liked: Vec<(i64, Vec<f32>, f64)> = weighed
        .into_iter()
        .filter(|(_, w)| *w > 0.0)
        .filter_map(|(id, w)| {
            let v = semantic.index.vector(id)?;
            Some((id, v.iter().map(|x| f32::from(*x)).collect(), w))
        })
        .collect();
    Ok((modes(&liked), seen))
}

/// Each profile's modes, kept until its signals change (`signal_fingerprint`): building
/// them is k-means six times over hundreds of vectors (about 260 ms for 650 films), and a
/// profile switch should not pay that while nothing has changed.
/// A profile's fingerprint, its modes, and the titles it has met.
type Cached = (String, Vec<Mode>, HashSet<i64>);

#[derive(Default)]
pub struct ModelCache {
    inner: std::sync::Mutex<std::collections::HashMap<i64, Cached>>,
}

impl ModelCache {
    /// The profile's modes and the titles it has met, built now only if its signals moved.
    pub async fn get(
        &self,
        db: &Db,
        semantic: &Semantic,
        profile: i64,
        today: f64,
    ) -> Result<(Vec<Mode>, HashSet<i64>), DbError> {
        let fingerprint = ProfileRepository::new(db)
            .signal_fingerprint(profile)
            .await?;
        if let Some((f, m, met)) = self
            .inner
            .lock()
            .ok()
            .and_then(|c| c.get(&profile).cloned())
        {
            if f == fingerprint {
                return Ok((m, met));
            }
        }
        let (m, met) = build(db, semantic, profile, today).await?;
        if let Ok(mut c) = self.inner.lock() {
            c.insert(profile, (fingerprint, m.clone(), met.clone()));
        }
        Ok((m, met))
    }
}

/// `k` titles across the modes, in proportion to their weight, none already met.
pub fn recommend(semantic: &Semantic, modes: &[Mode], k: usize, seen: &HashSet<i64>) -> Vec<i64> {
    let lists: Vec<(f64, Vec<i64>)> = modes
        .iter()
        .map(|m| (m.weight, crate::nearest(semantic, &m.direction, k, seen)))
        .collect();
    interleave_by_weight(&lists, k)
}

/// How much "what you love", in the profile's own words, counts among the modes.
pub const WORDS_WEIGHT: f64 = 0.2;

/// Add "what you love" as a mode of its own, weighted [`WORDS_WEIGHT`] (or alone, when
/// there is no history yet), the others scaled to make room.
pub fn add_words(modes: &mut Vec<Mode>, vector: Vec<f32>) {
    let n = vector.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    let direction: Vec<f32> = vector.iter().map(|x| x / n).collect();
    let w = if modes.is_empty() { 1.0 } else { WORDS_WEIGHT };
    for m in modes.iter_mut() {
        m.weight *= 1.0 - w;
    }
    modes.push(Mode {
        direction,
        weight: w,
        members: Vec::new(),
    });
}

/// How much being widely seen counts against being close to a taste. Chosen on users who
/// do not judge it (`eval taste --tune`; docs/eval-results.md).
pub const POPULARITY_PRIOR: f64 = 0.5;

/// Rank candidates by closeness to the nearest mode plus `prior` times how widely seen they
/// are (log votes over the most-voted's): a taste picks among films, and the films most
/// people have seen are, other things equal, the likelier favourites. `candidates`:
/// (title, its vector, votes).
pub fn rank(
    modes: &[Mode],
    candidates: &[(i64, Vec<f32>, i64)],
    max_votes: i64,
    prior: f64,
    k: usize,
) -> Vec<i64> {
    let top = ((max_votes.max(1) + 1) as f64).log10();
    let mut scored: Vec<(f64, i64)> = candidates
        .iter()
        .map(|(id, v, votes)| {
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
            let near = modes
                .iter()
                .map(|m| {
                    m.direction
                        .iter()
                        .zip(v)
                        .map(|(a, b)| a * b / n)
                        .sum::<f32>()
                })
                .fold(f32::MIN, f32::max);
            let pop = (((*votes).max(0) + 1) as f64).log10() / top;
            (f64::from(near) + prior * pop, *id)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(k).map(|(_, id)| id).collect()
}

/// [`rank`] over the modes' nearest and the most-voted titles, none already met.
pub fn recommend_with_prior(
    semantic: &Semantic,
    modes: &[Mode],
    k: usize,
    seen: &HashSet<i64>,
    popular: &[(i64, i64)],
    prior: f64,
) -> Vec<i64> {
    let mut pool: Vec<i64> = recommend(semantic, modes, k, seen);
    pool.extend(
        popular
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !seen.contains(id)),
    );
    let votes: std::collections::HashMap<i64, i64> = popular.iter().copied().collect();
    let max_votes = popular.iter().map(|(_, v)| *v).max().unwrap_or(1);
    let mut taken = HashSet::new();
    let candidates: Vec<(i64, Vec<f32>, i64)> = pool
        .into_iter()
        .filter(|id| taken.insert(*id))
        .filter_map(|id| {
            let v = semantic.index.vector(id)?;
            Some((
                id,
                v.iter().map(|x| f32::from(*x)).collect(),
                votes.get(&id).copied().unwrap_or(0),
            ))
        })
        .collect();
    rank(modes, &candidates, max_votes, prior, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closeness_decides_and_popularity_breaks_near_ties() {
        let m = vec![Mode {
            direction: vec![1.0, 0.0],
            weight: 1.0,
            members: vec![],
        }];
        let c = vec![
            (1, vec![1.0, 0.0], 10),          // exactly the taste, barely seen
            (2, vec![0.96, 0.28], 1_000_000), // nearly the taste, seen by everyone
            (3, vec![0.0, 1.0], 1_000_000),   // not the taste, seen by everyone
        ];
        assert_eq!(
            rank(&m, &c, 1_000_000, 0.0, 3),
            vec![1, 2, 3],
            "no prior: closeness alone"
        );
        assert_eq!(
            rank(&m, &c, 1_000_000, 0.3, 2),
            vec![2, 1],
            "a near miss everyone loved comes first"
        );
        assert_eq!(
            rank(&m, &c, 1_000_000, 0.3, 3)[2],
            3,
            "popularity alone does not beat the taste"
        );
    }

    #[test]
    fn stored_times_read_as_days() {
        assert_eq!(day_of("1970-01-02 00:00:00"), Some(1.0));
        assert_eq!(day_of("2026-11-07T12:00:00Z"), Some(20_764.0));
        assert_eq!(day_of("nonsense"), None);
    }
}
