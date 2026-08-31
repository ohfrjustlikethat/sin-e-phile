//! sin-e-phile data layer — schema, migrations, and repositories.
//!
//! `SPEC.md` Phase 3. Everything that touches SQL lives in this crate; `src-tauri`
//! re-exports it and `tools/guard` fails the build if a `sqlx` call or a SQL
//! literal appears anywhere under `src-tauri/` (ADR-0022).
//!
//! The reason is not tidiness. A test binary that links Tauri cannot launch on
//! Windows at all, and `SPEC.md` Phase 3 requires integration tests for migrations
//! forward *and* backward. Putting the data layer here is what makes that possible.

pub mod archive;
pub mod db;
pub mod model;
pub mod paths;
pub mod repositories;

pub use archive::{ArchiveError, Archiver, ImportSummary, ProfileArchive};
pub use db::{Db, DbError, SqlitePool};
pub use model::{
    EpisodeNumbering, IdSource, MediaItem, MediaKind, NewMediaItem, Title, TitleVariant,
};
pub use paths::{data_dir, DataLocation, PathError, DATA_DIR_ENV};
