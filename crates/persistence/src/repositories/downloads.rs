//! The download queue (SPEC.md Phase 13, migration 0023): what is waiting, running,
//! finished or failed, in what order, and why.

use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Active,
    /// Paused by the user: the queue never starts it by itself.
    Paused,
    Done,
    Failed,
    Cancelled,
}

impl DownloadState {
    pub fn as_str(self) -> &'static str {
        match self {
            DownloadState::Queued => "queued",
            DownloadState::Active => "active",
            DownloadState::Paused => "paused",
            DownloadState::Done => "done",
            DownloadState::Failed => "failed",
            DownloadState::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "active" => DownloadState::Active,
            "paused" => DownloadState::Paused,
            "done" => DownloadState::Done,
            "failed" => DownloadState::Failed,
            "cancelled" => DownloadState::Cancelled,
            _ => DownloadState::Queued,
        }
    }

    /// Finished one way or another: nothing more will happen to it.
    pub fn is_over(self) -> bool {
        matches!(
            self,
            DownloadState::Done | DownloadState::Failed | DownloadState::Cancelled
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Download {
    pub id: i64,
    pub media_item_id: Option<i64>,
    pub title: String,
    pub source: String,
    pub torrent: Option<Vec<u8>>,
    /// "librqbit" | "qbittorrent"
    pub backend: String,
    pub backend_id: Option<String>,
    pub priority: i64,
    pub state: DownloadState,
    pub total_bytes: Option<i64>,
    pub done_bytes: i64,
    pub limit_kib: Option<i64>,
    pub not_before: Option<String>,
    pub reason: Option<String>,
    pub error: Option<String>,
    pub file_path: Option<String>,
    pub added_at: String,
    pub finished_at: Option<String>,
}

/// A torrent's bytes would fill a screen; the debug view says how many there are.
impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("state", &self.state)
            .field("backend", &self.backend)
            .field("backend_id", &self.backend_id)
            .field("priority", &self.priority)
            .field("done_bytes", &self.done_bytes)
            .field("total_bytes", &self.total_bytes)
            .field("torrent_bytes", &self.torrent.as_ref().map(Vec::len))
            .field("not_before", &self.not_before)
            .field("error", &self.error)
            .field("file_path", &self.file_path)
            .finish_non_exhaustive()
    }
}

/// A download as it is asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewDownload<'a> {
    pub media_item_id: Option<i64>,
    pub title: &'a str,
    pub source: &'a str,
    pub torrent: Option<&'a [u8]>,
    pub backend: &'a str,
    pub priority: i64,
    pub limit_kib: Option<i64>,
    pub not_before: Option<&'a str>,
    pub reason: Option<&'a str>,
}

const COLUMNS: &str = "id, media_item_id, title, source, torrent, backend, backend_id, priority, state,
    total_bytes, done_bytes, limit_kib, not_before, reason, error, file_path, added_at, finished_at";

fn row(r: &SqliteRow) -> Result<Download, sqlx::Error> {
    Ok(Download {
        id: r.try_get("id")?,
        media_item_id: r.try_get("media_item_id")?,
        title: r.try_get("title")?,
        source: r.try_get("source")?,
        torrent: r.try_get("torrent")?,
        backend: r.try_get("backend")?,
        backend_id: r.try_get("backend_id")?,
        priority: r.try_get("priority")?,
        state: DownloadState::parse(&r.try_get::<String, _>("state")?),
        total_bytes: r.try_get("total_bytes")?,
        done_bytes: r.try_get("done_bytes")?,
        limit_kib: r.try_get("limit_kib")?,
        not_before: r.try_get("not_before")?,
        reason: r.try_get("reason")?,
        error: r.try_get("error")?,
        file_path: r.try_get("file_path")?,
        added_at: r.try_get("added_at")?,
        finished_at: r.try_get("finished_at")?,
    })
}

pub struct DownloadRepository<'a> {
    db: &'a Db,
}

