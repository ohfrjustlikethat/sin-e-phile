//! The torrent IPC surface (SPEC.md Phase 7). **Thin**: the engine, the streaming
//! server, the settings rules and the stream-to-player step all live in
//! `crates/torrent`; these convert types and call them.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use sinephile_persistence::repositories::ProfileRepository;
use sinephile_torrent::settings::{self, TorrentSettings, SETTINGS_KEY};

use crate::state::AppState;

/// The settings screen's view of torrent behaviour.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TorrentSettingsView {
    pub download_limit_kib: Option<u32>,
    pub upload_limit_kib: Option<u32>,
    pub peer_limit: u32,
    pub keep_seeding: bool,
    pub bind_interface: Option<String>,
    /// Adapters to offer, connected first.
    pub adapters: Vec<AdapterView>,
    /// Settings are read when the engine starts; a change applies after a restart if
    /// the engine is already running.
    pub engine_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AdapterView {
    pub name: String,
    pub connected: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn torrent_settings(state: State<'_, AppState>) -> Result<TorrentSettingsView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let stored = ProfileRepository::new(&db)
        .setting(SETTINGS_KEY)
        .await
        .map_err(|e| e.to_string())?;
    let s = TorrentSettings::parse(stored.as_deref());
    Ok(TorrentSettingsView {
        download_limit_kib: s.download_limit_kib,
        upload_limit_kib: s.upload_limit_kib,
        peer_limit: s.peer_limit,
        keep_seeding: s.keep_seeding,
        bind_interface: s.bind_interface,
        adapters: settings::adapters()
            .into_iter()
            .map(|a| AdapterView {
                name: a.name,
                connected: a.connected,
            })
            .collect(),
        engine_running: state.torrent_if_started().is_some(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn set_torrent_settings(
    state: State<'_, AppState>,
    download_limit_kib: Option<u32>,
    upload_limit_kib: Option<u32>,
    peer_limit: u32,
    keep_seeding: bool,
    bind_interface: Option<String>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let s = TorrentSettings {
        download_limit_kib,
        upload_limit_kib,
        peer_limit,
        keep_seeding,
        bind_interface: bind_interface.filter(|n| !n.trim().is_empty()),
    };
    s.validate()?;
    ProfileRepository::new(&db)
        .set_setting(SETTINGS_KEY, &s.to_json())
        .await
        .map_err(|e| e.to_string())?;
    // The limits apply at once to a running engine; the rest at its next start.
    if let Some(rt) = state.torrent_if_started() {
        let bps = |kib: Option<u32>| kib.map(|k| k.saturating_mul(1024));
        rt.engine
            .set_limits(bps(s.download_limit_kib), bps(s.upload_limit_kib));
    }
    Ok(())
}

/// One torrent, for the downloads surface and progress events.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TorrentView {
    pub id: String,
    pub name: Option<String>,
    /// `initializing`, `live`, `paused` or `error`.
    pub state: String,
    /// 0.0–1.0.
    pub progress: f64,
    /// Bytes as f64: a JavaScript number is exact to 2^53, ample for any file.
    pub total_bytes: f64,
    pub download_bps: f64,
    pub upload_bps: f64,
    pub live_peers: u32,
    pub finished: bool,
    pub error: Option<String>,
}

impl From<sinephile_torrent::Status> for TorrentView {
    fn from(s: sinephile_torrent::Status) -> Self {
        Self {
            progress: if s.total_bytes == 0 {
                0.0
            } else {
                s.progress_bytes as f64 / s.total_bytes as f64
            },
            state: match s.state {
                sinephile_torrent::State::Initializing => "initializing",
                sinephile_torrent::State::Live => "live",
                sinephile_torrent::State::Paused => "paused",
                sinephile_torrent::State::Error => "error",
            }
            .into(),
            id: s.id,
            name: s.name,
            total_bytes: s.total_bytes as f64,
            download_bps: s.download_bps,
            upload_bps: s.upload_bps,
            live_peers: s.live_peers,
            finished: s.finished,
            error: s.error,
        }
    }
}

/// Every torrent's progress, once a second while the engine runs.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TorrentProgress {
    pub torrents: Vec<TorrentView>,
}

#[tauri::command]
#[specta::specta]
pub async fn torrents(state: State<'_, AppState>) -> Result<Vec<TorrentView>, String> {
    Ok(match state.torrent_if_started() {
        Some(rt) => rt.engine.list().into_iter().map(Into::into).collect(),
        None => Vec::new(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn torrent_pause(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let rt = state.torrent().await?;
    rt.engine.pause(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn torrent_resume(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let rt = state.torrent().await?;
    rt.engine.resume(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn torrent_remove(
    state: State<'_, AppState>,
    id: String,
    delete_files: bool,
) -> Result<(), String> {
    let rt = state.torrent().await?;
    rt.engine
        .remove(&id, delete_files)
        .await
        .map_err(|e| e.to_string())
}

/// The `torrent-progress` event: every torrent's state, once a second, from the moment
/// the engine has been started by anything. Runs for the life of the app.
pub async fn progress_loop(app: AppHandle) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tick.tick().await;
        let Some(rt) = app.state::<AppState>().torrent_if_started() else {
            continue;
        };
        let list = rt.engine.list();
        if list.is_empty() {
            continue;
        }
        let _ = app.emit(
            "torrent-progress",
            TorrentProgress {
                torrents: list.into_iter().map(Into::into).collect(),
            },
        );
    }
}
