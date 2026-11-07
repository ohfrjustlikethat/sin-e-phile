//! Downloads (SPEC.md Phase 13). **Thin**: the queue, its plan and the filing are
//! `sinephile_downloads`'; this ticks the manager once a second and relays.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager as _, State};
use tokio::sync::Mutex;

use sinephile_downloads::advise::{advise, Choice, Inputs, Taste, PLAYER_BUFFER};
use sinephile_downloads::backend::source_for;
use sinephile_downloads::qbittorrent::Qbittorrent;
use sinephile_downloads::{measure, storage};
use sinephile_downloads::{now_iso, Backend, Manager, Rqbit};
use sinephile_library::probe::ffprobe;
use sinephile_library::scan::Prober;
use sinephile_persistence::repositories::{
    CredentialRepository, Download, DownloadRepository, DownloadState, NewDownload,
    ProfileRepository,
};
use source_protocol::PlayableStream;

use crate::state::AppState;

/// The manager, built once the database and the torrent engine are ready, and built again
/// when the qBittorrent settings change.
static MANAGER: Mutex<Option<Arc<Manager>>> = Mutex::const_new(None);

/// The qBittorrent Web API's address, user name (plain settings) and password (a secret).
pub const QBIT_ADDRESS: &str = "qbittorrent.address";
pub const QBIT_USER: &str = "qbittorrent.user";
pub const QBIT_PASSWORD: &str = "qbittorrent.password";
/// Plain settings: how many run at once, the size budget (GiB), and this connection's
/// speed by hour (`measure::Hourly` as JSON).
pub const MAX_ACTIVE: &str = "downloads.max_active";
pub const BUDGET_GIB: &str = "downloads.budget_gib";
pub const HOURLY: &str = "downloads.hourly";

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
    let mut held = MANAGER.lock().await;
    if let Some(m) = held.as_ref() {
        return Ok(m.clone());
    }
    let state = app.state::<AppState>();
    let db = state.db().ok_or("the library is still opening")?;
    let runtime = state.torrent().await?;
    let dir = crate::logging::data_dir().join("Downloads");
    let mut backends: Vec<Arc<dyn Backend>> = Vec::new();
    // The user's own qBittorrent first, when they configured one (ADR-0029); librqbit
    // always, and the one a download falls back to.
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(|e| e.to_string())?;
    let creds = CredentialRepository::new(&db);
    if let Some(address) = creds
        .get(profile, QBIT_ADDRESS)
        .await
        .ok()
        .flatten()
        .filter(|a| !a.is_empty())
    {
        let user = creds
            .get(profile, QBIT_USER)
            .await
            .ok()
            .flatten()
            .filter(|u| !u.is_empty());
        let password = creds.secret(profile, QBIT_PASSWORD).await.ok().flatten();
        backends.push(Arc::new(Qbittorrent::new(
            state.untrusted_http.clone(),
            &address,
            user,
            password,
        )));
    }
    backends.push(Arc::new(Rqbit::new(runtime.engine.clone())));
    let mut m = Manager::new((*db).clone(), backends, dir, ffprobe as Prober);
    if let Some(n) = setting(&db, MAX_ACTIVE).await.and_then(|v| v.parse().ok()) {
        m.max_active = n;
    }
    let m = Arc::new(m);
    *held = Some(m.clone());
    Ok(m)
}

/// Build the manager again on the next tick (the qBittorrent settings changed).
pub async fn rebuild_manager() {
    *MANAGER.lock().await = None;
}

