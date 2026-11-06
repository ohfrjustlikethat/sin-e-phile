//! The player IPC surface (SPEC.md Phase 8). **Thin**: libmpv, the video window and its
//! cutouts are `crates/player`; these hold one player and forward calls to it.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use sinephile_player::{Options, Placement, Player, VideoHost};

use crate::state::AppState;

/// The one player, created on first use and attached to the main window.
pub struct PlayerRuntime {
    pub player: Player,
    pub host: Mutex<VideoHost>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Get the player, creating it (and its video window) the first time.
fn runtime(app: &AppHandle, state: &AppState) -> Result<Arc<PlayerRuntime>, String> {
    if let Some(rt) = state.player() {
        return Ok(rt);
    }
    let window = app.get_webview_window("main").ok_or("no main window")?;
    let parent = window.hwnd().map_err(err)?.0 as isize;
    // The video window must belong to the UI thread, which pumps its messages; this
    // command runs on a worker (see `VideoHost::new`).
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(VideoHost::new(parent));
    })
    .map_err(err)?;
    let mut host = rx.recv().map_err(err)??;
    if let Some(p) = LAST_PLACEMENT.lock().expect("placement").as_ref() {
        host.place(p)?;
    }
    let options = Options {
        wid: Some(host.wid()),
        headless: false,
        hwdec: true,
        cache_dir: Some(crate::logging::data_dir().join("cache").join("mpv")),
    };
    let player = Player::new(&sinephile_player::dll_path(), &options).map_err(err)?;
    let rt = Arc::new(PlayerRuntime {
        player,
        host: Mutex::new(host),
    });
    state.set_player(rt.clone());
    Ok(rt)
}

/// Open something to play: an `http(s)` address or a file path. `start` resumes.
#[tauri::command]
#[specta::specta]
pub async fn player_open(
    app: AppHandle,
    state: State<'_, AppState>,
    target: String,
    start: Option<f64>,
) -> Result<(), String> {
    tracing::info!(%target, ?start, "player open");
    let t0 = std::time::Instant::now();
    let rt = runtime(&app, &state)
        .inspect_err(|e| tracing::error!(error = %e, "player could not start"))?;
    let ready_ms = t0.elapsed().as_millis();
    rt.player.load(&target, start).map_err(err)?;
    rt.host.lock().expect("host").show();
    // §2.3's play → first frame budget, measured where it is spent.
    let waiting = rt.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match waiting
            .player
            .wait_first_frame(std::time::Duration::from_secs(60))
        {
            Ok(_) => tracing::info!(
                ready_ms,
                first_frame_ms = t0.elapsed().as_millis(),
                "first frame"
            ),
            Err(e) => tracing::warn!(error = %e, "no first frame"),
        }
    });
    Ok(())
}

/// A chrome panel's silhouette in CSS pixels; the page converts nothing, this does.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CutoutRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub radius: f64,
}

/// Where the page last asked the video to sit. The page lays out on mount, before
/// `player_open` has created the video window, so the request is kept and applied the
/// moment the window exists — otherwise it stays at its 1×1 creation size, and both the
/// picture and the pause still are a single pixel.
static LAST_PLACEMENT: Mutex<Option<Placement>> = Mutex::new(None);

/// Place the video and cut the chrome out of it. Coordinates are CSS pixels within the
/// window's client area; `scale` is the page's `devicePixelRatio`.
#[tauri::command]
#[specta::specta]
pub async fn player_layout(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
    cutouts: Vec<CutoutRect>,
) -> Result<(), String> {
    let placement = Placement {
        x,
        y,
        width,
        height,
        scale,
        cutouts: cutouts
            .iter()
            .map(|c| (c.x, c.y, c.width, c.height, c.radius))
            .collect(),
    };
    *LAST_PLACEMENT.lock().expect("placement") = Some(placement.clone());
    match state.player() {
        Some(rt) => rt.host.lock().expect("host").place(&placement),
        None => Ok(()),
    }
}

/// One control action. A single command for all of them keeps the IPC surface small;
/// each maps to one `Player` method.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerAction {
    Pause { paused: bool },
    SeekTo { seconds: f64 },
    SeekBy { seconds: f64 },
    FrameStep { forward: bool },
    Volume { percent: f64 },
    Mute { muted: bool },
    Speed { speed: f64 },
    Audio { id: i32 },
    Subtitles { id: Option<i32> },
}

