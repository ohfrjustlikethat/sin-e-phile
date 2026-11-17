//! Posters (Phase 16, ADR-0048). **Thin**: which addresses to try and how a poster is
//! fetched, kept and refused are `sinephile_artwork::posters`'s; this reads the profile's
//! choices and hands the page a `data:` URL.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_artwork::posters::{data_url, is_imdb_id, sources};
use sinephile_artwork::{ArtworkCache, Posters};
use sinephile_metadata_api::tmdb::Tmdb;

use sinephile_persistence::repositories::{CredentialRepository, MediaRepository, TmdbAccess};

use crate::state::AppState;

/// The profile's switch for the keyless posters ("on" or absent).
const SHOW_POSTERS: &str = "artwork.posters";

/// One fetcher for the app's life: its session memory of misses and its limit of four at
/// once are shared by every card.
static POSTERS: OnceLock<Posters> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ArtworkView {
    /// The keyless posters are switched on for this profile.
    pub posters: bool,
    /// This profile has a TMDB key, so TMDB's posters are used.
    pub tmdb: bool,
}

async fn keyless_on(db: &sinephile_persistence::Db, me: i64) -> bool {
    CredentialRepository::new(db)
        .get(me, SHOW_POSTERS)
        .await
        .ok()
        .flatten()
        .as_deref()
        == Some("on")
}

#[tauri::command]
#[specta::specta]
pub async fn artwork_settings(state: State<'_, AppState>) -> Result<ArtworkView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let tmdb = matches!(
        CredentialRepository::new(&db).tmdb_access(me).await,
        Ok(TmdbAccess::Configured(_))
    );
    Ok(ArtworkView {
        posters: keyless_on(&db, me).await,
        tmdb,
    })
}

/// Switch the keyless posters on or off for this profile (onboarding, Settings → Library).
#[tauri::command]
#[specta::specta]
pub async fn set_show_posters(state: State<'_, AppState>, on: bool) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    CredentialRepository::new(&db)
        .set(me, SHOW_POSTERS, if on { "on" } else { "off" })
        .await
        .map_err(|e| e.to_string())
}

/// A title's poster as a `data:` URL, or `None`: no source chosen, no IMDb id, or no poster.
#[tauri::command]
#[specta::specta]
pub async fn poster(state: State<'_, AppState>, id: i32) -> Result<Option<String>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let keyless = keyless_on(&db, me).await;
    let key = match CredentialRepository::new(&db).tmdb_access(me).await {
        Ok(TmdbAccess::Configured(key)) => Some(key),
        _ => None,
    };
    if !keyless && key.is_none() {
        return Ok(None);
    }
    let Some(imdb) = MediaRepository::new(&db)
        .external_id(i64::from(id), "imdb")
        .await
        .map_err(|e| e.to_string())?
        .filter(|i| is_imdb_id(i))
    else {
        return Ok(None);
    };
    let tmdb = match key {
        Some(key) => Tmdb::new(state.http.clone(), key)
            .poster_url(&imdb)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let urls = sources(tmdb, keyless, &imdb);
    let Some(dir) = state.data_dir() else {
        return Ok(None);
    };
    let posters = POSTERS.get_or_init(|| {
        Posters::new(ArtworkCache::new(
            dir.join("cache").join("posters"),
            ArtworkCache::DEFAULT_BUDGET,
        ))
    });
    Ok(posters.first(&urls).await.map(|webp| data_url(&webp)))
}
