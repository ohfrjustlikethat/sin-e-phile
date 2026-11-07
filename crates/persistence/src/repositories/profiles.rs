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
    /// A design-system avatar colour name ("oxblood", "slate"…); `None` for the first.
    pub avatar_colour: Option<String>,
    /// Shows only what is positively known to suit children (Phase 14).
    pub is_kids: bool,
    /// The highest certification allowed when the profile has a TMDB key.
    pub kids_limit: Option<String>,
    pub last_used_at: Option<String>,
    /// A PIN is set (it lives wrapped in `profile_settings`, never here).
    pub has_pin: bool,
}

/// One viewing from the log, for the taste model (Phase 15).
#[derive(Debug, Clone, PartialEq)]
pub struct Viewing {
    pub media_item_id: i64,
    /// "YYYY-MM-DD HH:MM:SS" (UTC) or ISO, as recorded.
    pub started_at: String,
    pub watched_seconds: i64,
    pub completed: bool,
    /// Where it stopped, when known: the abandon point.
    pub stopped_at_seconds: Option<i64>,
}

/// What a new profile is made with.
#[derive(Debug, Clone, Default)]
pub struct NewProfile<'a> {
    pub name: &'a str,
    pub avatar_colour: Option<&'a str>,
    pub is_kids: bool,
    pub kids_limit: Option<&'a str>,
}

