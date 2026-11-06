//! The player: libmpv behind a small, safe API (SPEC.md Phase 8, ADR-0004).
//!
//! - Commands and property reads come from any thread; one event thread pumps
//!   `mpv_wait_event` and records what happened (see the SAFETY note in `ffi.rs`).
//! - mpv is configured for an embedded player, not a standalone one: no default key
//!   bindings, no on-screen controller, no config files or scripts from the user's
//!   profile, and **no youtube-dl** — the player opens exactly the address it is given
//!   and never goes looking for content itself (SPEC.md §2.1).
//! - Seeks are exact (`hr-seek`), so "seeking is frame-accurate" (E3) is a property of
//!   every seek, not a mode.

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::ffi::{self, Mpv};

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("the player library could not be loaded: {0}")]
    Load(String),
    #[error("{0}")]
    Mpv(String),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
    #[error("could not play this: {0}")]
    Playback(String),
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The native window to render into. `None` with `headless` renders nowhere.
    pub wid: Option<i64>,
    /// No video or audio output at all — for tests and measurements.
    pub headless: bool,
    /// Hardware decoding (`auto-safe`). Off forces software decode.
    pub hwdec: bool,
    /// Where mpv keeps compiled GPU shaders between runs. Without it (`config=no` leaves
    /// none) every launch recompiles them before the first frame.
    pub cache_dir: Option<std::path::PathBuf>,
}

/// What the event thread has seen since the last `load`.
#[derive(Debug, Clone, Default)]
struct Seen {
    loaded_at: Option<Instant>,
    first_frame_at: Option<Instant>,
    ended: Option<(i32, String)>,
    started_at: Option<Instant>,
    shutdown: bool,
    /// How many times playback has (re)started: once after load, once after each seek.
    restarts: u64,
}

/// A track as the UI lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    /// `video`, `audio` or `sub`.
    pub kind: String,
    pub lang: Option<String>,
    pub title: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
    /// A forced subtitle: only the foreign-language lines.
    #[serde(default)]
    pub forced: bool,
    /// Loaded from a separate file rather than inside the media.
    #[serde(default)]
    pub external: bool,
}

/// A chapter mark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    /// Seconds from the start.
    pub time: f64,
    pub title: Option<String>,
}

/// How plain subtitles look (Settings → Subtitles).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleStyle {
    pub font: String,
    /// mpv's scale: 55 is its default, at 720 lines.
    pub size: u32,
    /// 0 top … 100 bottom.
    pub position: u32,
    /// A dark box behind the text instead of an outline.
    pub background: bool,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            font: "Segoe UI".into(),
            size: 46,
            position: 95,
            background: false,
        }
    }
}

/// A snapshot of the player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    /// The furthest point buffered ahead, in seconds of media time.
    pub buffered_until: Option<f64>,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    /// The hardware decoder in use, e.g. `d3d11va`; `None` is software decode.
    pub hwdec: Option<String>,
    pub idle: bool,
    pub ended: bool,
    /// A seek is still in progress. mpv reports the TARGET as `position` as soon as a
    /// seek starts, so "done" is `!seeking`, not "the position reads right".
    pub seeking: bool,
    /// Frames the video output dropped since the file opened: the stutter measure
    /// (Phase 11 E2) and the stats overlay's.
    #[serde(default)]
    pub dropped_frames: Option<i64>,
    /// The decoded picture's size, in pixels.
    #[serde(default)]
    pub video_size: Option<(i64, i64)>,
    /// The chapter playing, as an index into [`Player::chapters`].
    #[serde(default)]
    pub chapter: Option<i64>,
    /// The first frame of what was last loaded is on screen. Until then the screen is
    /// the page's "opening" state, not a black video window.
    pub started: bool,
}

pub struct Player {
    mpv: Arc<Mpv>,
    seen: Arc<(Mutex<Seen>, Condvar)>,
    events: Option<JoinHandle<()>>,
}

