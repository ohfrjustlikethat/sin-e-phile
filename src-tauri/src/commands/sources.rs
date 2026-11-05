//! The Sources IPC surface (SPEC.md Phase 6). **Thin**: every rule — what counts as a
//! valid manifest, what a health line says, which backends are asked — lives in
//! `crates/sources` or `crates/source-protocol`. These functions call them in order
//! and convert types.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::{DirectLinkRepository, SourceRepository};
use sinephile_persistence::Db;
use sinephile_sources::backends::{all, Context};
use sinephile_sources::{addons, links, overview};

use crate::state::AppState;

fn db(state: &AppState) -> Result<Arc<Db>, String> {
    state
        .db()
        .ok_or_else(|| "The library is still opening. Try again in a moment.".to_string())
}

async fn context(state: &AppState, db: &Db) -> Result<(Context, Vec<String>), String> {
    overview::context_from(db, state.http.clone(), state.untrusted_http.clone())
        .await
        .map_err(|e| e.to_string())
}

/// One row of the Sources list.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// Set for an addon: the host its queries go to.
    pub addon_host: Option<String>,
    pub enabled: bool,
    pub health: String,
    pub last_error: Option<String>,
}

/// Everything the Sources section needs at once.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SourcesView {
    pub sources: Vec<SourceInfo>,
    /// Installed addons whose stored manifest no longer passes the checks.
    pub refused_addons: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn sources_overview(state: State<'_, AppState>) -> Result<SourcesView, String> {
    let db = db(&state)?;
    let (ctx, refused) = context(&state, &db).await?;
    let rows = overview::overview(&db, &all(&ctx), &ctx.addons)
        .await
        .map_err(|e| e.to_string())?;
    Ok(SourcesView {
        sources: rows
            .into_iter()
            .map(|r| SourceInfo {
                id: r.id,
                name: r.name,
                blurb: r.blurb,
                addon_host: r.addon_host,
                enabled: r.enabled,
                health: r.health,
                last_error: r.last_error,
            })
            .collect(),
        refused_addons: refused,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn sources_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let db = db(&state)?;
    SourceRepository::new(&db)
        .set_enabled(&id, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn sources_reorder(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    let db = db(&state)?;
    SourceRepository::new(&db)
        .reorder(&ids)
        .await
        .map_err(|e| e.to_string())
}

/// What the confirm screen shows for a pasted manifest address.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AddonPreview {
    /// The address as pasted; send it back to `addon_install`.
    pub pasted: String,
    pub backend_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub finds: String,
    pub delivers: Vec<String>,
    pub host: String,
    pub homepage: Option<String>,
    pub replaces: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn addon_preview(
    state: State<'_, AppState>,
    url: String,
) -> Result<AddonPreview, String> {
    let db = db(&state)?;
    let (u, manifest) = addons::fetch_manifest(&*state.untrusted_http, &url)
        .await
        .map_err(|e| e.to_string())?;
    let installed = SourceRepository::new(&db)
        .addons()
        .await
        .map_err(|e| e.to_string())?;
    let d = addons::describe(&u, &manifest, &installed);
    state.hold_addon(&url, u, manifest);
    Ok(AddonPreview {
        pasted: url,
        backend_id: d.backend_id,
        name: d.name,
        version: d.version,
        description: d.description,
        finds: d.finds,
        delivers: d.delivers,
        host: d.host,
        homepage: d.homepage,
        replaces: d.replaces,
    })
}

/// Install the manifest that `addon_preview` showed for this address — not a fresh
/// fetch of it.
#[tauri::command]
#[specta::specta]
pub async fn addon_install(state: State<'_, AppState>, pasted: String) -> Result<String, String> {
    let db = db(&state)?;
    let (url, manifest) = state
        .take_addon(&pasted)
        .ok_or_else(|| "Look at the source again before adding it.".to_string())?;
    addons::install(&db, &url, &manifest)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn addon_remove(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = db(&state)?;
    SourceRepository::new(&db)
        .remove_addon(&id)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CatalogueView {
    pub name: String,
    pub addons: Vec<CatalogueItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CatalogueItem {
    pub name: String,
    pub description: String,
    pub manifest: String,
}

/// Open a catalogue from an address the user supplied. No default exists.
#[tauri::command]
#[specta::specta]
pub async fn catalogue_open(
    state: State<'_, AppState>,
    url: String,
) -> Result<CatalogueView, String> {
    let c = addons::fetch_catalogue(&*state.untrusted_http, &url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CatalogueView {
        name: c.name,
        addons: c
            .addons
            .into_iter()
            .map(|e| CatalogueItem {
                name: e.name,
                description: e.description,
                manifest: e.manifest,
            })
            .collect(),
    })
}

/// One of the user's own links. Ids are rowids, far below 2^53 (see `SearchHit`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LinkInfo {
    #[specta(type = i32)]
    pub id: i64,
    pub title: String,
    #[specta(type = Option<i32>)]
    pub year: Option<i64>,
    pub url: String,
}

#[tauri::command]
#[specta::specta]
pub async fn links_list(state: State<'_, AppState>) -> Result<Vec<LinkInfo>, String> {
    let db = db(&state)?;
    let rows = DirectLinkRepository::new(&db)
        .all()
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| LinkInfo {
            id: r.id,
            title: r.title,
            year: r.year,
            url: r.url,
        })
        .collect())
}

/// Add from pasted playlist text. Returns how many links were written.
#[tauri::command]
#[specta::specta]
pub async fn links_import(state: State<'_, AppState>, text: String) -> Result<u32, String> {
    let db = db(&state)?;
    links::import(&db, &text)
        .await
        .map(|n| n as u32)
        .map_err(|e| e.to_string())
}

/// Add one link from the form's three fields.
#[tauri::command]
#[specta::specta]
pub async fn links_add(
    state: State<'_, AppState>,
    title: String,
    year: Option<u16>,
    url: String,
) -> Result<u32, String> {
    let db = db(&state)?;
    links::import(&db, &links::single(&title, year, &url))
        .await
        .map(|n| n as u32)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn links_remove(state: State<'_, AppState>, id: i32) -> Result<(), String> {
    let db = db(&state)?;
    DirectLinkRepository::new(&db)
        .remove(i64::from(id))
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A film a source found for a search the catalogue missed, now in the catalogue.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Backfilled {
    #[specta(type = i32)]
    pub id: i64,
    pub title: String,
    pub year: u16,
    /// The source's name, for "found by …".
    pub found_by: String,
}

/// Ask the sources about a search that found nothing (ADR-0030 layer 3).
///
/// A separate command, called by the search screen only after it has SHOWN the empty
/// result, so a search is never slower for it.
#[tauri::command]
#[specta::specta]
pub async fn search_backfill(
    state: State<'_, AppState>,
    query: String,
) -> Result<Option<Backfilled>, String> {
    let db = db(&state)?;
    let (ctx, _) = context(&state, &db).await?;
    let resolver = sinephile_sources::registry::resolver_from(&db, all(&ctx))
        .await
        .map_err(|e| e.to_string())?;
    let made = sinephile_sources::backfill::backfill(&db, &resolver, &query)
        .await
        .map_err(|e| e.to_string())?;
    Ok(made.map(|b| Backfilled {
        found_by: overview::display_name(&b.from, &ctx.addons),
        id: b.id,
        title: b.title,
        year: b.year,
    }))
}
