//! The player — SPEC.md Phase 8, ADR-0004 (libmpv), ADR-0020/0021 (chrome over video).
//!
//! [`Player`] wraps libmpv: open, pause, exact seeks, tracks, volume, speed, a state
//! snapshot, and the timings Phase 8 measures (load, first frame). [`dll_path`] finds the
//! pinned `libmpv-2.dll` that `tools/mpv/fetch.py` installs.

mod ffi;
#[cfg(windows)]
pub mod host;
pub mod player;
pub mod still;
pub mod thumbs;

#[cfg(windows)]
pub use host::{Cutout, Placement, VideoHost};
pub use player::{Chapter, Options, Player, PlayerError, State, SubtitleStyle, Track};

/// Where libmpv is: `SINEPHILE_LIBMPV` if set, else `lib/libmpv-2.dll` beside the
/// executable, else the repository's `lib/` (development and tests).
pub fn dll_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("SINEPHILE_LIBMPV") {
        return p.into();
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
    {
        for candidate in [
            dir.join("libmpv-2.dll"),
            dir.join("lib").join("libmpv-2.dll"),
        ] {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/libmpv-2.dll")
}
