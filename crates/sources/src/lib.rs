//! Sources: the resolver, and the backends the app ships — SPEC.md §6.1, Phase 6.
//!
//! The trait and its types are in `source-protocol`, which third parties depend on.
//! This crate is the app's half: it asks backends, merges what they say, and holds
//! the reference implementations.
//!
//! **Adding a backend** (Phase 6 E1) is one file implementing `SourceBackend` and one
//! line in [`backends::all`]. Nothing else in the app names a backend.

pub mod addons;
pub mod backends;
pub mod links;
pub mod merge;
pub mod overview;
pub mod registry;
pub mod resolver;

pub use resolver::{Batch, Entry, Found, Outcome, Resolver};
