//! The backends the app ships, and the one place they are named.
//!
//! **This file is the registration point for Phase 6 E1.** A new backend is a new
//! module here plus one line in [`all`]; the resolver, the registry, the IPC layer
//! and the UI all discover backends through this list and never name one.

use std::sync::Arc;

use sinephile_metadata_api::transport::ResponseFuture;
use sinephile_metadata_api::{Request, Transport, TransportError};
use source_protocol::SourceBackend;

pub mod archive;
pub mod direct;
pub mod local;

/// What backends may draw on when they are built. The app fills it once at startup
/// and again whenever the user changes something a backend reads.
pub struct Context {
    /// For backends that talk to a service the app trusts — the Internet Archive.
    pub http: Arc<dyn Transport>,
    /// The user's own links (migration 0016).
    pub direct_links: Vec<direct::DirectLink>,
}

impl Default for Context {
    /// Everything empty and no network: what the property tests build backends from.
    fn default() -> Self {
        Self {
            http: Arc::new(Unconnected),
            direct_links: Vec::new(),
        }
    }
}

/// A transport that refuses every request, for a context with no network.
struct Unconnected;

impl Transport for Unconnected {
    fn send(&self, _request: Request) -> ResponseFuture<'_> {
        Box::pin(async { Err(TransportError::Network("no network in this context".into())) })
    }
}

/// Every backend available right now, built-in and addon alike. Which of them are
/// asked, and in what order, is the registry's decision (`crate::registry`).
pub fn all(ctx: &Context) -> Vec<Arc<dyn SourceBackend>> {
    vec![
        Arc::new(local::LocalFileBackend),
        Arc::new(archive::InternetArchiveBackend::new(ctx.http.clone())),
        Arc::new(direct::DirectHttpBackend::new(ctx.direct_links.clone())),
    ]
}