impl Player {
    pub fn new(dll: &Path, options: &Options) -> Result<Self, PlayerError> {
        let mpv = Mpv::load(dll).map_err(PlayerError::Load)?;
        let set = |k: &str, v: &str| mpv.set_option(k, v).map_err(PlayerError::Mpv);
        set("config", "no")?;
        set("load-scripts", "no")?;
        set("ytdl", "no")?;
        set("input-default-bindings", "no")?;
        set("input-vo-keyboard", "no")?;
        set("osc", "no")?;
        set("osd-level", "0")?;
        set("terminal", "no")?;
        set("idle", "yes")?;
        set("keep-open", "yes")?;
        set("hr-seek", "yes")?;
        set("cache", "yes")?;
        // mpv would load a subtitle beside the file by itself, unaligned, and report it as
        // a track. The subtitle chain (crates/subtitles) finds, aligns and loads those.
        set("sub-auto", "no")?;
        set("demuxer-max-bytes", "256MiB")?;
        set("hwdec", if options.hwdec { "auto-safe" } else { "no" })?;
        if options.headless {
            set("vo", "null")?;
            set("ao", "null")?;
        } else {
            set("vo", "gpu-next")?;
        }
        if let Some(dir) = &options.cache_dir {
            let _ = std::fs::create_dir_all(dir);
            set("gpu-shader-cache-dir", &dir.to_string_lossy())?;
        }
        if let Some(wid) = options.wid {
            set("wid", &wid.to_string())?;
        }
        // Warnings by default; `SINEPHILE_MPV_LOG=v` (or `debug`) to diagnose playback.
        let level = std::env::var("SINEPHILE_MPV_LOG").unwrap_or_else(|_| "warn".into());
        mpv.request_log_messages(&level).map_err(PlayerError::Mpv)?;
        mpv.initialize().map_err(PlayerError::Mpv)?;

        let mpv = Arc::new(mpv);
        let seen: Arc<(Mutex<Seen>, Condvar)> = Arc::default();
        let events = {
            let (mpv, seen) = (mpv.clone(), seen.clone());
            std::thread::Builder::new()
                .name("mpv-events".into())
                .spawn(move || pump(&mpv, &seen))
                .map_err(|e| PlayerError::Mpv(e.to_string()))?
        };
        Ok(Self {
            mpv,
            seen,
            events: Some(events),
        })
    }

    fn cmd(&self, args: &[&str]) -> Result<(), PlayerError> {
        self.mpv.command(args).map_err(PlayerError::Mpv)
    }

    fn prop(&self, key: &str, value: &str) -> Result<(), PlayerError> {
        self.mpv.set_property(key, value).map_err(PlayerError::Mpv)
    }

    /// Open a URL or a path, optionally starting at `start` seconds (position resume).
    pub fn load(&self, target: &str, start: Option<f64>) -> Result<(), PlayerError> {
        {
            let mut s = self.seen.0.lock().expect("seen");
            *s = Seen {
                started_at: Some(Instant::now()),
                ..Default::default()
            };
        }
        match start.filter(|s| *s > 0.0) {
            Some(t) => {
                let opt = format!("start={t:.3}");
                self.cmd(&["loadfile", target, "replace", "-1", &opt])
            }
            None => self.cmd(&["loadfile", target, "replace"]),
        }
    }

    /// Wait for the file to load, or for it to fail with mpv's reason.
    pub fn wait_loaded(&self, timeout: Duration) -> Result<(), PlayerError> {
        self.wait(timeout, "the file to load", |s| {
            if let Some((reason, text)) = &s.ended {
                if *reason == 4 {
                    return Some(Err(PlayerError::Playback(if text.is_empty() {
                        "the file could not be opened".into()
                    } else {
                        text.clone()
                    })));
                }
            }
            s.loaded_at.map(|_| Ok(()))
        })
    }

    /// Time from `load` to the first frame shown (or decoded, headless).
    pub fn wait_first_frame(&self, timeout: Duration) -> Result<Duration, PlayerError> {
        self.wait(timeout, "the first frame", |s| {
            if let Some((4, text)) = &s.ended {
                return Some(Err(PlayerError::Playback(text.clone())));
            }
            match (s.started_at, s.first_frame_at) {
                (Some(a), Some(b)) => Some(Ok(b - a)),
                _ => None,
            }
        })
    }

