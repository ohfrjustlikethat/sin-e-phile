//! Posters (Phase 16, ADR-0048) and a title's backdrop (round 9, ADR-0052). **Thin**: which
//! addresses to try and how an image is fetched, kept and refused are
//! `sinephile_artwork::posters`'s; this reads the profile's choices and hands the page a
//! `data:` URL.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_artwork::posters::{data_url, is_imdb_id, sources};
use sinephile_artwork::{Art, ArtworkCache, Posters};
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
    image(&state, id, Art::Poster).await
}

/// A title's backdrop, the wide still across the top of its page, as a `data:` URL; `None` as
/// for a poster.
#[tauri::command]
#[specta::specta]
pub async fn backdrop(state: State<'_, AppState>, id: i32) -> Result<Option<String>, String> {
    image(&state, id, Art::Backdrop).await
}

async fn image(state: &State<'_, AppState>, id: i32, art: Art) -> Result<Option<String>, String> {
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
        Some(key) => {
            let tmdb = Tmdb::new(state.http.clone(), key);
            match art {
                Art::Poster => tmdb.poster_url(&imdb).await,
                Art::Backdrop => tmdb.backdrop_url(&imdb).await,
            }
            .ok()
            .flatten()
        }
        None => None,
    };
    let urls = sources(art, tmdb, keyless, &imdb);
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

// ── Profile pictures (Phase 16, brief item 11) ─────────────────────────────────────────
// **Thin**: decoding, the automatic square and the cut are `sinephile_artwork::avatar`'s.

/// A picture being chosen: its preview, its size, and the square chosen automatically.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PicturePreview {
    /// A `data:` URL of the preview.
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub crop: CropView,
}

/// A square in fractions of the picture: `x` and `side` of its width, `y` of its height.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
pub struct CropView {
    pub x: f64,
    pub y: f64,
    pub side: f64,
}

fn read_picture(path: &str) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("could not read it: {e}"))?
        .len();
    if size > sinephile_artwork::avatar::MAX_FILE_BYTES {
        return Err("that file is too large to be a picture".into());
    }
    std::fs::read(path).map_err(|e| format!("could not read it: {e}"))
}

fn picture_path(dir: &std::path::Path, profile: i32) -> std::path::PathBuf {
    dir.join("profiles").join(format!("{profile}.webp"))
}

/// Windows' file picker, for a profile picture.
#[tauri::command]
#[specta::specta]
pub async fn pick_picture(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri::Manager;
    let owner = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::spawn(move || {
            crate::commands::library::picker::file(owner, "Choose a picture for this profile", None)
        })
        .join()
        .unwrap_or(None)
    })
    .await
    .map_err(|e| e.to_string())
}

/// The chosen picture, ready for the crop to be adjusted.
#[tauri::command]
#[specta::specta]
pub async fn picture_preview(path: String) -> Result<PicturePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_picture(&path)?;
        let p = sinephile_artwork::avatar::preview(&bytes)
            .map_err(|_| "that file is not a picture this app can read".to_string())?;
        Ok(PicturePreview {
            url: data_url(&p.webp),
            width: p.width,
            height: p.height,
            crop: CropView {
                x: p.crop.x,
                y: p.crop.y,
                side: p.crop.side,
            },
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cut the chosen square from the picture and keep it as `profile`'s picture.
#[tauri::command]
#[specta::specta]
pub async fn set_profile_picture(
    state: State<'_, AppState>,
    profile: i32,
    path: String,
    crop: CropView,
) -> Result<(), String> {
    let dir = state.data_dir().ok_or("the data folder is not open")?;
    let to = picture_path(&dir, profile);
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_picture(&path)?;
        let made = sinephile_artwork::avatar::avatar(
            &bytes,
            sinephile_artwork::avatar::Crop {
                x: crop.x,
                y: crop.y,
                side: crop.side,
            },
        )
        .map_err(|_| "that file is not a picture this app can read".to_string())?;
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&to, made).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `profile`'s picture as a `data:` URL, or `None`: it has none.
#[tauri::command]
#[specta::specta]
pub async fn profile_picture(
    state: State<'_, AppState>,
    profile: i32,
) -> Result<Option<String>, String> {
    let Some(dir) = state.data_dir() else {
        return Ok(None);
    };
    Ok(std::fs::read(picture_path(&dir, profile))
        .ok()
        .map(|webp| data_url(&webp)))
}

/// Take `profile`'s picture away: its coloured initial shows again.
#[tauri::command]
#[specta::specta]
pub async fn clear_profile_picture(state: State<'_, AppState>, profile: i32) -> Result<(), String> {
    let dir = state.data_dir().ok_or("the data folder is not open")?;
    match std::fs::remove_file(picture_path(&dir, profile)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