async fn setting(db: &sinephile_persistence::Db, key: &str) -> Option<String> {
    ProfileRepository::new(db).setting(key).await.ok().flatten()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn downloads_dir() -> PathBuf {
    crate::logging::data_dir().join("Downloads")
}

/// The last minute's samples of the combined rate, folded into the hour's mean once a
/// minute (only while something downloads: an idle connection says nothing).
struct SpeedLog {
    samples: Vec<f64>,
}

impl SpeedLog {
    async fn add(&mut self, db: &sinephile_persistence::Db, speed: f64) {
        if speed <= 0.0 {
            return;
        }
        self.samples.push(speed);
        if self.samples.len() < 60 {
            return;
        }
        let mean = self.samples.iter().sum::<f64>() / self.samples.len() as f64;
        self.samples.clear();
        let hourly = measure::from_json(&setting(db, HOURLY).await.unwrap_or_default());
        let hour = measure::local_hour(unix_now(), measure::utc_offset_seconds());
        let hourly = measure::fold(hourly, hour, mean);
        let _ = ProfileRepository::new(db)
            .set_setting(HOURLY, &measure::to_json(&hourly))
            .await;
    }
}

/// Once a second: start what should run, follow what runs, file what finished. The
/// engine is only started when there is something to download.
pub async fn downloads_loop(app: AppHandle) {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let mut log = SpeedLog {
        samples: Vec::new(),
    };
    loop {
        tick.tick().await;
        let Some(db) = app.state::<AppState>().db() else {
            continue;
        };
        if MANAGER.lock().await.is_none() {
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
                // Completion notices, by title: the page shows them wherever the viewer is.
                let repo = DownloadRepository::new(&db);
                for (event, ids) in [
                    ("download-finished", &t.finished),
                    ("download-failed", &t.failed),
                ] {
                    for id in ids {
                        if let Ok(Some(d)) = repo.get(*id).await {
                            let _ = app.emit(event, d.title);
                        }
                    }
                }
            }
            Err(error) => tracing::warn!(%error, "downloads tick failed"),
        }
        log.add(&db, m.speed()).await;
        let _ = app.emit("downloads-changed", ());
    }
}

/// One download as the Downloads page lists it, with its live rate.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct DownloadRow {
    pub download: DownloadView,
    /// Bytes per second now (0 unless it is running).
    pub speed: f64,
    pub peers: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn downloads(state: State<'_, AppState>) -> Result<Vec<DownloadRow>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let live = match MANAGER.lock().await.as_ref() {
        Some(m) => m.live(),
        None => Default::default(),
    };
    DownloadRepository::new(&db)
        .all()
        .await
        .map(|all| {
            all.into_iter()
                .filter(|d| d.state != DownloadState::Cancelled)
                .map(|d| {
                    let now = live.get(&d.id);
                    DownloadRow {
                        speed: now.map_or(0.0, |p| p.speed),
                        peers: now.map_or(0, |p| p.peers),
                        download: DownloadView::from(d),
                    }
                })
                .collect()
        })
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

/// The advisor's answer for the title whose page is open (SPEC.md Phase 13).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AdviceView {
    /// "play-local" | "stream" | "download" | "download-tonight"
    pub choice: String,
    pub sentence: String,
    /// The estimated chance a stream pauses, 0 to 1.
    pub stall: Option<f64>,
    pub size_bytes: Option<f64>,
    /// This title's download, if it is already queued, running or done.
    pub download: Option<DownloadView>,
}

fn choice_name(c: Choice) -> &'static str {
    match c {
        Choice::PlayLocal => "play-local",
        Choice::Stream => "stream",
        Choice::Download => "download",
        Choice::DownloadTonight => "download-tonight",
    }
}

/// What the last plan chose, for the title it was made for.
struct Chosen {
    title: String,
    stream: Option<PlayableStream>,
    size: Option<u64>,
    local: bool,
    runtime: Option<f64>,
}

/// What the last plan chose for `id`, if the page asking is the one that planned.
fn planned_for(id: i64) -> Option<Chosen> {
    let held = crate::commands::playback::PLANNED.lock().ok()?;
    let p = held.as_ref().filter(|p| p.media_item_id == id)?;
    Some(Chosen {
        title: p.title.clone(),
        stream: p.stream.clone(),
        size: p.size_bytes,
        local: p.local,
        runtime: p.runtime_seconds,
    })
}

