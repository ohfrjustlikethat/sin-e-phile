//! Onboarding's title-card rounds (SPEC.md Phase 14): "pick any you love", three or four
//! times, each round adapted to the picks before it.
//!
//! - **Spread for information gain.** A round is drawn decade by decade, interleaved, from
//!   the films most people know, so one round spans the 1920s to now: a pick from a spread
//!   says more about a person than a pick among twelve near-identical titles would.
//! - **Adapted.** Once something is picked (and there is a vector half), half the next round
//!   is the titles nearest the taste so far, to confirm or refine it; the other half keeps
//!   spreading, so one early pick does not narrow everything after it.
//!
//! What has been shown is never shown again; a kids profile sees only what suits children.

use std::collections::HashSet;

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_search_engine::Semantic;

/// The decades a round spreads across, newest first.
pub const DECADES: [i64; 11] = [
    2020, 2010, 2000, 1990, 1980, 1970, 1960, 1950, 1940, 1930, 1920,
];

/// Interleave lists: the first of each, then the second of each, and so on.
pub fn interleave(lists: &[Vec<i64>]) -> Vec<i64> {
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..longest {
        for l in lists {
            if let Some(id) = l.get(i) {
                out.push(*id);
            }
        }
    }
    out
}

pub async fn round(
    db: &Db,
    semantic: Option<&mut Semantic>,
    profile: i64,
    kids: bool,
    shown: &HashSet<i64>,
    size: usize,
) -> Result<Vec<i64>, DbError> {
    let media = MediaRepository::new(db);
    let liked = ProfileRepository::new(db).liked(profile).await?;
    let avoid: HashSet<i64> = shown
        .iter()
        .copied()
        .chain(liked.iter().map(|(id, _)| *id))
        .collect();
    let per_decade = if kids { 120 } else { 12 };

    let mut decades = Vec::new();
    for d in DECADES {
        let ids = media.most_voted_films_between(d, d + 9, per_decade).await?;
        decades.push(
            ids.into_iter()
                .filter(|id| !avoid.contains(id))
                .collect::<Vec<_>>(),
        );
    }
    let mut spread = interleave(&decades);
    let mut near = match (semantic, liked.is_empty()) {
        (Some(s), false) => match crate::seed(s, &liked, None) {
            Some(seed) => crate::nearest(s, &seed, size * if kids { 6 } else { 2 }, &avoid),
            None => Vec::new(),
        },
        _ => Vec::new(),
    };
    if kids {
        let all: Vec<i64> = spread.iter().chain(near.iter()).copied().collect();
        let ok = media.suitable_for_children(&all).await?;
        spread.retain(|id| ok.contains(id));
        near.retain(|id| ok.contains(id));
    }

    // Half near the taste so far (when there is one), the rest spread; never twice.
    let mut out: Vec<i64> = near.into_iter().take(size / 2).collect();
    let taken: HashSet<i64> = out.iter().copied().collect();
    out.extend(
        spread
            .into_iter()
            .filter(|id| !taken.contains(id))
            .take(size - out.len()),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaving_takes_one_from_each_in_turn() {
        assert_eq!(
            interleave(&[vec![1, 2, 3], vec![10], vec![20, 21]]),
            vec![1, 10, 20, 2, 21, 3]
        );
        assert!(interleave(&[]).is_empty());
    }
}
