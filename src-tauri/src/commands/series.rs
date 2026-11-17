//! A series' page (SPEC.md Phase 11): seasons, episodes, and when the next one airs.
//! **Thin**: the queries are `EpisodeRepository`'s; this converts and, for anime, asks
//! AniList for a fresh schedule now and then.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_metadata_api::{AniList, Transport};
use sinephile_persistence::repositories::{
    EpisodeRepository, EpisodeRow, LibraryRepository, MediaRepository, ProfileRepository,
    WatchScope,
};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SeasonView {
    /// `None`: episodes without a season number.
    pub number: Option<i32>,
    pub episodes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AiringView {
    /// AniList's own number for the episode (ADR-0031: never converted).
    pub episode: i32,
    /// Unix seconds.
    pub airs_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SeriesView {
    /// Episodes with a file on this PC (Phase 16).
    pub on_this_pc: i32,
    /// The show's own files the catalogue has no episode for, as their names number
    /// them, in order (`sinephile_library::shelf`).
    pub also_here: Vec<ShelvedView>,
    pub seasons: Vec<SeasonView>,
    pub next_airing: Option<AiringView>,
    /// How many of its episodes this profile has seen through (Appendix F, I21).
    pub watched: i32,
    /// The episode this profile is in the middle of, or the one after the last it
    /// finished: the page's Resume (Phase 16). `None` until one is started.
    pub up_next: Option<UpNextView>,
}

/// The episode a show's page offers to resume (`sinephile_taste::continuing`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct UpNextView {
    pub id: i32,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    /// "S1 E2 · Traces to Nowhere", or "Next: S1 E3".
    pub line: String,
    /// How far into it, 0 to 1; `None` when it starts from the beginning.
    pub progress: Option<f64>,
}

/// A file of the show's, placed by its own name rather than by the catalogue.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ShelvedView {
    pub path: String,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct EpisodeView {
    pub id: i32,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub title: String,
    pub air_date: Option<String>,
    /// Seen through by this profile. Only a season's list fills it in.
    pub watched: bool,
    /// A file of this episode is on this PC (Phase 16).
    pub on_this_pc: bool,
}

impl From<EpisodeRow> for EpisodeView {
    fn from(e: EpisodeRow) -> Self {
        Self {
            id: e.media_item_id as i32,
            season: e.season_number.map(|n| n as i32),
            episode: e.episode_number.map(|n| n as i32),
            title: e.title,
            air_date: e.air_date,
            watched: false,
            on_this_pc: false,
        }
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// When each series' schedule was last asked for: at most hourly, so opening a page
/// twice does not ask AniList twice.
static ASKED: Mutex<Option<HashMap<i64, Instant>>> = Mutex::new(None);
const ASK_EVERY: Duration = Duration::from_secs(3600);

#[tauri::command]
#[specta::specta]
pub async fn series_detail(state: State<'_, AppState>, id: i32) -> Result<SeriesView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let series = i64::from(id);
    let episodes = EpisodeRepository::new(&db);

    // An anime with an AniList id: refresh its schedule in the background (ADR-0030).
    let anilist = MediaRepository::new(&db)
        .external_id(series, "anilist")
        .await
        .map_err(err)?
        .and_then(|v| v.parse::<i64>().ok());
    let due = {
        let mut asked = ASKED.lock().expect("asked");
        let map = asked.get_or_insert_with(HashMap::new);
        let due = map.get(&series).is_none_or(|t| t.elapsed() > ASK_EVERY);
        if due {
            map.insert(series, Instant::now());
        }
        due
    };
    if let (Some(anilist_id), true) = (anilist, due) {
        let http: Arc<dyn Transport> = state.http.clone();
        let db = db.clone();
        tauri::async_runtime::spawn(async move {
            let client = AniList::owned(http).await;
            if let Ok(Some(media)) = client.media(anilist_id).await {
                if let Some(next) = media.next_airing {
                    let _ = EpisodeRepository::new(&db)
                        .set_next_airing(series, next.episode, next.airing_at)
                        .await;
                }
            }
        });
    }

    let seasons = episodes.seasons(series).await.map_err(err)?;
    // Seen so far, season by season: a series has a handful of seasons.
    let mut watched = 0;
    if let Ok(profile) = state.profile().await {
        let profiles = ProfileRepository::new(&db);
        for (number, _) in &seasons {
            watched += profiles
                .finished_in_season(profile, series, *number)
                .await
                .map_err(err)?
                .len();
        }
    }

    let library = LibraryRepository::new(&db);
    let on_this_pc = library
        .episodes_on_this_pc(series)
        .await
        .map_err(err)?
        .len();
    let own: Vec<(i64, String)> = library
        .files_for_item(series)
        .await
        .map_err(err)?
        .into_iter()
        .map(|f| (f.id, f.path))
        .collect();
    let also_here = sinephile_library::shelf::in_order(&own)
        .into_iter()
        .map(|s| ShelvedView {
            path: s.path,
            season: s.season.map(|n| n as i32),
            episode: s.episode.map(|n| n as i32),
            title: s.title,
        })
        .collect();

    let up_next = match state.profile().await {
        Ok(profile) => sinephile_taste::continuing::for_series(&db, profile, series)
            .await
            .map_err(err)?
            .map(|c| UpNextView {
                id: c.plays as i32,
                season: c.season.map(|n| n as i32),
                episode: c.episode.map(|n| n as i32),
                line: c.note.unwrap_or_default(),
                progress: c.progress,
            }),
        Err(_) => None,
    };

    Ok(SeriesView {
        up_next,
        on_this_pc: on_this_pc as i32,
        also_here,
        watched: watched as i32,
        seasons: seasons
            .into_iter()
            .map(|(n, count)| SeasonView {
                number: n.map(|n| n as i32),
                episodes: count as i32,
            })
            .collect(),
        next_airing: episodes
            .next_airing(series)
            .await
            .map_err(err)?
            .map(|(episode, at)| AiringView {
                episode: episode as i32,
                airs_at: at as f64,
            }),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn season_episodes(
    state: State<'_, AppState>,
    id: i32,
    season: Option<i32>,
) -> Result<Vec<EpisodeView>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    let seen = ProfileRepository::new(&db)
        .finished_in_season(profile, i64::from(id), season.map(i64::from))
        .await
        .map_err(err)?;
    let here = LibraryRepository::new(&db)
        .episodes_on_this_pc(i64::from(id))
        .await
        .map_err(err)?;
    Ok(EpisodeRepository::new(&db)
        .episodes_of(i64::from(id), season.map(i64::from))
        .await
        .map_err(err)?
        .into_iter()
        .map(|e| {
            let watched = seen.contains(&e.media_item_id);
            let on_this_pc = here.contains(&e.media_item_id);
            EpisodeView {
                watched,
                on_this_pc,
                ..EpisodeView::from(e)
            }
        })
        .collect())
}

/// What a mark as watched covers, from the page that asked (Appendix F, I21).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatchedScope {
    /// A film or one episode.
    Title {
        id: i32,
    },
    Season {
        series: i32,
        season: Option<i32>,
    },
    Series {
        id: i32,
    },
}

/// Mark a film, an episode, a season or a whole show as watched, or as not. Returns how
/// many titles changed. **Thin**: the rule is `ProfileRepository::set_watched`'s.
#[tauri::command]
#[specta::specta]
pub async fn set_watched(
    state: State<'_, AppState>,
    scope: WatchedScope,
    watched: bool,
) -> Result<i32, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    let scope = match scope {
        WatchedScope::Title { id } => WatchScope::Title(i64::from(id)),
        WatchedScope::Season { series, season } => WatchScope::Season {
            series: i64::from(series),
            season: season.map(i64::from),
        },
        WatchedScope::Series { id } => WatchScope::Series(i64::from(id)),
    };
    let changed = ProfileRepository::new(&db)
        .set_watched(profile, scope, watched)
        .await
        .map_err(err)?;
    // What was watched is what the taste model reads.
    crate::commands::home::refresh_model(&state, profile);
    Ok(changed as i32)
}

/// Whether this profile has seen a film or an episode through.
#[tauri::command]
#[specta::specta]
pub async fn title_watched(state: State<'_, AppState>, id: i32) -> Result<bool, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    ProfileRepository::new(&db)
        .finished(profile, i64::from(id))
        .await
        .map_err(err)
}

/// The episodes either side of this one, for the player (Phase 11).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Neighbours {
    pub previous: Option<EpisodeView>,
    pub next: Option<EpisodeView>,
}

#[tauri::command]
#[specta::specta]
pub async fn episode_neighbours(state: State<'_, AppState>, id: i32) -> Result<Neighbours, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let repo = EpisodeRepository::new(&db);
    Ok(Neighbours {
        previous: repo
            .previous_before(i64::from(id))
            .await
            .map_err(err)?
            .map(EpisodeView::from),
        next: repo
            .next_after(i64::from(id))
            .await
            .map_err(err)?
            .map(EpisodeView::from),
    })
}
