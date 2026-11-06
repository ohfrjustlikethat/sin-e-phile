//! Subtitles (SPEC.md Phase 10). **Thin**: the chain, alignment, quota and nudges are
//! `crates/subtitles`; this reads the settings, hands the crate what the player knows,
//! and applies its answer to the player.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::{
    CredentialRepository, ProfileRepository, SubtitleRepository,
};
use sinephile_player::SubtitleStyle;
use sinephile_subtitles::opensubtitles::{OpenSubtitles, FREE_DAILY};
use sinephile_subtitles::plan::{self, Alignment, Ffmpeg, Film, Show, Track, Wants};

use crate::commands::player::PlayerRuntime;
use crate::state::AppState;
use crate::tiers::Capability;

const SETTINGS_KEY: &str = "subtitles.settings";
/// Ten minutes of audio is what alignment hears: enough for confidence (the corpus is
/// 8–12 minute films), little enough to extract in seconds.
const ALIGN_SECONDS: u32 = 600;

/// The viewer's subtitle settings, per profile.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SubtitleSettings {
    /// Languages, most preferred first (`en`, `fr`).
    pub languages: Vec<String>,
    /// Subtitles even when the audio is in one of `languages`.
    pub always: bool,
    pub font: String,
    pub size: u32,
    pub position: u32,
    pub background: bool,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        let s = SubtitleStyle::default();
        Self {
            languages: vec!["en".into()],
            always: false,
            font: s.font,
            size: s.size,
            position: s.position,
            background: s.background,
        }
    }
}