impl<'a> DownloadRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub async fn add(&self, d: &NewDownload<'_>) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO downloads
                 (media_item_id, title, source, torrent, backend, priority, limit_kib, not_before, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) RETURNING id",
        )
        .bind(d.media_item_id)
        .bind(d.title)
        .bind(d.source)
        .bind(d.torrent)
        .bind(d.backend)
        .bind(d.priority)
        .bind(d.limit_kib)
        .bind(d.not_before)
        .bind(d.reason)
        .fetch_one(self.db.pool())
        .await?)
    }

    pub async fn get(&self, id: i64) -> Result<Option<Download>, DbError> {
        let r = sqlx::query(&format!("SELECT {COLUMNS} FROM downloads WHERE id = ?1"))
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(r.as_ref().map(row).transpose()?)
    }

    /// Everything, in queue order: unfinished first by priority then age, then the
    /// finished, newest first.
    pub async fn all(&self) -> Result<Vec<Download>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM downloads
             ORDER BY state IN ('done', 'failed', 'cancelled'),
                      CASE WHEN state IN ('done', 'failed', 'cancelled') THEN -id ELSE 0 END,
                      priority DESC, id"
        ))
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.iter().map(row).collect::<Result<_, _>>()?)
    }

    /// Not finished: what the manager looks after.
    pub async fn open(&self) -> Result<Vec<Download>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM downloads
             WHERE state IN ('queued', 'active', 'paused') ORDER BY priority DESC, id"
        ))
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.iter().map(row).collect::<Result<_, _>>()?)
    }

    pub async fn set_state(&self, id: i64, state: DownloadState) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE downloads SET state = ?2,
                 finished_at = CASE WHEN ?2 IN ('done', 'failed', 'cancelled')
                                    THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now') END,
                 error = CASE WHEN ?2 = 'failed' THEN error END
             WHERE id = ?1",
        )
        .bind(id)
        .bind(state.as_str())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn set_backend_id(
        &self,
        id: i64,
        backend: &str,
        backend_id: &str,
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE downloads SET backend = ?2, backend_id = ?3 WHERE id = ?1")
            .bind(id)
            .bind(backend)
            .bind(backend_id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    pub async fn set_progress(
        &self,
        id: i64,
        done: i64,
        total: Option<i64>,
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE downloads SET done_bytes = ?2, total_bytes = COALESCE(?3, total_bytes) WHERE id = ?1")
            .bind(id)
            .bind(done)
            .bind(total)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    pub async fn set_priority(&self, id: i64, priority: i64) -> Result<(), DbError> {
        sqlx::query("UPDATE downloads SET priority = ?2 WHERE id = ?1")
            .bind(id)
            .bind(priority)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    pub async fn set_limit(&self, id: i64, limit_kib: Option<i64>) -> Result<(), DbError> {
        sqlx::query("UPDATE downloads SET limit_kib = ?2 WHERE id = ?1")
            .bind(id)
            .bind(limit_kib.filter(|k| *k > 0))
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    pub async fn set_not_before(&self, id: i64, not_before: Option<&str>) -> Result<(), DbError> {
        sqlx::query("UPDATE downloads SET not_before = ?2 WHERE id = ?1")
            .bind(id)
            .bind(not_before)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    pub async fn fail(&self, id: i64, error: &str) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE downloads SET state = 'failed', error = ?2,
                 finished_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
        )
        .bind(id)
        .bind(error)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Done, with the file it produced.
    pub async fn finish(&self, id: i64, file_path: &str) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE downloads SET state = 'done', file_path = ?2, error = NULL,
                 done_bytes = COALESCE(total_bytes, done_bytes),
                 finished_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
        )
        .bind(id)
        .bind(file_path)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Forget a finished download's row (the file is the library's business).
    pub async fn remove(&self, id: i64) -> Result<(), DbError> {
        sqlx::query("DELETE FROM downloads WHERE id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}
