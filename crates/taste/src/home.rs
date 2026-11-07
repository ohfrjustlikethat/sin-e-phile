//! Home's rails, first version (Phase 14; Phase 18 overhauls Home). What a profile has
//! started, where its taste points, what it saved, and what is widely loved: the last needs
//! nothing but the offline catalogue, so Home is never empty, with no key, no sources and
//! no history. A kids profile's rails hold only what suits children. Empty rails are left
//! out rather than shown empty.

use std::collections::HashSet;

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_search_engine::Semantic;

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let days = secs.div_euclid(86_400);
    // Only the day matters here: a date the model can read.
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

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
    cache: &crate::model::ModelCache,
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

    // The taste model (Phase 15): modes with the popularity prior; "what you love" in the
    // profile's own words joins as a small mode of its own.
    let taste = match semantic {
        Some(s) => {
            let today = crate::model::day_of(&now_iso()).unwrap_or(0.0);
            let (mut modes, mut met) = cache.get(db, s, profile, today).await?;
            met.extend(seen.iter().copied());
            // The profile's own corrections from its taste profile: more, less, not me.
            crate::describe::apply(
                &mut modes,
                &crate::describe::corrections(db, profile).await?,
            );
            if let Some(text) = love.map(str::trim).filter(|t| !t.is_empty()) {
                if let Ok(v) = s.embedder.embed_query(text) {
                    crate::model::add_words(&mut modes, v);
                }
            }
            let pool = media.most_voted_with_votes(3_000).await?;
            let want = PER_RAIL * if kids { 6 } else { 1 };
            crate::model::recommend_with_prior(
                s,
                &modes,
                want,
                &met,
                &pool,
                crate::model::POPULARITY_PRIOR,
            )
        }
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