impl SubtitleSettings {
    fn style(&self) -> SubtitleStyle {
        SubtitleStyle {
            font: self.font.clone(),
            size: self.size,
            position: self.position,
            background: self.background,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SubtitleSettingsView {
    pub settings: SubtitleSettings,
    /// Whether this profile has its own OpenSubtitles key.
    pub has_key: bool,
    /// Downloads spent today, and the daily budget.
    pub used_today: u32,
    pub budget: u32,
    /// What OpenSubtitles last said is left today, if it said.
    pub server_remaining: Option<u32>,
}

/// What is showing now, for the nudge: (file, subtitle, nudge in ms).
static CURRENT: Mutex<Option<(String, String, i64)>> = Mutex::new(None);

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    sinephile_subtitles::utc_day(now)
}

pub(crate) async fn settings(state: &AppState) -> Result<SubtitleSettings, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(err)?;
    let raw = CredentialRepository::new(&db)
        .get(profile, SETTINGS_KEY)
        .await
        .map_err(err)?;
    Ok(raw
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default())
}

#[tauri::command]
#[specta::specta]
pub async fn subtitle_settings(state: State<'_, AppState>) -> Result<SubtitleSettingsView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(err)?;
    let has_key = CredentialRepository::new(&db)
        .opensubtitles_key(profile)
        .await
        .map_err(err)?
        .is_some();
    let q = SubtitleRepository::new(&db)
        .quota("opensubtitles", &today())
        .await
        .map_err(err)?;
    Ok(SubtitleSettingsView {
        settings: settings(&state).await?,
        has_key,
        used_today: q.used.max(0) as u32,
        budget: FREE_DAILY as u32,
        server_remaining: q.server_remaining.map(|r| r.max(0) as u32),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn set_subtitle_settings(
    state: State<'_, AppState>,
    settings: SubtitleSettings,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(err)?;
    let json = serde_json::to_string(&settings).map_err(err)?;
    CredentialRepository::new(&db)
        .set(profile, SETTINGS_KEY, &json)
        .await
        .map_err(err)?;
    // The style shows at once if something is playing.
    if let Some(rt) = state.player() {
        rt.player
            .set_subtitle_style(&settings.style())
            .map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_opensubtitles_key(state: State<'_, AppState>, key: String) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(err)?;
    CredentialRepository::new(&db)
        .set_opensubtitles_key(profile, &key)
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub async fn clear_opensubtitles_key(state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(err)?;
    CredentialRepository::new(&db)
        .clear_opensubtitles_key(profile)
        .await
        .map_err(err)
}

/// Move the subtitles by `delta_ms` (G earlier, H later) and remember it for this file and
/// subtitle (Phase 10 E5). Returns the nudge now in force.
#[tauri::command]
#[specta::specta]
pub async fn subtitle_nudge(state: State<'_, AppState>, delta_ms: i32) -> Result<i32, String> {
    let rt = state.player().ok_or("nothing is playing")?;
    let (file, sub, total) = {
        let mut current = CURRENT.lock().expect("current");
        let Some((file, sub, nudge)) = current.as_mut() else {
            return Err("no subtitles are showing".into());
        };
        *nudge += i64::from(delta_ms);
        (file.clone(), sub.clone(), *nudge)
    };
    rt.player
        .set_subtitle_delay(total as f64 / 1000.0)
        .map_err(err)?;
    let db = state.db().ok_or("the library is still opening")?;
    SubtitleRepository::new(&db)
        .set_nudge(&file, &sub, total)
        .await
        .map_err(err)?;
    Ok(total as i32)
}

/// Once the first frame is up: decide the subtitle, align it if it needs it, and show it.
/// Runs on a blocking thread: extracting audio and aligning take seconds. `current` says
/// whether this film is still the one playing when the answer is ready.
pub(crate) async fn open_for(
    state: &AppState,
    rt: Arc<PlayerRuntime>,
    target: String,
    current: impl Fn() -> bool + Send + 'static,
) -> Result<(), String> {
    *CURRENT.lock().expect("current") = None;
    let db = state.db().ok_or("the library is still opening")?;
    let wanted = settings(state).await?;
    rt.player.set_subtitle_style(&wanted.style()).map_err(err)?;
    let profile = ProfileRepository::new(&db)
        .ensure_default()
        .await
        .map_err(err)?;
    let key = CredentialRepository::new(&db)
        .opensubtitles_key(profile)
        .await
        .map_err(err)?;
    let http: Arc<dyn sinephile_metadata_api::Transport> = state.http.clone();
    let provider = key.map(|k| OpenSubtitles::new(http, k));
    let vad = state
        .effective_tier()
        .allows(Capability::VadSubtitleAlignment);
    let data = crate::logging::data_dir();
    let player = rt.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tracks = player.player.tracks();
        let audio = tracks
            .iter()
            .find(|t| t.kind == "audio" && t.selected)
            .and_then(|t| t.lang.clone());
        let local = std::path::Path::new(&target);
        let film = Film {
            target: &target,
            local: local.is_file().then_some(local),
            audio_language: audio.as_deref(),
            tracks: tracks
                .iter()
                // Only tracks inside the file: an external one is a file the chain
                // already considers (and aligns).
                .filter(|t| t.kind == "sub" && !t.external)
                .map(|t| Track {
                    id: t.id,
                    language: t.lang.clone(),
                    forced: t.forced,
                })
                .collect(),
            torrent: vec![],
        };
        let speech = Ffmpeg {
            target: &target,
            seconds: ALIGN_SECONDS,
        };
        let alignment = if vad {
            Alignment::Speech(&speech)
        } else {
            Alignment::Duration(player.player.state().duration.map(|d| (d * 1000.0) as i64))
        };
        let wants = Wants {
            languages: wanted.languages.clone(),
            always: wanted.always,
        };
        let decided = tauri::async_runtime::block_on(plan::plan(
            &db,
            &data,
            provider.as_ref(),
            &film,
            &wants,
            alignment,
            &today(),
        ))
        .map_err(err)?;
        if !current() {
            return Ok(());
        }
        let p = &player.player;
        match &decided.show {
            Show::Off { why } => {
                tracing::info!(%why, "subtitles off");
                p.set_subtitles(None).map_err(err)?;
            }
            Show::Embedded { track_id } => {
                tracing::info!(track_id, "subtitles: the embedded track");
                p.set_subtitles(Some(*track_id)).map_err(err)?;
            }
            Show::File {
                path,
                language,
                note,
            } => {
                tracing::info!(path = %path.display(), %note, "subtitles: a file");
                p.add_subtitle(&path.to_string_lossy(), "sin-e-phile", language.as_deref())
                    .map_err(err)?;
            }
        }
        if let Some(sub) = decided.subtitle_key.clone() {
            p.set_subtitle_delay(decided.nudge_ms as f64 / 1000.0)
                .map_err(err)?;
            *CURRENT.lock().expect("current") = Some((decided.file_key, sub, decided.nudge_ms));
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(err)?
}
