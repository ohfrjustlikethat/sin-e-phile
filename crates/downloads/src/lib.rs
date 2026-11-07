//! Downloads (SPEC.md Phase 13): a queue that survives restarts, run by whichever backend
//! moves the bytes, filing each finished download into the library.
//!
//! - [`advise`]: stream or download, in one sentence, from the swarm, the disk, the hour and taste.
//! - [`queue`]: which downloads should run now (priority, schedule, the number at once).
//! - [`backend`]: what moves the bytes: librqbit in-process, or the user's qBittorrent.
//! - [`qbittorrent`]: the user's own qBittorrent, over its Web API, as a transport only.
//! - [`measure`]: free disk space, and this connection's speed by hour.
//! - [`manager`]: the plan applied, progress followed, the finished filed.

pub mod advise;
pub mod backend;
pub mod manager;
pub mod measure;
pub mod qbittorrent;
pub mod queue;

pub use backend::{Backend, BackendError, Progress, Rqbit};
pub use manager::{now_iso, Manager, ManagerError, Tick};
pub use queue::{plan, Step};
