//! The Tauri IPC surface — SPEC.md §7: **thin, no logic here.**
//!
//! Every command delegates. If a command starts making decisions, that decision
//! belongs in a module the command calls, so it can be tested without an app
//! handle and reused from somewhere that is not IPC.

pub mod system;
