//! Importing a viewing history (SPEC.md Phase 14). **Thin**: reading and matching are
//! `sinephile_imports`'; this asks for the file and records it for the profile watching.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager as _, State};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct UnmatchedView {
    pub title: String,
    pub year: Option<i32>,
    pub why: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ImportView {
    /// "Letterboxd" | "Trakt" | "IMDb" | "MyAnimeList"
    pub from: String,
    pub entries: u32,
    pub matched: u32,
    pub ratings: u32,
    pub watches: u32,
    pub watchlist: u32,
    /// What could not be placed, in words (the first hundred).
    pub unmatched: Vec<UnmatchedView>,
    pub unmatched_total: u32,
}

/// Windows' file picker for an export. `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn pick_import_file(app: AppHandle) -> Result<Option<String>, String> {
    let owner = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::spawn(move || {
            crate::commands::library::picker::file(
                owner,
                "Choose your Letterboxd, Trakt, IMDb or MyAnimeList export",
            )
        })
        .join()
        .unwrap_or(None)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Read an export and record it for the profile watching.
#[tauri::command]
#[specta::specta]
pub async fn import_history(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    let p = std::path::PathBuf::from(&path);
    let (format, entries) =
        tauri::async_runtime::spawn_blocking(move || sinephile_imports::read(&p))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    let now = sinephile_downloads::now_iso()
        .replace('T', " ")
        .replace('Z', "");
    let s = sinephile_imports::import(&db, profile, &entries, &now)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ImportView {
        from: format.name().into(),
        entries: s.entries as u32,
        matched: s.matched as u32,
        ratings: s.ratings as u32,
        watches: s.watches as u32,
        watchlist: s.watchlist as u32,
        unmatched_total: s.unmatched.len() as u32,
        unmatched: s
            .unmatched
            .into_iter()
            .take(100)
            .map(|u| UnmatchedView {
                title: u.title,
                year: u.year.map(|y| y as i32),
                why: u.why,
            })
            .collect(),
    })
}
