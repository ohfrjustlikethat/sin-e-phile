//! The source backend registry: which backends exist, which are on, in what order,
//! and how each has been behaving (SPEC.md Phase 6, migration 0015).
//!
//! The resolver never reads this. The app reads it, builds the resolver from the
//! enabled rows in order, and writes each answer's outcome back as health.

use sqlx::Row;

use crate::db::{Db, DbError};

/// How much one new latency sample moves the average. 0.2 means a backend's last five
/// or so answers dominate, so a recovered backend stops reading as slow within a day
/// of normal use rather than a month.
const LATENCY_WEIGHT: f64 = 0.2;

#[derive(Debug, Clone, PartialEq)]
pub struct BackendRow {
    pub id: String,
    pub enabled: bool,
    pub position: i64,
    pub successes: i64,
    pub failures: i64,
    pub timeouts: i64,
    pub latency_ms: Option<f64>,
    pub last_error: Option<String>,
    pub last_outcome_at: Option<String>,
}

/// One answer from one backend, as health sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Sample {
    Answered { latency_ms: f64 },
    Failed { error: String },
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonRow {
    pub backend_id: String,
    pub manifest_url: String,
    pub manifest: String,
    pub installed_at: String,
}

pub struct SourceRepository<'a> {
    db: &'a Db,
}

impl<'a> SourceRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Make sure every backend the app knows about has a row. New ones go at the end,
    /// enabled; existing rows — and the user's choices in them — are untouched.
    pub async fn ensure(&self, ids: &[&str]) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        for id in ids {
            sqlx::query(
                "INSERT INTO source_backends (id, position)
                 VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM source_backends))
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Every registered backend, in the user's order.
    pub async fn all(&self) -> Result<Vec<BackendRow>, DbError> {
        let rows = sqlx::query(
            "SELECT id, enabled, position, successes, failures, timeouts, latency_ms,
                    last_error, last_outcome_at
             FROM source_backends ORDER BY position, id",
        )
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| BackendRow {
                id: r.get(0),
                enabled: r.get::<i64, _>(1) != 0,
                position: r.get(2),
                successes: r.get(3),
                failures: r.get(4),
                timeouts: r.get(5),
                latency_ms: r.get(6),
                last_error: r.get(7),
                last_outcome_at: r.get(8),
            })
            .collect())
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), DbError> {
        let done = sqlx::query("UPDATE source_backends SET enabled = ?2 WHERE id = ?1")
            .bind(id)
            .bind(i64::from(enabled))
            .execute(self.db.pool())
            .await?;
        if done.rows_affected() == 0 {
            return Err(DbError::Invalid(format!("there is no source called {id}")));
        }
        Ok(())
    }

    /// Put the backends in this order. Every registered id must appear exactly once,
    /// so a stale list from an old screen cannot silently drop one.
    pub async fn reorder(&self, ids: &[String]) -> Result<(), DbError> {
        let mut known: Vec<String> = self.all().await?.into_iter().map(|b| b.id).collect();
        let mut given = ids.to_vec();
        known.sort();
        given.sort();
        if known != given {
            return Err(DbError::Invalid(
                "the new order must list every source exactly once".into(),
            ));
        }
        let mut tx = self.db.pool().begin().await?;
        for (position, id) in ids.iter().enumerate() {
            sqlx::query("UPDATE source_backends SET position = ?2 WHERE id = ?1")
                .bind(id)
                .bind(position as i64)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Fold one answer into the backend's health. An unknown id is ignored: the
    /// backend may have been removed while its answer was in flight.
    pub async fn record(&self, id: &str, sample: &Sample) -> Result<(), DbError> {
        let query = match sample {
            Sample::Answered { latency_ms } => sqlx::query(
                "UPDATE source_backends SET successes = successes + 1,
                    latency_ms = CASE WHEN latency_ms IS NULL THEN ?2
                                      ELSE latency_ms + ?3 * (?2 - latency_ms) END,
                    last_error = NULL,
                    last_outcome_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?1",
            )
            .bind(id)
            .bind(*latency_ms)
            .bind(LATENCY_WEIGHT),
            Sample::Failed { error } => sqlx::query(
                "UPDATE source_backends SET failures = failures + 1, last_error = ?2,
                    last_outcome_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?1",
            )
            .bind(id)
            // A backend's error text can come from a remote service: bounded.
            .bind(error.chars().take(300).collect::<String>()),
            Sample::TimedOut => sqlx::query(
                "UPDATE source_backends SET timeouts = timeouts + 1,
                    last_error = 'timed out',
                    last_outcome_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?1",
            )
            .bind(id),
        };
        query.execute(self.db.pool()).await?;
        Ok(())
    }

    /// Install an addon: its registry row (at the end, enabled) and its manifest, in
    /// one transaction. Installing the same backend id again replaces the manifest —
    /// that is what "update" is — and keeps the user's position and switch.
    pub async fn install_addon(
        &self,
        backend_id: &str,
        manifest_url: &str,
        manifest_json: &str,
    ) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query(
            "INSERT INTO source_backends (id, position)
             VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM source_backends))
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(backend_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO addons (backend_id, manifest_url, manifest) VALUES (?1, ?2, ?3)
             ON CONFLICT (backend_id) DO UPDATE SET
                 manifest_url = excluded.manifest_url, manifest = excluded.manifest",
        )
        .bind(backend_id)
        .bind(manifest_url)
        .bind(manifest_json)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn addons(&self) -> Result<Vec<AddonRow>, DbError> {
        let rows = sqlx::query(
            "SELECT a.backend_id, a.manifest_url, a.manifest, a.installed_at
             FROM addons a JOIN source_backends b ON b.id = a.backend_id
             ORDER BY b.position",
        )
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| AddonRow {
                backend_id: r.get(0),
                manifest_url: r.get(1),
                manifest: r.get(2),
                installed_at: r.get(3),
            })
            .collect())
    }

    /// Uninstall: the registry row goes, and the manifest with it (ON DELETE CASCADE).
    /// Only addons can be removed; a built-in backend can be switched off, not deleted.
    pub async fn remove_addon(&self, backend_id: &str) -> Result<(), DbError> {
        let done = sqlx::query(
            "DELETE FROM source_backends
             WHERE id = ?1 AND id IN (SELECT backend_id FROM addons)",
        )
        .bind(backend_id)
        .execute(self.db.pool())
        .await?;
        if done.rows_affected() == 0 {
            return Err(DbError::Invalid(format!(
                "there is no addon called {backend_id}"
            )));
        }
        Ok(())
    }
}

