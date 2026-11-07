//! Downloads (SPEC.md Phase 13). **Thin**: the queue, its plan and the filing are
//! `sinephile_downloads`'; this ticks the manager once a second and relays.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager as _, State};
use tokio::sync::OnceCell;

use sinephile_downloads::{now_iso, Backend, Manager, Rqbit};
use sinephile_library::probe::ffprobe;
use sinephile_library::scan::Prober;
use sinephile_persistence::repositories::{Download, DownloadRepository};

use crate::state::AppState;

/// The manager, built once the database and the torrent engine are ready.
static MANAGER: OnceCell<Arc<Manager>> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct DownloadView {
    pub id: i32,
    pub media_item_id: Option<i32>,
    pub title: String,
    /// "queued" | "active" | "paused" | "done" | "failed" | "cancelled"
    pub state: String,
    pub backend: String,
    pub priority: i32,
    pub done_bytes: f64,
    pub total_bytes: Option<f64>,
    pub limit_kib: Option<i32>,
    pub not_before: Option<String>,
    pub reason: Option<String>,
    pub error: Option<String>,
    pub file_path: Option<String>,
}

impl From<Download> for DownloadView {
    fn from(d: Download) -> Self {
        Self {
            id: d.id as i32,
            media_item_id: d.media_item_id.map(|m| m as i32),
            title: d.title,
            state: d.state.as_str().into(),
            backend: d.backend,
            priority: d.priority as i32,
            done_bytes: d.done_bytes as f64,
            total_bytes: d.total_bytes.map(|t| t as f64),
            limit_kib: d.limit_kib.map(|k| k as i32),
            not_before: d.not_before,
            reason: d.reason,
            error: d.error,
            file_path: d.file_path,
        }
    }
}

async fn manager(app: &AppHandle) -> Result<Arc<Manager>, String> {
    MANAGER
        .get_or_try_init(|| async {
            let state = app.state::<AppState>();
            let db = state.db().ok_or("the library is still opening")?;
            let runtime = state.torrent().await?;
            let dir = crate::logging::data_dir().join("Downloads");
            let backends: Vec<Arc<dyn Backend>> =
                vec![Arc::new(Rqbit::new(runtime.engine.clone()))];
            Ok::<_, String>(Arc::new(Manager::new(
                (*db).clone(),
                backends,
                dir,
                ffprobe as Prober,
            )))
        })
        .await
        .cloned()
}

/// Once a second: start what should run, follow what runs, file what finished. The
/// engine is only started when there is something to download.
pub async fn downloads_loop(app: AppHandle) {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tick.tick().await;
        let Some(db) = app.state::<AppState>().db() else {
            continue;
        };
        if MANAGER.get().is_none() {
            let open = DownloadRepository::new(&db)
                .open()
                .await
                .unwrap_or_default();
            if open.is_empty() {
                continue;
            }
        }
        let Ok(m) = manager(&app).await else {
            continue;
        };
        match m.tick(&now_iso()).await {
            Ok(t) => {
                if !(t.started.is_empty()
                    && t.held.is_empty()
                    && t.finished.is_empty()
                    && t.failed.is_empty())
                {
                    tracing::info!(?t, "downloads");
                }
                if !t.finished.is_empty() {
                    let _ = app.emit("library-changed", 0);
                }
            }
            Err(error) => tracing::warn!(%error, "downloads tick failed"),
        }
        let _ = app.emit("downloads-changed", ());
    }
}

#[tauri::command]
#[specta::specta]
pub async fn downloads(state: State<'_, AppState>) -> Result<Vec<DownloadView>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    DownloadRepository::new(&db)
        .all()
        .await
        .map(|all| all.into_iter().map(DownloadView::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn download_pause(app: AppHandle, id: i32) -> Result<(), String> {
    manager(&app)
        .await?
        .pause(i64::from(id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn download_resume(app: AppHandle, id: i32) -> Result<(), String> {
    manager(&app)
        .await?
        .resume(i64::from(id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn download_cancel(app: AppHandle, id: i32, delete_files: bool) -> Result<(), String> {
    manager(&app)
        .await?
        .cancel(i64::from(id), delete_files)
        .await
        .map_err(|e| e.to_string())
}

/// Where downloads land (a library folder of their own).
#[tauri::command]
#[specta::specta]
pub fn downloads_folder() -> String {
    let dir: PathBuf = crate::logging::data_dir().join("Downloads");
    dir.to_string_lossy().into_owned()
}
