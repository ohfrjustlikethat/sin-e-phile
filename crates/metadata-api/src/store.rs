//! The cache store: [`crate::cache`]'s policy applied to migration 0008's table.
//!
//! Split from the policy on purpose. The policy is pure arithmetic over durations
//! and is tested without a database; this is the part that needs one, and it is
//! deliberately thin so there is little here to be wrong.

use sinephile_persistence::{Db, DbError};

use crate::cache::{self, Freshness, Resource};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cache: {0}")]
    Db(#[from] DbError),
    #[error("cache: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// `(body, status, etag, last_modified, age_seconds)` as `http_cache` stores it.
type CacheRow = (String, i64, Option<String>, Option<String>, f64);

/// A response to store.
///
/// A struct rather than eight positional arguments: `put(url, source, resource,
/// status, body, etag, last_modified)` is a line where two `Option<&str>` sit
/// adjacent and swapping them compiles fine and is wrong forever.
#[derive(Debug, Clone)]
pub struct Store<'a> {
    pub url: &'a str,
    pub source: &'a str,
    pub resource: Resource,
    pub status: u16,
    pub body: &'a str,
    pub etag: Option<&'a str>,
    pub last_modified: Option<&'a str>,
}

impl<'a> Store<'a> {
    /// The common case: a 200 with no validators.
    pub fn ok(url: &'a str, source: &'a str, resource: Resource, body: &'a str) -> Self {
        Self {
            url,
            source,
            resource,
            status: 200,
            body,
            etag: None,
            last_modified: None,
        }
    }

    pub fn with_etag(mut self, etag: Option<&'a str>) -> Self {
        self.etag = etag;
        self
    }

    pub fn with_last_modified(mut self, last_modified: Option<&'a str>) -> Self {
        self.last_modified = last_modified;
        self
    }
}

/// A cached response, and how stale it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Cached {
    pub body: String,
    pub status: u16,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub freshness: Freshness,
}

impl Cached {
    /// Validators for a conditional request, so a refresh of unchanged content costs
    /// a 304 rather than a body.
    pub fn validators(&self) -> Option<(Option<&str>, Option<&str>)> {
        match (&self.etag, &self.last_modified) {
            (None, None) => None,
            (etag, modified) => Some((etag.as_deref(), modified.as_deref())),
        }
    }
}

pub struct CacheStore<'a> {
    db: &'a Db,
}

impl<'a> CacheStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Look up a response.
    ///
    /// Returns an entry only if the policy says it may be **served** — which depends
    /// on `offline`, because the same stale entry is a refresh candidate with a
    /// network and the answer without one. A stale entry that is not serveable is
    /// still returned when `include_stale` is set, so a caller can send its
    /// validators on a conditional request.
    pub async fn get(
        &self,
        url: &str,
        resource: Resource,
        offline: bool,
        include_stale: bool,
    ) -> Result<Option<Cached>, StoreError> {
        let key = cache::cache_key(url);

        // Age computed in SQL: julianday() differences are exact, and parsing the
        // stored timestamp in Rust would mean agreeing on a format with SQLite.
        let row: Option<CacheRow> = sqlx::query_as(
            "SELECT body, status, etag, last_modified,
                    (julianday('now') - julianday(fetched_at)) * 86400.0
             FROM http_cache WHERE url = ?",
        )
        .bind(&key)
        .fetch_optional(self.db.pool())
        .await?;

        let Some((body, status, etag, last_modified, age_seconds)) = row else {
            return Ok(None);
        };

        // A clock that has gone backwards would give a negative age, which would
        // otherwise read as "fetched in the future" and therefore fresh forever.
        let age = std::time::Duration::from_secs_f64(age_seconds.max(0.0));
        let freshness = cache::freshness(resource, age);

        if !cache::serveable(resource, age, offline) && !include_stale {
            return Ok(None);
        }
        if freshness == Freshness::Expired && !include_stale {
            return Ok(None);
        }

        Ok(Some(Cached {
            body,
            status: status as u16,
            etag,
            last_modified,
            freshness,
        }))
    }

    /// Store a response. Replaces any existing entry for the same key.
    pub async fn put(&self, entry: Store<'_>) -> Result<(), StoreError> {
        let key = cache::cache_key(entry.url);
        let ttl_seconds = entry.resource.ttl().as_secs() as i64;

        sqlx::query(
            "INSERT INTO http_cache
                 (url, source, resource, body, status, etag, last_modified,
                  fetched_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'),
                     datetime('now', '+' || ? || ' seconds'))
             ON CONFLICT (url) DO UPDATE SET
                 body = excluded.body,
                 status = excluded.status,
                 etag = excluded.etag,
                 last_modified = excluded.last_modified,
                 fetched_at = excluded.fetched_at,
                 expires_at = excluded.expires_at",
        )
        .bind(&key)
        .bind(entry.source)
        .bind(entry.resource.as_str())
        .bind(entry.body)
        .bind(i64::from(entry.status))
        .bind(entry.etag)
        .bind(entry.last_modified)
        .bind(ttl_seconds)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Mark an entry as revalidated, without rewriting its body.
    ///
    /// What a 304 means: the content is unchanged, so only the clock moves. Writing
    /// the body again would be identical work for an identical result.
    pub async fn touch(&self, url: &str, resource: Resource) -> Result<(), StoreError> {
        let key = cache::cache_key(url);
        sqlx::query(
            "UPDATE http_cache
             SET fetched_at = datetime('now'),
                 expires_at = datetime('now', '+' || ? || ' seconds')
             WHERE url = ?",
        )
        .bind(resource.ttl().as_secs() as i64)
        .bind(&key)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Delete entries past the point where they are useful even offline.
    ///
    /// Keyed on `max_age`, not `expires_at`: expiry means "refresh when you can",
    /// and deleting on it would throw away exactly the entries that make offline
    /// behaviour graceful.
    pub async fn purge(&self) -> Result<u64, StoreError> {
        let mut removed = 0;
        for resource in [
            Resource::Detail,
            Resource::Search,
            Resource::Images,
            Resource::Schedule,
            Resource::Ratings,
        ] {
            let seconds = resource.max_age().as_secs() as i64;
            let result = sqlx::query(
                "DELETE FROM http_cache
                 WHERE resource = ?
                   AND fetched_at < datetime('now', '-' || ? || ' seconds')",
            )
            .bind(resource.as_str())
            .bind(seconds)
            .execute(self.db.pool())
            .await?;
            removed += result.rows_affected();
        }
        Ok(removed)
    }

    pub async fn len(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM http_cache")
            .fetch_one(self.db.pool())
            .await?)
    }

    pub async fn is_empty(&self) -> Result<bool, StoreError> {
        Ok(self.len().await? == 0)
    }
}
