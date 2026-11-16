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
pub mod secrets;

/// At or above this confidence a file's match is accepted without asking; below it the
/// match is the best guess the review queue shows, and the library, Home and a title's
/// Play never use it (Phase 16: the author's episodes, misread as films at 0.02, filled
/// Home with "Entrapment" and "The Fugitive"). Set from `eval identify` (Phase 12); the
/// library's `ACCEPT_AT` is this.
pub const MATCH_ACCEPTED_AT: f64 = 0.75;

pub use archive::{ArchiveError, Archiver, ImportSummary, ProfileArchive};
pub use db::{Compacted, Db, DbError, SqlitePool};
pub use model::{
    EpisodeNumbering, IdSource, MediaItem, MediaKind, NewMediaItem, Title, TitleVariant,
};
pub use paths::{data_dir, DataLocation, PathError, DATA_DIR_ENV};