    fn wait<T>(
        &self,
        timeout: Duration,
        what: &'static str,
        done: impl Fn(&Seen) -> Option<Result<T, PlayerError>>,
    ) -> Result<T, PlayerError> {
        let (lock, cv) = &*self.seen;
        let deadline = Instant::now() + timeout;
        let mut s = lock.lock().expect("seen");
        loop {
            if let Some(r) = done(&s) {
                return r;
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(PlayerError::Timeout(what));
            }
            s = cv.wait_timeout(s, deadline - now).expect("seen").0;
        }
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), PlayerError> {
        self.prop("pause", if paused { "yes" } else { "no" })
    }

    /// Seek to an absolute time, exactly.
    pub fn seek_to(&self, seconds: f64) -> Result<(), PlayerError> {
        self.cmd(&["seek", &format!("{seconds:.3}"), "absolute+exact"])
    }

    /// Seek to an absolute time and wait until mpv has finished: it sends one
    /// `playback-restart` per completed seek. Neither `position` (which reads the target
    /// at once) nor `seeking` (raised asynchronously) says that reliably; this does.
    pub fn seek_to_and_wait(&self, seconds: f64, timeout: Duration) -> Result<(), PlayerError> {
        let before = self.seen.0.lock().expect("seen").restarts;
        self.seek_to(seconds)?;
        self.wait(timeout, "the seek to finish", |s| {
            (s.restarts > before).then_some(Ok(()))
        })
    }

    /// Seek by `delta` seconds from the current position, exactly.
    pub fn seek_by(&self, delta: f64) -> Result<(), PlayerError> {
        self.cmd(&["seek", &format!("{delta:.3}"), "relative+exact"])
    }

    /// One frame forward or back (`,` and `.`).
    pub fn frame_step(&self, forward: bool) -> Result<(), PlayerError> {
        self.cmd(&[if forward {
            "frame-step"
        } else {
            "frame-back-step"
        }])
    }

    pub fn set_volume(&self, percent: f64) -> Result<(), PlayerError> {
        self.prop("volume", &format!("{:.0}", percent.clamp(0.0, 130.0)))
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), PlayerError> {
        self.prop("mute", if muted { "yes" } else { "no" })
    }

    pub fn set_speed(&self, speed: f64) -> Result<(), PlayerError> {
        self.prop("speed", &format!("{:.2}", speed.clamp(0.25, 4.0)))
    }

    /// Select an audio track by id.
    pub fn set_audio(&self, id: i64) -> Result<(), PlayerError> {
        self.prop("aid", &id.to_string())
    }

    /// Select a subtitle track by id, or `None` for no subtitles.
    pub fn set_subtitles(&self, id: Option<i64>) -> Result<(), PlayerError> {
        self.prop("sid", &id.map_or("no".to_string(), |i| i.to_string()))
    }

    /// Load a subtitle file and show it (SPEC.md Phase 10).
    pub fn add_subtitle(
        &self,
        path: &str,
        title: &str,
        lang: Option<&str>,
    ) -> Result<(), PlayerError> {
        self.cmd(&["sub-add", path, "select", title, lang.unwrap_or("")])
    }

    /// Shift the subtitles: positive shows them later. The viewer's nudge.
    pub fn set_subtitle_delay(&self, seconds: f64) -> Result<(), PlayerError> {
        self.prop("sub-delay", &format!("{seconds:.3}"))
    }

    /// Shift the sound: positive plays it later (Phase 11's audio delay).
    pub fn set_audio_delay(&self, seconds: f64) -> Result<(), PlayerError> {
        self.prop("audio-delay", &format!("{seconds:.3}"))
    }

    pub fn audio_delay(&self) -> f64 {
        self.num("audio-delay").unwrap_or(0.0)
    }

    /// Picture adjustments for a badly encoded source, each −100…100, 0 untouched.
    pub fn set_picture(
        &self,
        brightness: i64,
        contrast: i64,
        saturation: i64,
    ) -> Result<(), PlayerError> {
        self.prop("brightness", &brightness.clamp(-100, 100).to_string())?;
        self.prop("contrast", &contrast.clamp(-100, 100).to_string())?;
        self.prop("saturation", &saturation.clamp(-100, 100).to_string())
    }

