//! A series' page (SPEC.md Phase 11): seasons, episodes, and when the next one airs.
//! **Thin**: the queries are `EpisodeRepository`'s; this converts and, for anime, asks
//! AniList for a fresh schedule now and then.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_metadata_api::{AniList, Transport};
use sinephile_persistence::repositories::{EpisodeRepository, EpisodeRow, MediaRepository};

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
    pub seasons: Vec<SeasonView>,
    pub next_airing: Option<AiringView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct EpisodeView {
    pub id: i32,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub title: String,
    pub air_date: Option<String>,
}

impl From<EpisodeRow> for EpisodeView {
    fn from(e: EpisodeRow) -> Self {
        Self {
            id: e.media_item_id as i32,
            season: e.season_number.map(|n| n as i32),
            episode: e.episode_number.map(|n| n as i32),
            title: e.title,
            air_date: e.air_date,
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

    Ok(SeriesView {
        seasons: episodes
            .seasons(series)
            .await
            .map_err(err)?
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
    Ok(EpisodeRepository::new(&db)
        .episodes_of(i64::from(id), season.map(i64::from))
        .await
        .map_err(err)?
        .into_iter()
        .map(EpisodeView::from)
        .collect())
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