async fn existing(db: &sinephile_persistence::Db, id: i64) -> Option<Download> {
    DownloadRepository::new(db)
        .all()
        .await
        .ok()?
        .into_iter()
        .find(|d| {
            d.media_item_id == Some(id)
                && !matches!(d.state, DownloadState::Cancelled | DownloadState::Failed)
        })
}

fn tonight_iso() -> String {
    sinephile_downloads::manager::iso_utc(measure::tonight(
        unix_now(),
        measure::utc_offset_seconds(),
    ))
}

/// Stream or download? Measured on the swarm the page already started (D61): three
/// seconds of the engine's rate, the later half kept. `None` when there is nothing to
/// advise on: no plan for this title, a copy that is not a torrent, or no runtime to
/// judge a stream by.
#[tauri::command]
#[specta::specta]
pub async fn download_advice(
    state: State<'_, AppState>,
    id: i32,
) -> Result<Option<AdviceView>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let id = i64::from(id);
    let download = existing(&db, id).await.map(DownloadView::from);
    let Some(chosen) = planned_for(id) else {
        return Ok(None);
    };
    let free = measure::free_space(&downloads_dir()).unwrap_or(0);
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(|e| e.to_string())?;
    let kept = storage::stored(&db, profile, unix_now())
        .await
        .map_err(|e| e.to_string())?;
    let used = kept.iter().map(|s| s.bytes).sum();
    let hourly = measure::from_json(&setting(&db, HOURLY).await.unwrap_or_default());
    let mut inputs = Inputs {
        file_bytes: chosen.size.unwrap_or(0),
        runtime_seconds: chosen.runtime.unwrap_or(0.0),
        buffer_seconds: PLAYER_BUFFER,
        swarm: None,
        local_copy: chosen.local,
        free_disk_bytes: storage::room(free, used, budget_bytes(&db).await),
        hourly: measure::rates(&hourly),
        hour_now: measure::local_hour(unix_now(), measure::utc_offset_seconds()),
        taste: Taste::neutral(),
    };
    let view = |advice: sinephile_downloads::advise::Advice, size: Option<u64>| AdviceView {
        choice: choice_name(advice.choice).into(),
        sentence: advice.sentence,
        stall: advice.stall,
        size_bytes: size.map(|s| s as f64),
        download: download.clone(),
    };
    if chosen.local {
        return Ok(Some(view(advise(&inputs), chosen.size)));
    }
    let Some(PlayableStream::Torrent { info_hash, .. }) = chosen.stream else {
        return Ok(None);
    };
    let Some(rt) = state.torrent_if_started() else {
        return Ok(None);
    };
    let hash = info_hash.to_ascii_lowercase();
    let mut rates = Vec::new();
    let mut peers = 0;
    let mut total = 0;
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let Ok(s) = rt.engine.status(&hash) else {
            break;
        };
        rates.push(s.download_bps);
        peers = peers.max(s.live_peers);
        total = s.total_bytes;
        if s.finished {
            break;
        }
    }
    let size = chosen.size.or((total > 0).then_some(total));
    let (Some(bytes), Some(_)) = (size, chosen.runtime) else {
        return Ok(None);
    };
    inputs.file_bytes = bytes;
    // Nothing sampled is nobody sharing: the advisor says so.
    inputs.swarm = Some(measure::swarm_from(&rates, peers).unwrap_or(
        sinephile_downloads::advise::Swarm {
            throughput: 0.0,
            spread: 0.0,
            peers: 0,
        },
    ));
    Ok(Some(view(advise(&inputs), size)))
}

async fn budget_bytes(db: &sinephile_persistence::Db) -> Option<u64> {
    setting(db, BUDGET_GIB)
        .await
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|g| *g > 0.0)
        .map(|g| (g * 1024.0 * 1024.0 * 1024.0) as u64)
}