    /// What the stats overlay shows: (property, value) in display order, only those mpv
    /// reports for this file.
    pub fn stats(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let mut put = |k: &'static str, v: Option<String>| {
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                out.push((k, v));
            }
        };
        put("Video", self.get("current-tracks/video/codec"));
        put(
            "Size",
            self.num("video-params/w")
                .zip(self.num("video-params/h"))
                .map(|(w, h)| format!("{w}×{h}")),
        );
        put(
            "Frame rate",
            self.num("container-fps").map(|f| format!("{f:.3} fps")),
        );
        put(
            "Decoder",
            Some(
                self.get("hwdec-current")
                    .filter(|h| h != "no")
                    .unwrap_or_else(|| "software".into()),
            ),
        );
        put(
            "Video bitrate",
            self.num("video-bitrate")
                .map(|b| format!("{:.1} Mbit/s", b / 1e6)),
        );
        put("Audio", self.get("audio-codec-name"));
        put(
            "Audio bitrate",
            self.num("audio-bitrate")
                .map(|b| format!("{:.0} kbit/s", b / 1e3)),
        );
        put(
            "Dropped frames",
            self.num("frame-drop-count").map(|n| format!("{n}")),
        );
        put(
            "Decoder drops",
            self.num("decoder-frame-drop-count").map(|n| format!("{n}")),
        );
        put(
            "Buffered ahead",
            self.num("demuxer-cache-duration")
                .map(|s| format!("{s:.1} s")),
        );
        put(
            "Audio delay",
            Some(format!("{:+.0} ms", self.audio_delay() * 1000.0)),
        );
        put(
            "Subtitle delay",
            Some(format!("{:+.0} ms", self.subtitle_delay() * 1000.0)),
        );
        out
    }

    pub fn subtitle_delay(&self) -> f64 {
        self.num("sub-delay").unwrap_or(0.0)
    }

    /// The viewer's subtitle style. Applies to plain subtitles only: an ASS/SSA file keeps
    /// its own typesetting (`sub-ass-override=no`), since it was styled on purpose.
    pub fn set_subtitle_style(&self, style: &SubtitleStyle) -> Result<(), PlayerError> {
        self.prop("sub-ass-override", "no")?;
        self.prop("sub-font", &style.font)?;
        self.prop("sub-font-size", &style.size.to_string())?;
        // 100 is the bottom edge; lower raises the line.
        self.prop("sub-pos", &style.position.clamp(0, 100).to_string())?;
        if style.background {
            self.prop("sub-border-style", "opaque-box")?;
            self.prop("sub-back-color", "#B0000000")
        } else {
            self.prop("sub-border-style", "outline-and-shadow")
        }
    }

    /// Save the frame on screen, at window resolution, to `path` (JPEG by extension).
    /// The pause overlay shows it while the video window is hidden (ADR-0021).
    pub fn screenshot(&self, path: &Path) -> Result<(), PlayerError> {
        let p = path.to_string_lossy();
        // "window" is what the viewer saw, at window resolution, so its cost does not
        // grow with a 4K source (ADR-0021). With no video output (headless) there is no
        // window, and the decoded frame is the next best thing.
        self.cmd(&["screenshot-to-file", &p, "window"])
            .or_else(|_| self.cmd(&["screenshot-to-file", &p, "video"]))
    }

    /// Capture the frame on screen and return it once mpv has finished writing it.
    pub fn capture_still(&self, path: &Path, timeout: Duration) -> Result<Vec<u8>, PlayerError> {
        let _ = std::fs::remove_file(path);
        self.screenshot(path)?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(bytes) = std::fs::read(path) {
                if crate::still::is_complete_jpeg(&bytes) {
                    let _ = std::fs::remove_file(path);
                    return Ok(bytes);
                }
            }
            if Instant::now() >= deadline {
                return Err(PlayerError::Timeout("the still frame"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn stop(&self) -> Result<(), PlayerError> {
        self.cmd(&["stop"])
    }

    fn get(&self, key: &str) -> Option<String> {
        self.mpv.get_property(key)
    }

    fn num(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    fn flag(&self, key: &str) -> bool {
        self.get(key).as_deref() == Some("yes")
    }

    pub fn state(&self) -> State {
        let hwdec = self
            .get("hwdec-current")
            .filter(|v| !v.is_empty() && v != "no");
        let buffered_until = self.num("demuxer-cache-time");
        State {
            position: self.num("time-pos"),
            duration: self.num("duration"),
            paused: self.flag("pause"),
            buffered_until,
            volume: self.num("volume").unwrap_or(100.0),
            muted: self.flag("mute"),
            speed: self.num("speed").unwrap_or(1.0),
            hwdec,
            idle: self.flag("idle-active"),
            ended: self.flag("eof-reached"),
            seeking: self.flag("seeking"),
            started: self.seen.0.lock().expect("seen").first_frame_at.is_some(),
            chapter: self.num("chapter").filter(|c| *c >= 0.0).map(|c| c as i64),
            dropped_frames: self.num("frame-drop-count").map(|n| n as i64),
            video_size: self
                .num("video-params/w")
                .zip(self.num("video-params/h"))
                .map(|(w, h)| (w as i64, h as i64)),
        }
    }

    /// The file's chapters, in order (Phase 11: the scrub bar's marks and the overlay's
    /// "current chapter").
    pub fn chapters(&self) -> Vec<Chapter> {
        let n = self.num("chapter-list/count").unwrap_or(0.0) as usize;
        (0..n)
            .map(|i| Chapter {
                time: self.num(&format!("chapter-list/{i}/time")).unwrap_or(0.0),
                title: self
                    .get(&format!("chapter-list/{i}/title"))
                    .filter(|t| !t.is_empty()),
            })
            .collect()
    }

    pub fn tracks(&self) -> Vec<Track> {
        let n = self.num("track-list/count").unwrap_or(0.0) as usize;
        (0..n)
            .map(|i| {
                let p = |k: &str| self.get(&format!("track-list/{i}/{k}"));
                Track {
                    id: p("id").and_then(|v| v.parse().ok()).unwrap_or(0),
                    kind: p("type").unwrap_or_default(),
                    lang: p("lang"),
                    title: p("title"),
                    codec: p("codec"),
                    selected: p("selected").as_deref() == Some("yes"),
                    forced: p("forced").as_deref() == Some("yes"),
                    external: p("external").as_deref() == Some("yes"),
                }
            })
            .collect()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.mpv.command(&["quit"]);
        if let Some(h) = self.events.take() {
            let _ = h.join();
        }
    }
}

/// The one thread that calls `mpv_wait_event`. It records load, first frame and end of
/// file, wakes anyone waiting, and passes mpv's warnings to the log.
fn pump(mpv: &Mpv, seen: &(Mutex<Seen>, Condvar)) {
    loop {
        let ffi::Event { id, log: text, end } = mpv.wait_event(0.25);
        if id == ffi::MPV_EVENT_NONE {
            continue;
        }
        if let Some(t) = text {
            tracing::warn!(target: "mpv", "{t}");
        }
        let mut s = seen.0.lock().expect("seen");
        match id {
            ffi::MPV_EVENT_FILE_LOADED => s.loaded_at = Some(Instant::now()),
            // The first "playback restart" after a load is the first frame on screen
            // (or decoded, headless): mpv sends it once playback actually begins.
            ffi::MPV_EVENT_PLAYBACK_RESTART => {
                s.restarts += 1;
                if s.first_frame_at.is_none() && s.loaded_at.is_some() {
                    s.first_frame_at = Some(Instant::now());
                }
            }
            ffi::MPV_EVENT_END_FILE => s.ended = end,
            ffi::MPV_EVENT_SHUTDOWN => {
                s.shutdown = true;
                seen.1.notify_all();
                return;
            }
            _ => {}
        }
        seen.1.notify_all();
    }
}
