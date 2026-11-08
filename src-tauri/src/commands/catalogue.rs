//! The catalogue keeping itself current (SPEC.md Phase 16, ADR-0030 made automatic).
//! **Thin**: what a refresh does is `sinephile_catalogue::freshness::refresh`; this runs it
//! on a timer, says when it last ran, and lets Settings ask for one now.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use sinephile_metadata_api::tmdb::{Tmdb, TmdbError};
use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::{
    CredentialRepository, EpisodeRepository, MediaRepository, ProfileRepository, TmdbAccess,
};

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
        showing_now(&app).await;
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

// ---- TMDB, with the viewer's own key (Phase 16, ADR-0013, ADR-0027) ----------------------

/// Today, UTC, `YYYY-MM-DD`.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    sinephile_subtitles::utc_day(secs)
}

/// For each profile with its own key, what is showing now, once a day.
async fn showing_now(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(db) = state.db() else { return };
    let Ok(profiles) = ProfileRepository::new(&db).all().await else {
        return;
    };
    let day = today();
    for p in profiles {
        let Ok(TmdbAccess::Configured(key)) =
            CredentialRepository::new(&db).tmdb_access(p.id).await
        else {
            continue;
        };
        if sinephile_catalogue::now::fetched_on(&db, p.id)
            .await
            .ok()
            .flatten()
            .as_deref()
            == Some(day.as_str())
        {
            continue;
        }
        let tmdb = Tmdb::new(state.http.clone(), key);
        match sinephile_catalogue::now::refresh(&db, &tmdb, p.id, &day).await {
            Ok(n) => {
                tracing::info!(profile = p.id, matched = n, "showing now, from TMDB");
                let _ = app.emit("catalogue-refreshed", ());
            }
            Err(error) => tracing::warn!(%error, profile = p.id, "showing now not fetched"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TmdbStatus {
    /// This profile has its own key saved.
    pub has_key: bool,
    /// When "showing now" was last fetched with it.
    pub fetched_on: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn tmdb_status(state: State<'_, AppState>) -> Result<TmdbStatus, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let has_key = matches!(
        CredentialRepository::new(&db)
            .tmdb_access(me)
            .await
            .map_err(|e| e.to_string())?,
        TmdbAccess::Configured(_)
    );
    Ok(TmdbStatus {
        has_key,
        fetched_on: sinephile_catalogue::now::fetched_on(&db, me)
            .await
            .map_err(|e| e.to_string())?,
    })
}

/// Save this profile's own TMDB key, checked first: a refused key is not saved. Then what
/// is showing now is fetched with it, in the background.
#[tauri::command]
#[specta::specta]
pub async fn set_tmdb_key(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let key = key.trim().to_string();
    match Tmdb::new(state.http.clone(), key.clone()).check().await {
        Err(TmdbError::BadKey) => return Err(
            "TMDB did not accept that key. Copy the 'API Key' from your TMDB account's API page."
                .into(),
        ),
        Err(e) => tracing::warn!(error = %e, "could not check the TMDB key; saved unchecked"),
        Ok(()) => {}
    }
    CredentialRepository::new(&db)
        .set_tmdb_key(me, &key)
        .await
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn(async move { showing_now(&app).await });
    Ok(())
}

/// Remove this profile's key and what was fetched with it (ADR-0027).
#[tauri::command]
#[specta::specta]
pub async fn clear_tmdb_key(state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let creds = CredentialRepository::new(&db);
    let (removed, _) = creds.clear_tmdb_key(me).await.map_err(|e| e.to_string())?;
    creds
        .set(me, sinephile_catalogue::now::NOW_KEY, "")
        .await
        .map_err(|e| e.to_string())?;
    Ok(removed)
}