/// Download the copy the last plan chose for `id`: now, or from one in the morning. The
/// advisor's sentence is kept as the reason. A title already downloading is not added
/// twice.
#[tauri::command]
#[specta::specta]
pub async fn download_title(
    state: State<'_, AppState>,
    id: i32,
    tonight: bool,
    reason: Option<String>,
) -> Result<DownloadView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let id = i64::from(id);
    if let Some(d) = existing(&db, id).await {
        return Ok(d.into());
    }
    let chosen = planned_for(id).ok_or("open the title's page again")?;
    let Some(PlayableStream::Torrent {
        info_hash,
        trackers,
        ..
    }) = chosen.stream
    else {
        return Err("this copy is a web stream, which cannot be downloaded".into());
    };
    let source = source_for(&info_hash, &trackers);
    let not_before = tonight.then(tonight_iso);
    let repo = DownloadRepository::new(&db);
    let new = repo
        .add(&NewDownload {
            media_item_id: Some(id),
            title: &chosen.title,
            source: &source,
            torrent: None,
            backend: "librqbit",
            priority: 0,
            limit_kib: None,
            not_before: not_before.as_deref(),
            reason: reason.as_deref(),
        })
        .await
        .map_err(|e| e.to_string())?;
    repo.get(new)
        .await
        .map_err(|e| e.to_string())?
        .map(DownloadView::from)
        .ok_or_else(|| "the download was not recorded".into())
}

/// Higher runs first.
#[tauri::command]
#[specta::specta]
pub async fn download_priority(
    state: State<'_, AppState>,
    id: i32,
    priority: i32,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    DownloadRepository::new(&db)
        .set_priority(i64::from(id), i64::from(priority))
        .await
        .map_err(|e| e.to_string())
}

/// This download's own speed limit, KiB/s; `None` for none.
#[tauri::command]
#[specta::specta]
pub async fn download_limit(app: AppHandle, id: i32, limit_kib: Option<i32>) -> Result<(), String> {
    manager(&app)
        .await?
        .set_limit(i64::from(id), limit_kib.map(i64::from))
        .await
        .map_err(|e| e.to_string())
}

