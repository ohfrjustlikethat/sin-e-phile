//! Home's rails (Phase 14, ordered by the author in Phase 16; Phase 19 overhauls Home):
//! what a profile has started; new episodes of the shows it follows; what is in its
//! library; this year's new releases, by its taste; where its taste points; what it saved;
//! and what is widely loved, which needs nothing but the offline catalogue, so Home is never
//! empty, with no key, no sources and no history. A kids profile's rails hold only what
//! suits children. Empty rails are left out rather than shown empty.

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::{
    EpisodeRepository, LibraryRepository, MediaRepository, ProfileRepository,
};
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
    /// "continue" | "new_episodes" | "library" | "new" | "taste" | "watchlist" | "loved"
    pub key: &'static str,
    pub title: &'static str,
    pub items: Vec<i64>,
    /// A line for an item beyond its own facts: "S2 E5" for a series with a new episode.
    pub notes: HashMap<i64, String>,
}

/// New episodes count if the catalogue gained them in this many days.
const NEW_EPISODE_DAYS: i64 = 30;
/// A new release has this many votes: it has come out and people are watching it.
const NEW_RELEASE_VOTES: i64 = 1_000;

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

    // The taste model (Phase 15), with the profile's corrections: built once, for the new
    // releases and the taste rail.
    let today = crate::model::day_of(&now_iso()).unwrap_or(0.0);
    let year = (today / 365.2425 + 1970.0).floor() as i64;
    let mut model = None;
    if let Some(s) = semantic {
        let (mut modes, mut met) = cache.get(db, s, profile, today).await?;
        met.extend(seen.iter().copied());
        crate::describe::apply(
            &mut modes,
            &crate::describe::corrections(db, profile).await?,
        );
        if let Some(text) = love.map(str::trim).filter(|t| !t.is_empty()) {
            if let Ok(v) = s.embedder.embed_query(text) {
                crate::model::add_words(&mut modes, v);
            }
        }
        model = Some((s, modes, met));
    }

    // New episodes of followed series: one card per series, opening the series.
    let mut episode_notes = HashMap::new();
    let new_episodes: Vec<i64> = EpisodeRepository::new(db)
        .new_episodes(profile, NEW_EPISODE_DAYS, year - 1, PER_RAIL as i64)
        .await?
        .into_iter()
        .map(|(series, _, season, number)| {
            let note = match (season, number) {
                (Some(s), Some(n)) => format!("New: S{s} E{n}"),
                _ => "New episode".to_string(),
            };
            episode_notes.insert(series, note);
            series
        })
        .collect();

    // The library, most recently identified first.
    let library = LibraryRepository::new(db)
        .recently_added(PER_RAIL as i64)
        .await?;

    // This year's releases (last year's too while this year's are few), by taste where
    // the model has a vector for them, by votes otherwise. The catalogue has years, not
    // dates; daily refreshes keep the votes moving with what people watch.
    let mut released = media.new_releases(year, NEW_RELEASE_VOTES, 300).await?;
    if released.len() < PER_RAIL {
        released = media.new_releases(year - 1, NEW_RELEASE_VOTES, 300).await?;
    }
    let new_releases: Vec<i64> = match &model {
        Some((s, modes, met)) if !modes.is_empty() => {
            let fresh: Vec<&(i64, i64)> = released
                .iter()
                .filter(|(id, _)| !met.contains(id))
                .collect();
            let max = fresh.iter().map(|(_, v)| *v).max().unwrap_or(1);
            let with: Vec<(i64, Vec<f32>, i64)> = fresh
                .iter()
                .filter_map(|(id, v)| {
                    let vec = s.index.vector(*id)?;
                    Some((*id, vec.iter().map(|x| f32::from(*x)).collect(), *v))
                })
                .collect();
            let mut ranked = crate::model::rank(
                modes,
                &with,
                max,
                crate::model::POPULARITY_PRIOR,
                PER_RAIL * 6,
            );
            let have: HashSet<i64> = ranked.iter().copied().collect();
            ranked.extend(
                fresh
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| !have.contains(id)),
            );
            ranked
        }
        _ => released
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !seen.contains(id))
            .collect(),
    };

    let taste = match &model {
        Some((s, modes, met)) => {
            let mut met = met.clone();
            met.extend(new_releases.iter().take(PER_RAIL).copied());
            let pool = media.most_voted_with_votes(3_000).await?;
            let want = PER_RAIL * if kids { 6 } else { 1 };
            crate::model::recommend_with_prior(
                s,
                modes,
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
        .chain(new_releases.iter().take(PER_RAIL))
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
        ("new_episodes", "New episodes of your shows", new_episodes),
        ("library", "Your library", library),
        ("new", "New releases", new_releases),
        ("taste", "Where your taste points", taste),
        ("watchlist", "On your watchlist", saved),
        ("loved", "Widely loved", loved),
    ] {
        let items: Vec<i64> = suits(ids).await?.into_iter().take(PER_RAIL).collect();
        if !items.is_empty() {
            let notes = if key == "new_episodes" {
                items
                    .iter()
                    .filter_map(|id| episode_notes.get(id).map(|n| (*id, n.clone())))
                    .collect()
            } else {
                HashMap::new()
            };
            out.push(Rail {
                key,
                title,
                items,
                notes,
            });
        }
    }
    Ok(out)
}
