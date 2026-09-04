//! Artwork: the cache between TMDB and the poster grid (`SPEC.md` Phase 4).
//!
//! Four requirements from §15 Phase 4: lazy fetch, a disk cache with a size budget,
//! WebP re-encoding, and blurhash placeholders.
//!
//! **Nothing here fetches anything by itself.** ADR-0027 makes the TMDB key
//! per-profile and optional, so a fetch is only ever made by a caller that has already
//! obtained `TmdbAccess::Configured` — and `TmdbAccess::Absent` is the state the app is
//! designed around, not an error to route past. This crate deliberately does not depend
//! on the persistence crate, so it cannot reach for a key even accidentally.

pub mod blurhash;
pub mod cache;
pub mod encode;

pub use blurhash::{encode as blurhash_encode, BlurhashError};
pub use cache::{ArtworkCache, CacheError, Stored};
pub use encode::{prepare, EncodeError, Prepared};