/// Start it from one in the morning (`true`), or as soon as there is room (`false`). A
/// running download moved to tonight stops and waits like any other.
#[tauri::command]
#[specta::specta]
pub async fn download_tonight(app: AppHandle, id: i32, tonight: bool) -> Result<(), String> {
    let m = manager(&app).await?;
    let at = tonight.then(tonight_iso);
    m.schedule(i64::from(id), at.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct DownloadsSettingsView {
    pub max_active: u32,
    pub budget_gib: Option<f64>,
    pub qbittorrent_address: Option<String>,
    pub qbittorrent_user: Option<String>,
    pub qbittorrent_has_password: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn downloads_settings(
    state: State<'_, AppState>,
) -> Result<DownloadsSettingsView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(|e| e.to_string())?;
    let creds = CredentialRepository::new(&db);
    let plain = |v: Option<String>| v.filter(|s| !s.is_empty());
    Ok(DownloadsSettingsView {
        max_active: setting(&db, MAX_ACTIVE)
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(3),
        budget_gib: setting(&db, BUDGET_GIB)
            .await
            .and_then(|v| v.parse().ok())
            .filter(|g: &f64| *g > 0.0),
        qbittorrent_address: plain(creds.get(profile, QBIT_ADDRESS).await.ok().flatten()),
        qbittorrent_user: plain(creds.get(profile, QBIT_USER).await.ok().flatten()),
        qbittorrent_has_password: creds
            .secret(profile, QBIT_PASSWORD)
            .await
            .ok()
            .flatten()
            .is_some(),
    })
}

/// How many run at once (1 to 10) and the size budget (GiB; `None` for the whole disk).
#[tauri::command]
#[specta::specta]
pub async fn set_downloads_settings(
    state: State<'_, AppState>,
    max_active: u32,
    budget_gib: Option<f64>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    if !(1..=10).contains(&max_active) {
        return Err("between 1 and 10 at once".into());
    }
    let profiles = ProfileRepository::new(&db);
    profiles
        .set_setting(MAX_ACTIVE, &max_active.to_string())
        .await
        .map_err(|e| e.to_string())?;
    let budget = budget_gib.filter(|g| *g > 0.0).map(|g| g.to_string());
    profiles
        .set_setting(BUDGET_GIB, budget.as_deref().unwrap_or(""))
        .await
        .map_err(|e| e.to_string())?;
    rebuild_manager().await;
    Ok(())
}

/// The user's own qBittorrent (ADR-0029): its Web API address, user and password. An
/// empty address removes it; `password: None` keeps the one stored, `Some("")` clears it.
#[tauri::command]
#[specta::specta]
pub async fn set_qbittorrent(
    state: State<'_, AppState>,
    address: String,
    user: String,
    password: Option<String>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let address = address.trim();
    if !address.is_empty() {
        let ok = url::Url::parse(address)
            .map(|u| matches!(u.scheme(), "http" | "https"))
            .unwrap_or(false);
        if !ok {
            return Err("an address like http://localhost:8080".into());
        }
    }
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(|e| e.to_string())?;
    let creds = CredentialRepository::new(&db);
    let e = |e: sinephile_persistence::DbError| e.to_string();
    creds.set(profile, QBIT_ADDRESS, address).await.map_err(e)?;
    creds
        .set(profile, QBIT_USER, user.trim())
        .await
        .map_err(e)?;
    match password.as_deref() {
        _ if address.is_empty() => {
            creds
                .clear_secret(profile, QBIT_PASSWORD)
                .await
                .map_err(e)?;
        }
        Some("") => {
            creds
                .clear_secret(profile, QBIT_PASSWORD)
                .await
                .map_err(e)?;
        }
        Some(p) => creds
            .set_secret(profile, QBIT_PASSWORD, p)
            .await
            .map_err(e)?,
        None => {}
    }
    rebuild_manager().await;
    Ok(())
}

/// Whether the configured qBittorrent answers and accepts the login.
#[tauri::command]
#[specta::specta]
pub async fn qbittorrent_check(app: AppHandle) -> Result<bool, String> {
    Ok(manager(&app).await?.backend_available("qbittorrent").await)
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct StoredView {
    pub download: DownloadView,
    pub bytes: f64,
    pub age_days: f64,
    pub watched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct StorageView {
    pub used_bytes: f64,
    pub free_bytes: Option<f64>,
    pub budget_bytes: Option<f64>,
    /// Finished downloads, the best to clear first.
    pub items: Vec<StoredView>,
}

#[tauri::command]
#[specta::specta]
pub async fn downloads_storage(state: State<'_, AppState>) -> Result<StorageView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(|e| e.to_string())?;
    let kept = storage::stored(&db, profile, unix_now())
        .await
        .map_err(|e| e.to_string())?;
    Ok(StorageView {
        used_bytes: kept.iter().map(|s| s.bytes as f64).sum(),
        free_bytes: measure::free_space(&downloads_dir()).map(|f| f as f64),
        budget_bytes: budget_bytes(&db).await.map(|b| b as f64),
        items: kept
            .into_iter()
            .map(|s| StoredView {
                bytes: s.bytes as f64,
                age_days: s.age_days,
                watched: s.watched,
                download: s.download.into(),
            })
            .collect(),
    })
}

/// Delete a finished download from disk and from the library.
#[tauri::command]
#[specta::specta]
pub async fn download_clear(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i32,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    storage::clear(&db, i64::from(id), &downloads_dir())
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("library-changed", 0);
    let _ = app.emit("downloads-changed", ());
    Ok(())
}
