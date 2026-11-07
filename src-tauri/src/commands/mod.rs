//! The Tauri IPC surface — SPEC.md §7: **thin, no logic here.**
//!
//! Every command delegates. If a command starts making decisions, that decision
//! belongs in a module the command calls, so it can be tested without an app
//! handle and reused from somewhere that is not IPC.

pub mod assets;
pub mod downloads;
pub mod home;
pub mod imports;
pub mod library;
pub mod overlay;
pub mod playback;
pub mod player;
pub mod profiles;
pub mod search;
pub mod series;
pub mod sources;
pub mod subtitles;
pub mod system;
pub mod taste;
pub mod thumbs;
pub mod torrent;
