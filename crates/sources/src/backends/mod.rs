//! The backends the app ships, and the one place they are named.
//!
//! **This file is the registration point for Phase 6 E1.** A new backend is a new
//! module here plus one line in [`all`]; the resolver, the registry, the IPC layer
//! and the UI all discover backends through this list and never name one.
