//! Re-exports of `crates/persistence` (ADR-0022).
//!
//! This module contains re-exports and NOTHING ELSE, and `tools/guard` fails the
//! build if that stops being true or if a SQL literal appears anywhere under
//! `src-tauri/`. `cargo test` cannot run inside this crate on Windows, so logic
//! placed here would be permanently untestable.

pub use sinephile_persistence::archive::{
    ArchiveError, Archiver, ImportSummary, ItemRef, ProfileArchive,
};
pub use sinephile_persistence::db::{Db, DbError, SqlitePool};
pub use sinephile_persistence::model::{
    EpisodeNumbering, IdSource, MediaItem, MediaKind, NewMediaItem, Title, TitleVariant,
};
pub use sinephile_persistence::paths::{
    backup_path, data_dir, database_path, DataLocation, PathError, DATA_DIR_ENV,
};
pub use sinephile_persistence::repositories::{
    EpisodeRepository, MediaRepository, ProfileRepository,
};