#[tauri::command]
#[specta::specta]
pub async fn player_action(state: State<'_, AppState>, action: PlayerAction) -> Result<(), String> {
    let rt = state.player().ok_or("nothing is playing")?;
    let p = &rt.player;
    match action {
        PlayerAction::Pause { paused } => p.set_paused(paused),
        PlayerAction::SeekTo { seconds } => p.seek_to(seconds),
        PlayerAction::SeekBy { seconds } => p.seek_by(seconds),
        PlayerAction::FrameStep { forward } => p.frame_step(forward),
        PlayerAction::Volume { percent } => p.set_volume(percent),
        PlayerAction::Mute { muted } => p.set_muted(muted),
        PlayerAction::Speed { speed } => p.set_speed(speed),
        PlayerAction::Audio { id } => p.set_audio(i64::from(id)),
        PlayerAction::Subtitles { id } => p.set_subtitles(id.map(i64::from)),
    }
    .map_err(err)
}

/// Pause, and hand the page the frame on screen so it can draw the pause overlay over
/// it while the video window is hidden (ADR-0021). Returns a `data:` URI.
#[tauri::command]
#[specta::specta]
pub async fn player_pause_still(state: State<'_, AppState>) -> Result<String, String> {
    let rt = state.player().ok_or("nothing is playing")?;
    rt.player.set_paused(true).map_err(err)?;
    let path = std::env::temp_dir().join(format!("sinephile-still-{}.jpg", std::process::id()));
    let jpeg = rt
        .player
        .capture_still(&path, std::time::Duration::from_millis(500))
        .map_err(err)?;
    rt.host.lock().expect("host").hide();
    Ok(sinephile_player::still::data_uri(&jpeg))
}

/// Show the video again after the pause overlay.
#[tauri::command]
#[specta::specta]
pub async fn player_resume_video(state: State<'_, AppState>) -> Result<(), String> {
    let rt = state.player().ok_or("nothing is playing")?;
    rt.host.lock().expect("host").show();
    rt.player.set_paused(false).map_err(err)
}

/// Stop and hide the video.
#[tauri::command]
#[specta::specta]
pub async fn player_close(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(rt) = state.player() {
        rt.host.lock().expect("host").hide();
        rt.player.stop().map_err(err)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TrackView {
    pub id: i32,
    pub kind: String,
    pub lang: Option<String>,
    pub title: Option<String>,
    pub selected: bool,
}

/// The player as the chrome draws it, a few times a second.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PlayerStateView {
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    pub buffered_until: Option<f64>,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub hwdec: Option<String>,
    pub ended: bool,
    pub tracks: Vec<TrackView>,
}

fn view(rt: &PlayerRuntime) -> PlayerStateView {
    let s = rt.player.state();
    PlayerStateView {
        position: s.position,
        duration: s.duration,
        paused: s.paused,
        buffered_until: s.buffered_until,
        volume: s.volume,
        muted: s.muted,
        speed: s.speed,
        hwdec: s.hwdec,
        ended: s.ended,
        tracks: rt
            .player
            .tracks()
            .into_iter()
            .map(|t| TrackView {
                id: t.id as i32,
                kind: t.kind,
                lang: t.lang,
                title: t.title,
                selected: t.selected,
            })
            .collect(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn player_state(state: State<'_, AppState>) -> Result<Option<PlayerStateView>, String> {
    Ok(state.player().map(|rt| view(&rt)))
}

/// `player-state`, four times a second while a player exists.
pub async fn state_loop(app: AppHandle) {
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tick.tick().await;
        if let Some(rt) = app.state::<AppState>().player() {
            let _ = app.emit("player-state", view(&rt));
        }
    }
}

/// Debug builds only: `SINEPHILE_AUTOPLAY=<address or path>` plays it on launch, so the
/// real window can be checked without driving the UI. Compiled out of release builds.
#[tauri::command]
#[specta::specta]
pub async fn dev_autoplay() -> Result<Option<String>, String> {
    #[cfg(debug_assertions)]
    {
        let v = std::env::var("SINEPHILE_AUTOPLAY").ok();
        tracing::info!(autoplay = ?v, "dev autoplay asked");
        Ok(v)
    }
    #[cfg(not(debug_assertions))]
    {
        Ok(None)
    }
}
