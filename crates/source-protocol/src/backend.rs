//! The one interface every source implements — SPEC.md §6.1.
//!
//! **This trait is defined in Phase 6 and must not be violated afterwards.** If
//! something does not fit, the trait changes deliberately, with an ADR.
//!
//! WHY `async_trait`. The resolver holds its backends as `Arc<dyn SourceBackend>` —
//! a list of different types behind one interface, chosen at runtime. A native
//! `async fn` in a trait returns an anonymous future type per implementation, which
//! makes the trait impossible to use as `dyn`. `#[async_trait]` rewrites each method
//! to return a boxed future, one allocation per call, which is nothing next to a
//! network round trip.

use async_trait::async_trait;

use crate::types::{BackendCapabilities, MediaQuery, PlayableStream, SourceCandidate};

#[async_trait]
pub trait SourceBackend: Send + Sync {
    /// Stable and unique: the registry stores settings and health under it.
    /// Lowercase ASCII, digits, `.`, `-` and `_`.
    fn id(&self) -> &str;

    fn capabilities(&self) -> BackendCapabilities;

    /// Every way this backend knows to watch the thing asked for. Empty is a normal
    /// answer, not an error.
    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError>;

    /// Turn one of this backend's own candidates into something playable.
    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError>;
}

/// Why a backend could not answer. Each arm is something the UI can say in plain
/// words; none of them is allowed to take down another backend's results.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    /// It could not be reached: offline, DNS, refused, a 5xx.
    #[error("unavailable: {0}")]
    Unavailable(String),
    /// It answered, but not in the protocol: bad JSON, wrong shape, too large.
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    /// It took longer than its budget. Set by the resolver, not by backends.
    #[error("timed out after {0} ms")]
    Timeout(u64),
    /// `resolve` was given a candidate this backend did not make, or that has gone.
    #[error("not found: {0}")]
    NotFound(String),
}
