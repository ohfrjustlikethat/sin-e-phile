//! Home's taste rail through Phase 17's hybrid ranker (`sinephile_recommend::rank`).
//!
//! This module gathers and describes; the ranker decides. About a thousand candidates come
//! from three places, each capped so none crowds out the others:
//!
//! - **Meaning** (Layer 1): the titles nearest each taste mode in the meaning index.
//! - **People who loved the same** (Layer 2): the stored neighbours of the titles the
//!   profile loved most, from the item-item matrix.
//! - **Widely seen**: the most-voted titles, so a thin profile still has a rail.
//!
//! Each is then described as the ranker needs it (closeness to the nearest mode, which liked
//! titles point to it, votes, year, directors, franchise, whether it plays now, the axes),
//! and every pick comes back with a reason that [`note`] says in words.

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::{Facts, MediaRepository, NeighbourRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_recommend::rank::{self, Candidate, Pick, Reason};
use sinephile_search_engine::Semantic;

use crate::axes::{decade, length_band, Axes};
use crate::modes::Mode;

/// Nearest titles asked for per mode, and the most the meaning pool keeps.
const PER_MODE: usize = 200;
const MEANING: usize = 500;
/// Liked titles whose neighbours are asked for, strongest first, and the most kept. Asking
/// for only 60 left most of a long history out, and cost about a fifth of the recall on
/// MovieLens' tuning users (docs/eval-results.md); 500 is one query of about 25,000 rows.
const LIKED_ASKED: usize = 500;
const LOVED: usize = 400;
/// The most-voted titles in the pool.
const POPULAR: i64 = 100;

/// The profile's axes on one title, -1 to 1: shares of its decade, length, best genre and
/// best director among what the profile liked, less its strongest avoided genre or director.
pub fn axis_score(axes: &Axes, f: &Facts) -> f32 {
    let share = |list: &[(String, f64)], v: &str| {
        list.iter().find(|(x, _)| x == v).map_or(0.0, |(_, s)| *s)
    };
    let mut up = 0.0;
    if let Some(y) = f.year {
        up += share(&axes.decades, &decade(y).to_string());
    }
    if let Some(m) = f.runtime_minutes {
        up += share(&axes.lengths, length_band(m));
    }
    up += f
        .genres
        .iter()
        .map(|g| share(&axes.genres, g))
        .fold(0.0, f64::max);
    up += f
        .directors
        .iter()
        .map(|(_, n)| share(&axes.directors, n))
        .fold(0.0, f64::max);
    let down = f
        .genres
        .iter()
        .map(String::as_str)
        .chain(f.directors.iter().map(|(_, n)| n.as_str()))
        .map(|x| share(&axes.avoids, x))
        .fold(0.0, f64::max);
    ((up - down) as f32).clamp(-1.0, 1.0)
}

/// What the rail needs of the meaning index: the titles nearest a direction, and a title's
/// vector. The real index ([`Semantic`]) in the app; a few hand-made vectors in a test, since
/// the real one needs the language model's files.
pub trait Meaning {
    fn nearest(&self, direction: &[f32], k: usize, exclude: &HashSet<i64>) -> Vec<i64>;
    fn vector(&self, id: i64) -> Option<Vec<f32>>;
}

impl Meaning for Semantic {
    fn nearest(&self, direction: &[f32], k: usize, exclude: &HashSet<i64>) -> Vec<i64> {
        crate::nearest(self, direction, k, exclude)
    }

    fn vector(&self, id: i64) -> Option<Vec<f32>> {
        self.index
            .vector(id)
            .map(|v| v.iter().map(|x| f32::from(*x)).collect())
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

/// The profile's best-loved titles, strongest first: the ones whose neighbours are asked for.
pub fn strongest(liked: &[(i64, f64)]) -> Vec<i64> {
    strongest_n(liked, LIKED_ASKED)
}

/// [`strongest`], `n` of them: the evaluation tries other counts.
pub fn strongest_n(liked: &[(i64, f64)], n: usize) -> Vec<i64> {
    let mut by: Vec<(i64, f64)> = liked.iter().copied().filter(|(_, w)| *w > 0.0).collect();
    by.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    by.into_iter().take(n).map(|(id, _)| id).collect()
}

/// The stored neighbours of the best-loved titles, by the title they point to: each with the
/// loved titles that point there and the matrix's score. Titles already met are left out.
pub async fn loved_with(
    db: &Db,
    liked: &[(i64, f64)],
    met: &HashSet<i64>,
) -> Result<HashMap<i64, Vec<(i64, f32)>>, DbError> {
    let mut out: HashMap<i64, Vec<(i64, f32)>> = HashMap::new();
    for (from, to, score) in NeighbourRepository::new(db)
        .of_many(&strongest(liked))
        .await?
    {
        if !met.contains(&to) {
            out.entry(to).or_default().push((from, score));
        }
    }
    Ok(out)
}

/// A rail of `k` for a profile: `modes` and `met` from the taste model, `liked` its weighed
/// titles (`modes::weigh`), `playable` what is in its library now.
#[allow(clippy::too_many_arguments)]
pub async fn rail(
    db: &Db,
    semantic: &mut (dyn Meaning + Send),
    modes: &[Mode],
    met: &HashSet<i64>,
    liked: &[(i64, f64)],
    axes: &Axes,
    playable: &HashSet<i64>,
    k: usize,
) -> Result<Vec<Pick>, DbError> {
    let loved = loved_with(db, liked, met).await?;
    let candidates = candidates(db, semantic, modes, met, &loved, axes, playable).await?;
    Ok(rank::rank(
        &candidates,
        &rank::WEIGHTS,
        &rank::CAPS,
        rank::DIVERSITY,
        k,
    ))
}

/// The described candidates [`rail`] ranks, with the neighbours given rather than read: the
/// evaluation passes a matrix built without the users it judges, and ranks the same
/// candidates under each setting it tries (`eval recommend --whole`).
pub async fn candidates(
    db: &Db,
    semantic: &mut (dyn Meaning + Send),
    modes: &[Mode],
    met: &HashSet<i64>,
    loved_with: &HashMap<i64, Vec<(i64, f32)>>,
    axes: &Axes,
    playable: &HashSet<i64>,
) -> Result<Vec<Candidate>, DbError> {
    // Meaning: each mode's nearest, mode by mode so a small mode is not drowned.
    let mut pool: Vec<i64> = Vec::new();
    let mut seen: HashSet<i64> = met.clone();
    let lists: Vec<Vec<i64>> = modes
        .iter()
        .map(|m| semantic.nearest(&m.direction, PER_MODE, met))
        .collect();
    for at in 0..PER_MODE {
        for list in &lists {
            if pool.len() >= MEANING {
                break;
            }
            if let Some(&id) = list.get(at) {
                if seen.insert(id) {
                    pool.push(id);
                }
            }
        }
    }

    // People who loved the same: the neighbours of the best-loved titles.
    let mut by_love: Vec<(i64, f32)> = loved_with
        .iter()
        .map(|(id, from)| (*id, from.iter().map(|(_, s)| s).sum()))
        .collect();
    by_love.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut added = 0;
    for (id, _) in by_love {
        if added >= LOVED {
            break;
        }
        if seen.insert(id) {
            pool.push(id);
            added += 1;
        }
    }

    // Widely seen.
    let media = MediaRepository::new(db);
    for (id, _) in media.most_voted_with_votes(POPULAR).await? {
        if seen.insert(id) {
            pool.push(id);
        }
    }

    let facts: HashMap<i64, Facts> = media
        .facts(&pool)
        .await?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let candidates: Vec<Candidate> = pool
        .iter()
        .filter_map(|id| facts.get(id))
        .map(|f| {
            let vector = semantic.vector(f.id).map(|v| unit(&v)).unwrap_or_default();
            let near = (!vector.is_empty())
                .then(|| {
                    modes
                        .iter()
                        .enumerate()
                        .map(|(m, mode)| {
                            let c: f32 =
                                mode.direction.iter().zip(&vector).map(|(a, b)| a * b).sum();
                            (m, c)
                        })
                        .max_by(|a, b| a.1.total_cmp(&b.1))
                })
                .flatten();
            Candidate {
                id: f.id,
                near,
                loved_with: loved_with.get(&f.id).cloned().unwrap_or_default(),
                votes: f.votes.unwrap_or(0),
                year: f.year,
                directors: f.directors.iter().map(|(p, _)| *p).collect(),
                franchise: rank::franchise_key(&f.title),
                playable: playable.contains(&f.id),
                axes: axis_score(axes, f),
                vector,
            }
        })
        .collect();
    Ok(candidates)
}

/// A pick's reason in words, naming only what is true of the profile (E4): the titles it
/// loved that point here, or the loved titles its taste mode is made of.
pub fn note(pick: &Pick, modes: &[Mode], titles: &HashMap<i64, String>) -> String {
    let named = |ids: &[i64]| -> String {
        let names: Vec<&str> = ids
            .iter()
            .filter_map(|id| titles.get(id).map(String::as_str))
            .collect();
        match names.as_slice() {
            [] => String::new(),
            [one] => (*one).to_string(),
            [init @ .., last] => format!("{} and {last}", init.join(", ")),
        }
    };
    match &pick.reason {
        Reason::LovedWith(ids) => {
            let who = named(ids);
            if who.is_empty() {
                "Loved by people who share your taste".into()
            } else {
                format!("Loved by people who loved {who}")
            }
        }
        Reason::Mode(m) => {
            let members: Vec<i64> = modes
                .get(*m)
                .map(|mode| mode.members.iter().take(2).copied().collect())
                .unwrap_or_default();
            let like = named(&members);
            if like.is_empty() {
                "Close to your taste".into()
            } else {
                format!("Close to {like}, which you loved")
            }
        }
        Reason::Popular => "Widely loved".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes() -> Axes {
        Axes {
            decades: vec![("1970".into(), 0.5)],
            lengths: vec![("long".into(), 0.3)],
            genres: vec![("Crime".into(), 0.4)],
            directors: vec![("Michael Mann".into(), 0.2)],
            avoids: vec![("Horror".into(), 0.6)],
        }
    }

    #[test]
    fn the_axes_raise_what_was_liked_and_lower_what_was_avoided() {
        let liked = Facts {
            year: Some(1974),
            runtime_minutes: Some(150),
            genres: vec!["Crime".into()],
            directors: vec![(1, "Michael Mann".into())],
            ..Default::default()
        };
        assert!(
            (axis_score(&axes(), &liked) - 1.0).abs() < 1e-6,
            "0.5 + 0.3 + 0.4 + 0.2, capped at 1"
        );
        let avoided = Facts {
            year: Some(1974),
            genres: vec!["Horror".into()],
            ..Default::default()
        };
        assert!(
            (axis_score(&axes(), &avoided) - (-0.1)).abs() < 1e-6,
            "0.5 less 0.6"
        );
        assert_eq!(axis_score(&axes(), &Facts::default()), 0.0);
    }

    #[test]
    fn a_note_names_only_titles_the_profile_loved() {
        let titles: HashMap<i64, String> = [(1, "Stalker"), (2, "Solaris"), (3, "Mirror")]
            .map(|(i, t)| (i, t.to_string()))
            .into();
        let modes = vec![Mode {
            direction: vec![1.0],
            weight: 1.0,
            members: vec![1, 2, 3],
        }];
        let pick = |reason| Pick {
            id: 9,
            score: 1.0,
            reason,
        };
        assert_eq!(
            note(&pick(Reason::LovedWith(vec![2, 1, 3])), &modes, &titles),
            "Loved by people who loved Solaris, Stalker and Mirror"
        );
        assert_eq!(
            note(&pick(Reason::LovedWith(vec![1])), &modes, &titles),
            "Loved by people who loved Stalker"
        );
        assert_eq!(
            note(&pick(Reason::Mode(0)), &modes, &titles),
            "Close to Stalker and Solaris, which you loved"
        );
        assert_eq!(
            note(&pick(Reason::Mode(5)), &modes, &titles),
            "Close to your taste"
        );
        assert_eq!(
            note(&pick(Reason::Popular), &modes, &titles),
            "Widely loved"
        );
    }
}
