//! The database handle, its PRAGMAs, and migrations.

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Sqlite};

use crate::paths::{self, PathError};

pub type SqlitePool = Pool<Sqlite>;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("could not write the pre-migration backup to {0}: {1}")]
    Backup(PathBuf, #[source] std::io::Error),
    /// The caller asked for something the repository refuses to store — a malformed
    /// API key, a secret read through the wrong accessor. Distinct from `Sqlx` on
    /// purpose: this is a message a user should see, not a database fault.
    #[error("{0}")]
    Invalid(String),
}

/// An open database.
///
/// Cloning is cheap — `SqlitePool` is an `Arc` internally — so repositories take
/// `&Db` and the application holds exactly one.
#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
    path: PathBuf,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The pool has no useful Debug and the path is the identifying fact.
        f.debug_struct("Db").field("path", &self.path).finish()
    }
}

/// Migrations are compiled into the binary rather than read from disk at runtime.
///
/// A portable app is a folder the user can move, rename, and copy. If migrations
/// were loose `.sql` files beside the executable, a half-copied folder would be a
/// database the app cannot open and cannot repair. Embedding them means the binary
/// and the schema it expects travel together, always.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

impl Db {
    /// Open (creating if absent), apply PRAGMAs, and migrate to the latest version.
    pub async fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbError::Backup(parent.to_path_buf(), e))?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            // WAL: readers do not block the writer and the writer does not block
            // readers. This app scans the library and ingests metadata on
            // background tasks while the UI queries continuously, so the default
            // rollback journal — which takes a whole-database lock for every
            // write — would make the UI stutter whenever a scan was running.
            .journal_mode(SqliteJournalMode::Wal)
            // NORMAL is the correct pairing with WAL: it fsyncs at checkpoints
            // rather than on every commit. The window it opens is losing the last
            // few transactions on power loss, never a corrupt database. For a
            // media catalogue that is the right trade; FULL costs an order of
            // magnitude on bulk insert for durability nobody here needs.
            .synchronous(SqliteSynchronous::Normal)
            // Without this, concurrent writers fail instantly with SQLITE_BUSY
            // instead of waiting their turn.
            .busy_timeout(std::time::Duration::from_secs(5))
            // SQLite does NOT enforce foreign keys unless asked, per connection.
            // Every schema constraint below is decorative without it.
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            // SQLite serialises writes regardless of pool size, but readers do run
            // concurrently under WAL, so a small pool is genuinely useful.
            .max_connections(8)
            .connect_with(options)
            .await?;

        let db = Self {
            pool,
            path: path.to_path_buf(),
        };
        db.migrate().await?;
        Ok(db)
    }

    /// Open the database for the given data directory, creating it if absent.
    pub async fn open_in(data_dir: &Path) -> Result<Self, DbError> {
        // Create BEFORE probing. The writability probe writes a file into the
        // directory, so running it first on a first launch — where `data/` does
        // not exist yet — reported "not writable" for a directory that was merely
        // absent. That is the normal case on a fresh portable install.
        std::fs::create_dir_all(data_dir)
            .map_err(|e| DbError::Path(PathError::Create(data_dir.to_path_buf(), e)))?;
        paths::assert_writable(data_dir)?;
        Self::open(&paths::database_path(data_dir)).await
    }

    /// An in-memory database, migrated. For tests.
    pub async fn in_memory() -> Result<Self, DbError> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        // One connection: every `:memory:` connection is a *separate* database, so
        // a multi-connection pool would hand out empty, unmigrated databases at
        // random. This is the single most confusing thing about SQLite in tests.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let db = Self {
            pool,
            path: PathBuf::from(":memory:"),
        };
        db.migrate().await?;
        Ok(db)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Apply every pending migration.
    async fn migrate(&self) -> Result<(), DbError> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    /// The highest migration this binary carries.
    ///
    /// Tests assert against this rather than a hard-coded number: migration 0005
    /// broke three tests that said `Some(4)`, which is a maintenance tax on every
    /// future migration for no benefit.
    pub fn latest_schema_version() -> i64 {
        MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
    }

    /// The highest applied migration version, or `None` on a fresh database.
    pub async fn schema_version(&self) -> Result<Option<i64>, DbError> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations WHERE success = TRUE")
                .fetch_optional(&self.pool)
                .await
                .unwrap_or(None);
        Ok(row.map(|(v,)| v))
    }

    /// Roll back to `target`, applying each migration's `down` script in reverse.
    ///
    /// `SPEC.md` Phase 3 requires backward migrations to be *tested*, not merely
    /// present, which is why this exists as a real API rather than a developer
    /// gesture: E1 runs the whole ladder down against a populated database.
    pub async fn migrate_down_to(&self, target: i64) -> Result<(), DbError> {
        MIGRATOR.undo(&self.pool, target).await?;
        Ok(())
    }

    /// Copy the database aside before a migration that will change it.
    ///
    /// Deliberately a plain file copy of a checkpointed database rather than a
    /// SQLite backup API call: the resulting file must be openable by any SQLite,
    /// including the user double-clicking it into a viewer, because the point of a
    /// backup is that it is useful when the app itself will not start.
    pub async fn backup_to(&self, destination: &Path) -> Result<(), DbError> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DbError::Backup(parent.to_path_buf(), e))?;
        }
        // TRUNCATE folds the WAL back into the main file, so the copy is complete.
        // Without it the backup can be missing every write since the last
        // checkpoint — which is exactly the writes a migration is about to touch.
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await?;
        std::fs::copy(&self.path, destination)
            .map_err(|e| DbError::Backup(destination.to_path_buf(), e))?;
        Ok(())
    }

    /// Migrate, taking a backup first if the schema is actually going to change.
    ///
    /// `SPEC.md` Phase 3: "backup-on-migrate". Nothing is copied when there is
    /// nothing to do, so ordinary startup does not duplicate the database on disk
    /// every launch.
    pub async fn migrate_with_backup(&self, data_dir: &Path) -> Result<Option<PathBuf>, DbError> {
        let before = self.schema_version().await?;
        if !self.has_pending().await? {
            return Ok(None);
        }
        let backup = before.map(|v| paths::backup_path(data_dir, v));
        if let Some(destination) = &backup {
            self.backup_to(destination).await?;
        }
        self.migrate().await?;
        Ok(backup)
    }

    async fn has_pending(&self) -> Result<bool, DbError> {
        let applied = self.schema_version().await?.unwrap_or(-1);
        Ok(MIGRATOR.iter().any(|m| m.version > applied))
    }
}
