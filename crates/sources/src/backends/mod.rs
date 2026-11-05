//! The backends the app ships, and the one place they are named.
//!
//! **This file is the registration point for Phase 6 E1.** A new backend is a new
//! module here plus one line in [`all`]; the resolver, the registry, the IPC layer
//! and the UI all discover backends through this list and never name one.

use std::sync::Arc;

use source_protocol::SourceBackend;

pub mod direct;
pub mod local;

/// What backends may draw on when they are built. The app fills it once at startup
/// and again whenever the user changes something a backend reads.
#[derive(Default)]
pub struct Context {
    /// The user's own links (migration 0016).
    pub direct_links: Vec<direct::DirectLink>,
}

/// Every backend available right now, built-in and addon alike. Which of them are
/// asked, and in what order, is the registry's decision (`crate::registry`).
pub fn all(ctx: &Context) -> Vec<Arc<dyn SourceBackend>> {
    vec![
        Arc::new(local::LocalFileBackend),
        Arc::new(direct::DirectHttpBackend::new(ctx.direct_links.clone())),
    ]
}
