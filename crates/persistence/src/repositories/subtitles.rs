//! Subtitles (SPEC.md Phase 10, migration 0018): the viewer's nudge per file and
//! subtitle, the download quota per provider per day, and what has been downloaded.

use sqlx::Row;

use crate::db::{Db, DbError};

/// A subtitle already downloaded for a media hash and language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSubtitle {
    pub provider: String,
    pub file_id: String,
    /// Relative to the data folder, so a copied app folder keeps its cache (§2.4).
    pub relative_path: String,
}

/// A provider's day so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuotaDay {
    pub used: i64,
    /// What the provider last said was left today, if it said.
    pub server_remaining: Option<i64>,
}

pub struct SubtitleRepository<'a> {
    db: &'a Db,
}

impl<'a> SubtitleRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// The nudge saved for this file and subtitle, in milliseconds.
    pub async fn nudge(&self, file_key: &str, subtitle_key: &str) -> Result<Option<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT offset_ms FROM subtitle_nudges WHERE file_key = ?1 AND subtitle_key = ?2",
        )
        .bind(file_key)
        .bind(subtitle_key)
        .fetch_optional(self.db.pool())
        .await?)
    }

    /// Save a nudge. Zero removes it: no row is the same as no nudge.
    pub async fn set_nudge(
        &self,
        file_key: &str,
        subtitle_key: &str,
        offset_ms: i64,
    ) -> Result<(), DbError> {
        if offset_ms == 0 {
            sqlx::query("DELETE FROM subtitle_nudges WHERE file_key = ?1 AND subtitle_key = ?2")
                .bind(file_key)
                .bind(subtitle_key)
                .execute(self.db.pool())
                .await?;
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO subtitle_nudges (file_key, subtitle_key, offset_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT (file_key, subtitle_key) DO UPDATE
               SET offset_ms = excluded.offset_ms,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        )
        .bind(file_key)
        .bind(subtitle_key)
        .bind(offset_ms)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn quota(&self, provider: &str, day: &str) -> Result<QuotaDay, DbError> {
        let row = sqlx::query(
            "SELECT used, server_remaining FROM subtitle_quota WHERE provider = ?1 AND day = ?2",
        )
        .bind(provider)
        .bind(day)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row
            .map(|r| QuotaDay {
                used: r.get(0),
                server_remaining: r.get(1),
            })
            .unwrap_or_default())
    }

    /// Take one download from today's budget, if any is left; `false` means none is,
    /// and nothing was taken. One statement, so two callers cannot both take the last.
    /// "Left" is the smaller of our own budget and what the provider last reported.
    pub async fn reserve(&self, provider: &str, day: &str, budget: i64) -> Result<bool, DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query(
            "INSERT INTO subtitle_quota (provider, day) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
        )
        .bind(provider)
        .bind(day)
        .execute(&mut *tx)
        .await?;
        let taken = sqlx::query(
            "UPDATE subtitle_quota SET used = used + 1
              WHERE provider = ?1 AND day = ?2 AND used < ?3
                AND (server_remaining IS NULL OR server_remaining > 0)",
        )
        .bind(provider)
        .bind(day)
        .bind(budget)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if taken {
            // The reservation is spent from what the provider allows, too, until it
            // reports its own number.
            sqlx::query(
                "UPDATE subtitle_quota SET server_remaining = server_remaining - 1
                  WHERE provider = ?1 AND day = ?2 AND server_remaining IS NOT NULL",
            )
            .bind(provider)
            .bind(day)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(taken)
    }

    /// Give back a reservation whose download did not happen.
    pub async fn release(&self, provider: &str, day: &str) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE subtitle_quota SET used = used - 1,
                    server_remaining = server_remaining + 1
              WHERE provider = ?1 AND day = ?2 AND used > 0",
        )
        .bind(provider)
        .bind(day)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// What the provider says is left today.
    pub async fn set_server_remaining(
        &self,
        provider: &str,
        day: &str,
        remaining: i64,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO subtitle_quota (provider, day, server_remaining) VALUES (?1, ?2, ?3)
             ON CONFLICT (provider, day) DO UPDATE SET server_remaining = excluded.server_remaining",
        )
        .bind(provider)
        .bind(day)
        .bind(remaining.max(0))
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn cached(
        &self,
        media_hash: &str,
        language: &str,
    ) -> Result<Option<CachedSubtitle>, DbError> {
        let row = sqlx::query(
            "SELECT provider, file_id, relative_path FROM subtitle_cache WHERE media_hash = ?1 AND language = ?2",
        )
        .bind(media_hash)
        .bind(language)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| CachedSubtitle {
            provider: r.get(0),
            file_id: r.get(1),
            relative_path: r.get(2),
        }))
    }

    pub async fn remember(
        &self,
        media_hash: &str,
        language: &str,
        entry: &CachedSubtitle,
    ) -> Result<(), DbError> {
        let p = std::path::Path::new(&entry.relative_path);
        if p.is_absolute() || p.has_root() || entry.relative_path.contains("..") {
            return Err(DbError::Invalid(format!(
                "{:?} must be relative to the data folder",
                entry.relative_path
            )));
        }
        sqlx::query(
            "INSERT INTO subtitle_cache (media_hash, language, provider, file_id, relative_path)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (media_hash, language) DO UPDATE
               SET provider = excluded.provider, file_id = excluded.file_id,
                   relative_path = excluded.relative_path,
                   fetched_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        )
        .bind(media_hash)
        .bind(language)
        .bind(&entry.provider)
        .bind(&entry.file_id)
        .bind(&entry.relative_path)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }
}
