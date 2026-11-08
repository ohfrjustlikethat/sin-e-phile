//! The catalogue keeping itself current (SPEC.md Phase 16, ADR-0030 made automatic).
//! **Thin**: what a refresh does is `sinephile_catalogue::freshness::refresh`; this runs it
//! on a timer, says when it last ran, and lets Settings ask for one now.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::{EpisodeRepository, MediaRepository, ProfileRepository};

use crate::state::AppState;

/// One refresh at a time: the timer's and "refresh now" never overlap.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// How often the publisher is asked. Each ask is one HEAD request; the download happens
/// only when IMDb has republished, which it does about once a day.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// How often a followed anime's next airing is asked of AniList.
const AIRING_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct CatalogueStatus {
    /// When the last refresh that did something finished (UTC, ISO 8601).
    pub refreshed_at: Option<String>,
    pub titles_added: Option<i32>,
    pub episodes_added: Option<i32>,
    pub running: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn catalogue_status(state: State<'_, AppState>) -> Result<CatalogueStatus, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let raw = ProfileRepository::new(&db)
        .setting(sinephile_catalogue::freshness::LAST_REFRESH)
        .await
        .map_err(|e| e.to_string())?;
    let mut out = CatalogueStatus {
        running: RUNNING.load(Ordering::SeqCst),
        ..Default::default()
    };
    if let Some(v) = raw.and_then(|r| serde_json::from_str::<serde_json::Value>(&r).ok()) {
        out.refreshed_at = v["at"].as_str().map(str::to_string);
        out.titles_added = v["titles_added"].as_i64().map(|n| n as i32);
        out.episodes_added = v["episodes_added"].as_i64().map(|n| n as i32);
    }
    Ok(out)
}

/// "Refresh now": a refresh whatever the publisher says, in the background.
#[tauri::command]
#[specta::specta]
pub async fn refresh_catalogue(app: AppHandle) -> Result<bool, String> {
    Ok(start(app, true))
}

/// Start a refresh unless one is running; `false` if one was.
fn start(app: AppHandle, force: bool) -> bool {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return false;
    }
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let (Some(db), Some(dir)) = (state.db(), state.data_dir()) {
            let _ = app.emit("catalogue-refreshing", true);
            let transport = state.http.clone();
            let outcome = sinephile_catalogue::freshness::refresh(
                &db,
                transport.as_ref(),
                &dir.join("datasets"),
                force,
            )
            .await;
            tracing::info!(?outcome, "catalogue freshness");
            if matches!(
                outcome,
                sinephile_catalogue::freshness::Outcome::Refreshed { .. }
            ) {
                let _ = app.emit("catalogue-refreshed", ());
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit("catalogue-refreshing", false);
    });
    true
}

/// The timers: the catalogue checked hourly (launch included), and the next airing of
/// every followed anime every six hours.
pub async fn keep_current(app: AppHandle) {
    let state = app.state::<AppState>();
    while state.db().is_none() || state.data_dir().is_none() {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let mut last_airing: Option<std::time::Instant> = None;
    loop {
        start(app.clone(), false);
        if last_airing.is_none_or(|t| t.elapsed() >= AIRING_EVERY) {
            last_airing = Some(std::time::Instant::now());
            airing(&app).await;
        }
        tokio::time::sleep(CHECK_EVERY).await;
    }
}

/// AniList's next airing for each followed anime that has an AniList id (ADR-0030 layer 2).
async fn airing(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(db) = state.db() else { return };
    let Ok(followed) = sinephile_catalogue::refresh::followed_series(&db).await else {
        return;
    };
    let http: Arc<dyn Transport> = state.http.clone();
    let client = sinephile_metadata_api::AniList::owned(http).await;
    let media = MediaRepository::new(&db);
    for series in followed {
        let Ok(Some(id)) = media.external_id(series, "anilist").await else {
            continue;
        };
        let Ok(id) = id.parse::<i64>() else { continue };
        if let Ok(Some(m)) = client.media(id).await {
            if let Some(next) = m.next_airing {
                let _ = EpisodeRepository::new(&db)
                    .set_next_airing(series, next.episode, next.airing_at)
                    .await;
            }
        }
    }
}