/// A link the user added, for the direct HTTP backend (migration 0016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLinkRow {
    pub id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub url: String,
}

pub struct DirectLinkRepository<'a> {
    db: &'a Db,
}

impl<'a> DirectLinkRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Add links in one transaction. A URL already present has its title and year
    /// replaced, so re-importing a corrected playlist corrects it. Returns how many
    /// rows were written.
    pub async fn add(&self, links: &[(String, Option<i64>, String)]) -> Result<usize, DbError> {
        let mut tx = self.db.pool().begin().await?;
        for (title, year, url) in links {
            sqlx::query(
                "INSERT INTO direct_links (title, year, url) VALUES (?1, ?2, ?3)
                 ON CONFLICT (url) DO UPDATE SET title = excluded.title, year = excluded.year",
            )
            .bind(title)
            .bind(year)
            .bind(url)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(links.len())
    }

    pub async fn all(&self) -> Result<Vec<DirectLinkRow>, DbError> {
        let rows = sqlx::query("SELECT id, title, year, url FROM direct_links ORDER BY title, id")
            .fetch_all(self.db.pool())
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| DirectLinkRow {
                id: r.get(0),
                title: r.get(1),
                year: r.get(2),
                url: r.get(3),
            })
            .collect())
    }

    pub async fn remove(&self, id: i64) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM direct_links WHERE id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(done.rows_affected() == 1)
    }
}
