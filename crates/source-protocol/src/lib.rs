//! The sin-e-phile source protocol — SPEC.md §6.1 and Phase 6.
//!
//! Everything a third party needs to write a source backend, and nothing else:
//!
//! - [`SourceBackend`], the one trait every source implements.
//! - The values that cross it ([`MediaQuery`], [`SourceCandidate`], [`PlayableStream`],
//!   [`BackendCapabilities`]), which are also the addon wire format.
//! - The declarative addon [`Manifest`] and the [`Catalogue`] that lists manifests,
//!   each with a validator that treats its input as hostile.
//!
//! The specification is `docs/specs/addon-protocol.md`; the JSON Schema is
//! `docs/schemas/addon-manifest.schema.json`.
//!
//! **This crate cannot reach the network.** It has no HTTP client, by design: it
//! parses what the app fetched, and the app decides what to fetch.

pub mod backend;
pub mod catalogue;
pub mod manifest;
pub mod types;

pub use backend::{SourceBackend, SourceError};
pub use catalogue::{Catalogue, CatalogueEntry};
pub use manifest::{parse_manifest_url, Endpoints, Manifest, ManifestError};
pub use types::{
    BackendCapabilities, ExternalIds, Health, MediaKind, MediaQuery, PlayableStream, Provenance,
    Quality, Resolution, SourceCandidate, Transport, VideoCodec,
};

/// The protocol version this build speaks. A manifest declaring any other is refused
/// with a message that says so, rather than half-understood.
pub const PROTOCOL_VERSION: u32 = 1;

/// Re-exported so a backend author writes `#[source_protocol::async_trait]` without
/// adding the dependency themselves at a mismatched version.
pub use async_trait::async_trait;
