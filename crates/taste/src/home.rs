//! Home's rails, first version (Phase 14; Phase 18 overhauls Home). What a profile has
//! started, where its taste points, what it saved, and what is widely loved: the last needs
//! nothing but the offline catalogue, so Home is never empty, with no key, no sources and
//! no history. A kids profile's rails hold only what suits children. Empty rails are left
//! out rather than shown empty.

use std::collections::HashSet;

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_search_engine::Semantic;

#[derive(Debug, Clone, PartialEq)]
pub struct Rail {
    /// "continue" | "taste" | "watchlist" | "loved"
    pub key: &'static str,
    pub title: &'static str,
    pub items: Vec<i64>,
}

const PER_RAIL: usize = 20;

pub async fn rails(
    db: &Db,
    semantic: Option<&mut Semantic>,
    profile: i64,
    kids: bool,
    love: Option<&str>,
) -> Result<Vec<Rail>, DbError> {
    let profiles = ProfileRepository::new(db);
    let media = MediaRepository::new(db);
    let suits = |ids: Vec<i64>| async move {
        if !kids {
            return Ok::<_, DbError>(ids);
        }
        let ok = MediaRepository::new(db).suitable_for_children(&ids).await?;
        Ok(ids.into_iter().filter(|id| ok.contains(id)).collect())
    };

    let started: Vec<i64> = profiles
        .continue_watching(profile, PER_RAIL as i64)
        .await?
        .into_iter()
        .map(|p| p.media_item_id)
        .collect();
    let liked = profiles.liked(profile).await?;
    let saved = profiles.watchlist(profile).await?;
    let seen: HashSet<i64> = liked
        .iter()
        .map(|(id, _)| *id)
        .chain(started.iter().copied())
        .collect();

    let taste = match semantic {
        Some(s) => match crate::seed(s, &liked, love) {
            Some(seed) => crate::nearest(s, &seed, PER_RAIL * if kids { 6 } else { 1 }, &seen),
            None => Vec::new(),
        },
        None => Vec::new(),
    };
    let shown: HashSet<i64> = seen
        .iter()
        .chain(taste.iter())
        .chain(saved.iter())
        .copied()
        .collect();
    let loved: Vec<i64> = media
        .most_voted_films(if kids { 2_000 } else { 200 })
        .await?
        .into_iter()
        .filter(|id| !shown.contains(id))
        .collect();

    let mut out = Vec::new();
    for (key, title, ids) in [
        ("continue", "Continue watching", started),
        ("taste", "Where your taste points", taste),
        ("watchlist", "On your watchlist", saved),
        ("loved", "Widely loved", loved),
    ] {
        let items: Vec<i64> = suits(ids).await?.into_iter().take(PER_RAIL).collect();
        if !items.is_empty() {
            out.push(Rail { key, title, items });
        }
    }
    Ok(out)
}
