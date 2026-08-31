//! Profiles, watch history, resume positions, watchlist, and settings.
//!
//! This is the data that cannot be rebuilt. The catalogue can be re-ingested from
//! public datasets; a user's viewing history exists nowhere else, which is why the
//! export in `crate::archive` covers exactly these tables.

use sqlx::Row;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackPosition {
    pub media_item_id: i64,
    pub position_seconds: i64,
    pub duration_seconds: Option<i64>,
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
}

pub struct ProfileRepository<'a> {
    db: &'a Db,
}

impl<'a> ProfileRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub async fn create(&self, name: &str, is_default: bool) -> Result<i64, DbError> {
        // A partial unique index makes two defaults impossible, so demote the
        // incumbent first rather than letting the insert fail.
        if is_default {
            sqlx::query("UPDATE profiles SET is_default = 0 WHERE is_default = 1")
                .execute(self.db.pool())
                .await?;
        }
        Ok(
            sqlx::query_scalar(
                "INSERT INTO profiles (name, is_default) VALUES (?, ?) RETURNING id",
            )
            .bind(name)
            .bind(i64::from(is_default))
            .fetch_one(self.db.pool())
            .await?,
        )
    }

    pub async fn default_profile(&self) -> Result<Option<Profile>, DbError> {
        let row = sqlx::query("SELECT id, name, is_default FROM profiles WHERE is_default = 1")
            .fetch_optional(self.db.pool())
            .await?;
        row.map(map_profile).transpose()
    }

    pub async fn all(&self) -> Result<Vec<Profile>, DbError> {
        let rows = sqlx::query("SELECT id, name, is_default FROM profiles ORDER BY id")
            .fetch_all(self.db.pool())
            .await?;
        rows.into_iter().map(map_profile).collect()
    }

    /// Append to the watch log. Never updates: the log is the record.
    pub async fn record_watch(
        &self,
        profile_id: i64,
        media_item_id: i64,
        watched_seconds: i64,
        completed: bool,
    ) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO watch_events (profile_id, media_item_id, watched_seconds, completed)
             VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .bind(watched_seconds)
        .bind(i64::from(completed))
        .fetch_one(self.db.pool())
        .await?)
    }

    pub async fn save_position(
        &self,
        profile_id: i64,
        position: &PlaybackPosition,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO playback_positions (
                 profile_id, media_item_id, position_seconds, duration_seconds,
                 audio_language, subtitle_language, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT (profile_id, media_item_id) DO UPDATE
                 SET position_seconds  = excluded.position_seconds,
                     duration_seconds  = excluded.duration_seconds,
                     audio_language    = excluded.audio_language,
                     subtitle_language = excluded.subtitle_language,
                     updated_at        = datetime('now')",
        )
        .bind(profile_id)
        .bind(position.media_item_id)
        .bind(position.position_seconds)
        .bind(position.duration_seconds)
        .bind(&position.audio_language)
        .bind(&position.subtitle_language)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn position(
        &self,
        profile_id: i64,
        media_item_id: i64,
    ) -> Result<Option<PlaybackPosition>, DbError> {
        let row = sqlx::query(
            "SELECT media_item_id, position_seconds, duration_seconds,
                    audio_language, subtitle_language
             FROM playback_positions WHERE profile_id = ? AND media_item_id = ?",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(|row| {
            Ok(PlaybackPosition {
                media_item_id: row.try_get("media_item_id")?,
                position_seconds: row.try_get("position_seconds")?,
                duration_seconds: row.try_get("duration_seconds")?,
                audio_language: row.try_get("audio_language")?,
                subtitle_language: row.try_get("subtitle_language")?,
            })
        })
        .transpose()
    }

    /// Continue-watching, most recent first.
    pub async fn continue_watching(
        &self,
        profile_id: i64,
        limit: i64,
    ) -> Result<Vec<PlaybackPosition>, DbError> {
        let rows = sqlx::query(
            "SELECT media_item_id, position_seconds, duration_seconds,
                    audio_language, subtitle_language
             FROM playback_positions
             WHERE profile_id = ?
               -- Finished items are not 'continue watching'. 95% rather than 100%
               -- because credits mean almost nobody reaches the true end.
               AND (duration_seconds IS NULL
                    OR position_seconds < (duration_seconds * 95 / 100))
             ORDER BY updated_at DESC
             LIMIT ?",
        )
        .bind(profile_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(PlaybackPosition {
                    media_item_id: row.try_get("media_item_id")?,
                    position_seconds: row.try_get("position_seconds")?,
                    duration_seconds: row.try_get("duration_seconds")?,
                    audio_language: row.try_get("audio_language")?,
                    subtitle_language: row.try_get("subtitle_language")?,
                })
            })
            .collect()
    }

    pub async fn add_to_watchlist(
        &self,
        profile_id: i64,
        media_item_id: i64,
        reason: Option<&str>,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO watchlist_items (profile_id, media_item_id, reason)
             VALUES (?, ?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .bind(reason)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn watchlist(&self, profile_id: i64) -> Result<Vec<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT media_item_id FROM watchlist_items
             WHERE profile_id = ? ORDER BY added_at DESC",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT (key) DO UPDATE
                 SET value = excluded.value, updated_at = datetime('now')",
        )
        .bind(key)
        .bind(value)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn setting(&self, key: &str) -> Result<Option<String>, DbError> {
        Ok(
            sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(self.db.pool())
                .await?,
        )
    }
}

fn map_profile(row: sqlx::sqlite::SqliteRow) -> Result<Profile, DbError> {
    Ok(Profile {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        is_default: row.try_get::<i64, _>("is_default")? != 0,
    })
}
