//! The torrent engine and its local streaming server — SPEC.md Phase 7.
//!
//! - [`engine`]: librqbit in-process, configured for portability, a seeding policy, fast
//!   failure on a dead swarm, and interface binding.
//! - [`select`]: which file in a torrent is the one to play.
//! - [`server`]: a loopback HTTP server that makes an in-progress torrent look like an
//!   ordinary seekable file to the player.
//! - [`play`]: from a resolved torrent to an address the player opens.
//! - [`settings`]: what the user can configure, and the adapters to bind to.

pub mod engine;
pub mod play;
pub mod select;
pub mod server;
pub mod settings;

pub use engine::{
    AddOptions, Engine, EngineConfig, SeedingPolicy, Source, State, Status, TorrentError,
};
pub use select::{TorrentFile, Wanted};
pub use server::{Source as StreamSource, StreamServer};
