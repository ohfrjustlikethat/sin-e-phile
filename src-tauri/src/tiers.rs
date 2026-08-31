//! Re-export of the standalone tier crate.
//!
//! The logic lives in `crates/tiers` so it can be unit-tested: a test binary that
//! links Tauri cannot launch on Windows without a manifest `cargo test` does not
//! supply (ADR-0022). This keeps `crate::tiers::…` working everywhere else.

pub use sinephile_tiers::*;
