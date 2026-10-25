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
    /// `downloading` or `building`.
    ///
    /// The two stages count different things — bytes off the network, then titles into a
    /// graph — and the screen has to be told which. A bar labelled in megabytes that sits
    /// still for half a minute reads as a hang, and inferring the stage from a zero total
    /// would be guessing at something the backend already knows.
    pub phase: String,
    #[specta(type = i32)]
    pub done_bytes: i64,
    #[specta(type = i32)]
    pub total_bytes: i64,
}

/// The search engine this data directory can support right now.
///
/// One function rather than two, because the engine is built twice — at launch, and again
/// the moment the optional downloads finish — and two copies of "how do we decide whether
/// search is hybrid" would agree on the day they were written and never again. The
/// keyword-only path is not a convenience fallback; it is the Tier 0 floor (SPEC.md §8,
/// ADR-0014) taking the same code path as everything else.
pub fn engine_for(dir: &std::path::Path) -> sinephile_search_engine::Engine {
    let models = dir.join("models");
    match sinephile_embedder::Embedder::pinned(
        &models.join("bge-small-en-v1.5-int8.onnx"),
        &models.join("bge-small-en-v1.5-tokenizer.json"),
    ) {
        Ok(embedder) => match sinephile_vector_index::VectorIndex::view(
            &sinephile_vector_index::index_path(dir),
        ) {
            Ok(index) => {
                tracing::info!("search: hybrid");
                sinephile_search_engine::Engine::hybrid(sinephile_search_engine::Semantic {
                    index,
                    embedder,
                })
            }
            Err(error) => {
                tracing::info!(%error, "search: keyword only (no index)");
                sinephile_search_engine::Engine::keyword_only()
            }
        },
        Err(error) => {
            tracing::info!(%error, "search: keyword only (no model)");
            sinephile_search_engine::Engine::keyword_only()
        }
    }
}

/// Download everything that is missing, having been given permission — then make it
/// usable.
///
/// **Only ever called from an explicit action.** Nothing here runs on launch: ADR-0014's
/// requirement is consent first, with the size shown, and the shape of this API is what
/// enforces that — there is no path that fetches without someone having invoked it.
///
/// # The download is not the deliverable
///
/// Until 2026-09-22 this stopped after the last byte, and that was the whole bug (D44):
/// ADR-0014 publishes *vectors*, and the graph over them is derived on the machine, so a
/// downloaded artefact with no index is 313 MB that nothing reads. Search stayed
/// keyword-only, permanently, on every machine but the one where the index had been built
/// by hand. So the build is part of this command, and the engine is reinstalled at the end
/// — without it the user would have to restart the app to use what they just fetched.
#[tauri::command]
#[specta::specta]
pub async fn download_optional_assets(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
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
                    phase: "downloading".into(),
                    done_bytes: progress.downloaded as i64,
                    total_bytes: progress.total.unwrap_or(0) as i64,
                },
            );
        })
        .await
        .map_err(|e| format!("{}: {e}", asset.name))?;
    }

    build_index(&app, &state, &dir).await?;
    state.set_engine(engine_for(&dir));
    Ok(())
}

/// Derive the HNSW index from the downloaded artefact.
///
/// Skipped when an index is already present and usable — the graph is derived, so
/// rebuilding it on every launch would cost minutes for nothing.
async fn build_index(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    dir: &std::path::Path,
) -> Result<(), String> {
    if sinephile_vector_index::index_path(dir).is_file() {
        return Ok(());
    }
    let Some(db) = state.db() else {
        return Err("the catalogue is not open yet".into());
    };

    let models = dir.join("models");
    let mut embedder = sinephile_embedder::Embedder::pinned(
        &models.join("bge-small-en-v1.5-int8.onnx"),
        &models.join("bge-small-en-v1.5-tokenizer.json"),
    )
    .map_err(|e| format!("the language model could not be loaded: {e}"))?;

    let handle = app.clone();
    let built = sinephile_catalogue::index::build(&db, &mut embedder, dir, move |done, total| {
        // Same channel as the download, because to the person waiting this is one
        // operation — but a different `phase`, because it is counting titles now rather
        // than bytes, and a bar that silently changed units would misreport both.
        if done % 25_000 == 0 {
            let _ = handle.emit(
                "asset-progress",
                AssetProgress {
                    name: "Meaning index".into(),
                    phase: "building".into(),
                    done_bytes: done as i64,
                    total_bytes: total as i64,
                },
            );
        }
    })
    .await
    .map_err(|e| format!("the meaning index could not be built: {e}"))?;

    tracing::info!(
        indexed = built.indexed,
        beyond_artefact = built.beyond_artefact,
        bytes = built.bytes,
        "vector index built"
    );
    Ok(())
}