const PROFILE_COLUMNS: &str = "id, name, is_default, avatar_colour, is_kids, kids_limit, last_used_at,
    EXISTS (SELECT 1 FROM profile_settings s WHERE s.profile_id = profiles.id AND s.key = 'pin') AS has_pin";

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
        let row = sqlx::query(&format!(
            "SELECT {PROFILE_COLUMNS} FROM profiles WHERE is_default = 1"
        ))
        .fetch_optional(self.db.pool())
        .await?;
        row.map(map_profile).transpose()
    }

    /// A profile with its avatar and kids settings; never the default (that stays with
    /// the first profile until the user says otherwise).
    pub async fn create_with(&self, p: &NewProfile<'_>) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO profiles (name, is_default, avatar_colour, is_kids, kids_limit)
             VALUES (?, 0, ?, ?, ?) RETURNING id",
        )
        .bind(p.name.trim())
        .bind(p.avatar_colour)
        .bind(i64::from(p.is_kids))
        .bind(p.kids_limit)
        .fetch_one(self.db.pool())
        .await?)
    }

    pub async fn get(&self, id: i64) -> Result<Option<Profile>, DbError> {
        let row = sqlx::query(&format!(
            "SELECT {PROFILE_COLUMNS} FROM profiles WHERE id = ?"
        ))
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        row.map(map_profile).transpose()
    }

    /// Name, avatar and kids settings in one write.
    pub async fn update(&self, id: i64, p: &NewProfile<'_>) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE profiles SET name = ?2, avatar_colour = ?3, is_kids = ?4, kids_limit = ?5
             WHERE id = ?1",
        )
        .bind(id)
        .bind(p.name.trim())
        .bind(p.avatar_colour)
        .bind(i64::from(p.is_kids))
        .bind(p.kids_limit)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// The one opened on launch when nobody is asked.
    pub async fn set_default(&self, id: i64) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("UPDATE profiles SET is_default = 0 WHERE is_default = 1 AND id <> ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE profiles SET is_default = 1 WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Delete a profile and everything that is only its (the tables cascade). The last
    /// profile is never deleted; deleting the default hands it to the most recently used.
    /// Returns whether anything was deleted.
    pub async fn delete(&self, id: i64) -> Result<bool, DbError> {
        let mut tx = self.db.pool().begin().await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profiles")
            .fetch_one(&mut *tx)
            .await?;
        let was_default: Option<i64> =
            sqlx::query_scalar("SELECT is_default FROM profiles WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(was_default) = was_default else {
            return Ok(false);
        };
        if count <= 1 {
            return Ok(false);
        }
        sqlx::query("DELETE FROM profiles WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if was_default == 1 {
            sqlx::query(
                "UPDATE profiles SET is_default = 1 WHERE id = (
                     SELECT id FROM profiles
                     ORDER BY last_used_at IS NULL, last_used_at DESC, id LIMIT 1)",
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    /// Opened just now: the picker lists the most recent first.
    pub async fn touch(&self, id: i64) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE profiles SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// The default profile's id, creating one if there is none. Profiles proper arrive
    /// in Phase 14; until then everything personal — resume points first — belongs to
    /// this one, and Phase 14's first real profile can adopt it.
    pub async fn ensure_default(&self) -> Result<i64, DbError> {
        if let Some(p) = self.default_profile().await? {
            return Ok(p.id);
        }
        self.create("You", true).await
    }

    pub async fn all(&self) -> Result<Vec<Profile>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {PROFILE_COLUMNS} FROM profiles
             ORDER BY last_used_at IS NULL, last_used_at DESC, id"
        ))
        .fetch_all(self.db.pool())
        .await?;
        rows.into_iter().map(map_profile).collect()
    }

    /// Append to the watch log. Never updates: the log is the record.
    /// The profile's 1–5 rating of a title; rating again replaces it, `None` clears it.
    pub async fn rate(
        &self,
        profile_id: i64,
        media_item_id: i64,
        stars: Option<i64>,
    ) -> Result<(), DbError> {
        match stars {
            Some(stars) if (1..=5).contains(&stars) => {
                sqlx::query(
                    "INSERT INTO ratings (profile_id, media_item_id, stars) VALUES (?1, ?2, ?3)
                     ON CONFLICT (profile_id, media_item_id) DO UPDATE
                       SET stars = excluded.stars, rated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                )
                .bind(profile_id)
                .bind(media_item_id)
                .bind(stars)
                .execute(self.db.pool())
                .await?;
            }
            Some(stars) => return Err(DbError::Invalid(format!("{stars} is not 1 to 5 stars"))),
            None => {
                sqlx::query("DELETE FROM ratings WHERE profile_id = ?1 AND media_item_id = ?2")
                    .bind(profile_id)
                    .bind(media_item_id)
                    .execute(self.db.pool())
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn rating(
        &self,
        profile_id: i64,
        media_item_id: i64,
    ) -> Result<Option<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT stars FROM ratings WHERE profile_id = ?1 AND media_item_id = ?2",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .fetch_optional(self.db.pool())
        .await?)
    }

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

    /// A viewing as the player saw it end (Phase 15): seconds actually played, whether it
    /// was completed, and where it stopped.
    pub async fn record_viewing(
        &self,
        profile_id: i64,
        media_item_id: i64,
        watched_seconds: i64,
        completed: bool,
        stopped_at_seconds: i64,
    ) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO watch_events
                 (profile_id, media_item_id, watched_seconds, completed, stopped_at_seconds, ended_at)
             VALUES (?, ?, ?, ?, ?, datetime('now')) RETURNING id",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .bind(watched_seconds)
        .bind(i64::from(completed))
        .bind(stopped_at_seconds)
        .fetch_one(self.db.pool())
        .await?)
    }

    /// A cheap fingerprint of everything the taste model reads for a profile: it changes
    /// whenever a viewing, a rating or the watchlist changes, so a cached model can be
    /// trusted while it does not (Phase 15).
    pub async fn signal_fingerprint(&self, profile_id: i64) -> Result<String, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT
                 (SELECT COUNT(*) || '/' || COALESCE(MAX(id), 0) FROM watch_events WHERE profile_id = ?1)
                 || ':' ||
                 (SELECT COUNT(*) || '/' || COALESCE(MAX(rated_at), '') || '/' || COALESCE(SUM(stars * media_item_id), 0)
                    FROM ratings WHERE profile_id = ?1)
                 || ':' ||
                 (SELECT COUNT(*) || '/' || COALESCE(SUM(media_item_id), 0) FROM watchlist_items WHERE profile_id = ?1)",
        )
        .bind(profile_id)
        .fetch_one(self.db.pool())
        .await?)
    }

    /// Every rating with when it was given: (title, stars, rated_at), oldest first.
    pub async fn ratings(&self, profile_id: i64) -> Result<Vec<(i64, i64, String)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT media_item_id, stars, rated_at FROM ratings WHERE profile_id = ? ORDER BY rated_at, media_item_id",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Every viewing in the log, oldest first.
    pub async fn viewings(&self, profile_id: i64) -> Result<Vec<Viewing>, DbError> {
        let rows: Vec<(i64, String, i64, i64, Option<i64>)> = sqlx::query_as(
            "SELECT media_item_id, started_at, watched_seconds, completed, stopped_at_seconds
             FROM watch_events WHERE profile_id = ? ORDER BY started_at, id",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(media_item_id, started_at, watched_seconds, completed, stopped_at_seconds)| {
                    Viewing {
                        media_item_id,
                        started_at,
                        watched_seconds,
                        completed: completed != 0,
                        stopped_at_seconds,
                    }
                },
            )
            .collect())
    }

    /// A light signal (Phase 15): "search" (opened from a search) or "detail" (a detail page
    /// opened). The same signal for the same title within a minute is one.
    pub async fn record_signal(
        &self,
        profile_id: i64,
        media_item_id: i64,
        kind: &str,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO signals (profile_id, media_item_id, kind)
             SELECT ?1, ?2, ?3
             WHERE NOT EXISTS (SELECT 1 FROM signals WHERE profile_id = ?1 AND media_item_id = ?2
                               AND kind = ?3 AND at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 minute'))",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .bind(kind)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Signals of one kind, oldest first: (title, when).
    pub async fn signals(
        &self,
        profile_id: i64,
        kind: &str,
    ) -> Result<Vec<(i64, String)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT media_item_id, at FROM signals WHERE profile_id = ? AND kind = ? ORDER BY at, id",
        )
        .bind(profile_id)
        .bind(kind)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// A watch that happened at `at` (an import, Phase 14): the date is kept, because the
    /// taste model weighs recent watching more. Importing the same file twice adds
    /// nothing: a watch of this title at this moment is recorded once. Returns whether it
    /// was new.
    pub async fn record_watch_at(
        &self,
        profile_id: i64,
        media_item_id: i64,
        watched_seconds: i64,
        completed: bool,
        at: &str,
    ) -> Result<bool, DbError> {
        let inserted = sqlx::query(
            "INSERT INTO watch_events (profile_id, media_item_id, watched_seconds, completed, started_at)
             SELECT ?1, ?2, ?3, ?4, ?5
             WHERE NOT EXISTS (SELECT 1 FROM watch_events
                               WHERE profile_id = ?1 AND media_item_id = ?2 AND started_at = ?5)",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .bind(watched_seconds)
        .bind(i64::from(completed))
        .bind(at)
        .execute(self.db.pool())
        .await?
        .rows_affected();
        Ok(inserted == 1)
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

    /// Whether this profile has seen it through: a completed watch, or a saved position
    /// past 95% (the line continue-watching draws, for the same reason: credits).
    pub async fn finished(&self, profile_id: i64, media_item_id: i64) -> Result<bool, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM watch_events
                 WHERE profile_id = ?1 AND media_item_id = ?2 AND completed = 1
             ) OR EXISTS (
                 SELECT 1 FROM playback_positions
                 WHERE profile_id = ?1 AND media_item_id = ?2
                   AND duration_seconds IS NOT NULL
                   AND position_seconds >= (duration_seconds * 95 / 100)
             )",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .fetch_one(self.db.pool())
        .await?)
    }

    /// What this profile liked, with a weight, strongest first (Phase 14's taste seed;
    /// Phase 15 builds the full model on the same signals): five stars 1.0, four 0.7, and
    /// a completed watch with no rating 0.3. Three stars and below are not liking.
    pub async fn liked(&self, profile_id: i64) -> Result<Vec<(i64, f64)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT media_item_id, CASE stars WHEN 5 THEN 1.0 ELSE 0.7 END AS weight
             FROM ratings WHERE profile_id = ?1 AND stars >= 4
             UNION ALL
             SELECT DISTINCT w.media_item_id, 0.3
             FROM watch_events w
             WHERE w.profile_id = ?1 AND w.completed = 1
               AND NOT EXISTS (SELECT 1 FROM ratings r
                               WHERE r.profile_id = ?1 AND r.media_item_id = w.media_item_id)
             ORDER BY weight DESC, media_item_id",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?)
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
        avatar_colour: row.try_get("avatar_colour")?,
        is_kids: row.try_get::<i64, _>("is_kids")? != 0,
        kids_limit: row.try_get("kids_limit")?,
        last_used_at: row.try_get("last_used_at")?,
        has_pin: row.try_get::<i64, _>("has_pin")? != 0,
    })
}
