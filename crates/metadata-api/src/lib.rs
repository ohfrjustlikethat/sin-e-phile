//! Live metadata API clients (`SPEC.md` Phase 4).
//!
//! Four services — TMDB, AniList, Jikan, Fanart.tv — with one shared rate limiter,
//! one backoff policy, and one persistent response cache between them.
//!
//! **Every one of them is optional.** ADR-0013 makes the offline IMDb + MovieLens
//! catalogue the base rather than an optimisation, and ADR-0027 means no key ever
//! ships. So the correct behaviour when a service is unavailable, unconfigured, or
//! rate-limited is to return "no enrichment", never to fail the operation that asked.

pub mod backoff;
pub mod cache;
pub mod limiter;
pub mod store;

pub use backoff::{classify, Backoff, Retryable};
pub use cache::{cache_key, freshness, serveable, Freshness, Resource};
pub use limiter::{Limit, RateLimiter};
pub use store::{CacheStore, Cached, Store, StoreError};
