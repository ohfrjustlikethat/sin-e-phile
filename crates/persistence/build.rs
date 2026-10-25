//! Rebuild when a migration is added, changed, or removed.
//!
//! `sqlx::migrate!` embeds the migration directory at COMPILE time, but adding a
//! new file does not reliably invalidate cargo's cache on its own. The symptom is
//! brutal and silent: the code is new, the binary carries the OLD migration set, and
//! the database is missing a column that the source plainly says exists.
//!
//! That happened while adding migration 0006 — every loader test failed with
//! "table media_items has no column named in_core" against a source tree where the
//! column was clearly declared. `crates/persistence/tests/migrations.rs` asserts the
//! embedded count matches the files on disk, so a stale embed now fails loudly; this
//! makes it not happen in the first place.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
