//! The optional-download IPC surface (ADR-0014, subtask 5.9). Thin, per §7.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use sinephile_catalogue::assets;

use crate::state::AppState;

/// One optional download, as the consent screen needs it.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct OptionalAsset {
    pub name: String,
    pub purpose: String,
    /// Bytes, so the UI can format them in its own locale rather than parse a string.
    #[specta(type = i32)]
    pub bytes: i64,
    /// `absent`, `present`, or a reason it cannot be used.
    pub state: String,
    pub unusable_reason: Option<String>,
}

/// What is missing, and what agreeing would cost.
///
/// ADR-0014 requires the size to be shown **before** the download, so it is stated here
/// from pinned values rather than discovered by starting one.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AssetPlan {
    pub assets: Vec<OptionalAsset>,
    /// Total bytes still to fetch — what the person is actually agreeing to.
    #[specta(type = i32)]
    pub outstanding_bytes: i64,
    /// Is the semantic half already working? If so there is nothing to consent to.
    pub complete: bool,
}

fn data_dir() -> std::path::PathBuf {
    sinephile_persistence::paths::data_dir(sinephile_persistence::DataLocation::Development)
        .unwrap_or_else(|_| std::path::PathBuf::from("data"))
}

/// What the semantic half needs that is not here yet.
#[tauri::command]
#[specta::specta]
pub fn optional_assets() -> AssetPlan {
    let dir = data_dir();
    let assets = assets::optional_assets(&dir);

    let mut outstanding = 0i64;
    let described: Vec<OptionalAsset> = assets
        .iter()
        .map(|asset| {
            let state = assets::state_of(asset);
            // An UNUSABLE asset counts toward the total: it has to be fetched again, and
            // a consent screen that omitted it would understate what it is asking for.
            if !matches!(state, assets::AssetState::Present) {
                outstanding += asset.bytes as i64;
            }
            OptionalAsset {
                name: asset.name.to_string(),
                purpose: asset.purpose.to_string(),
                bytes: asset.bytes as i64,
                state: match &state {
                    assets::AssetState::Absent => "absent".into(),
                    assets::AssetState::Present => "present".into(),
                    assets::AssetState::Unusable(_) => "unusable".into(),
                },
                unusable_reason: match state {
                    assets::AssetState::Unusable(reason) => Some(reason),
                    _ => None,
                },
            }
        })
        .collect();

    AssetPlan {
        complete: outstanding == 0,
        assets: described,
        outstanding_bytes: outstanding,
    }
}

/// Progress, emitted as it happens. One event per asset per chunk.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AssetProgress {
    pub name: String,
    #[specta(type = i32)]
    pub done_bytes: i64,
    #[specta(type = i32)]
    pub total_bytes: i64,
}

/// Download everything that is missing, having been given permission.
///
/// **Only ever called from an explicit action.** Nothing here runs on launch: ADR-0014's
/// requirement is consent first, with the size shown, and the shape of this API is what
/// enforces that — there is no path that fetches without someone having invoked it.
#[tauri::command]
#[specta::specta]
pub async fn download_optional_assets(
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let dir = data_dir();
    for asset in assets::optional_assets(&dir) {
        if matches!(assets::state_of(&asset), assets::AssetState::Present) {
            continue;
        }
        let name = asset.name.to_string();
        let handle = app.clone();
        assets::download(&asset, move |progress| {
            let _ = handle.emit(
                "asset-progress",
                AssetProgress {
                    name: name.clone(),
                    done_bytes: progress.downloaded as i64,
                    total_bytes: progress.total.unwrap_or(0) as i64,
                },
            );
        })
        .await
        .map_err(|e| format!("{}: {e}", asset.name))?;
    }
    Ok(())
}
