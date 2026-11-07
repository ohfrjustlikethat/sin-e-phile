//! Downloads (SPEC.md Phase 13): a queue that survives restarts, run by whichever backend
//! moves the bytes, filing each finished download into the library.
//!
//! - [`queue`]: which downloads should run now (priority, schedule, the number at once).
//! - [`backend`]: what moves the bytes: librqbit in-process, or the user's qBittorrent.
//! - [`qbittorrent`]: the user's own qBittorrent, over its Web API, as a transport only.
//! - [`manager`]: the plan applied, progress followed, the finished filed.

pub mod backend;
pub mod manager;
pub mod qbittorrent;
pub mod queue;

pub use backend::{Backend, BackendError, Progress, Rqbit};
pub use manager::{now_iso, Manager, ManagerError, Tick};
pub use queue::{plan, Step};
