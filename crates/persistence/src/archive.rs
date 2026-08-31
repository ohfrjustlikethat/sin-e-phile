//! Export and import a whole profile as a portable archive (`SPEC.md` Phase 3).
//!
//! WHAT IS IN IT, AND WHY THAT LIST.
//!
//! Only the data that cannot be rebuilt. The catalogue — media items, titles,
//! people, genres — is re-ingestible from public datasets by `tools/ingest`, so
//! putting it in the archive would turn a small file into a several-hundred-
//! megabyte one that goes stale. A user's viewing history exists nowhere else.
//!
//! Items are referenced by their EXTERNAL ids, never by internal row ids. Internal
//! ids are assigned in ingestion order, so the same film is a different number in
//! two installations; an archive keyed on them would restore a history pointing at
//! the wrong films, silently. That is the single most important decision in this
//! module.
//!
//! JSON rather than a binary format or a SQLite file: it is inspectable, it
//! diffs, and a user can see exactly what they are carrying between machines —
//! which matters for §2.4's no-telemetry posture, where "your data is yours" has
//! to be verifiable rather than promised.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::{Db, DbError};
use crate::model::IdSource;

/// Bumped when the shape changes. An importer refusing an unknown version is
/// better than one silently dropping fields it does not recognise.
pub const ARCHIVE_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("archive: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("archive io: {0}")]
    Io(#[from] std::io::Error),
    #[error("archive json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("archive version {found} is newer than this build understands ({ARCHIVE_VERSION})")]
    UnknownVersion { found: u32 },
    #[error("no profile named {0}")]
    NoSuchProfile(String),
}

/// How an item is named across installations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemRef {
    pub source: String,
    pub external_id: String,
    /// Carried for diagnostics and for a human reading the file. Never used to
    /// match on import — a title match would be a guess.
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchRecord {
    pub item: ItemRef,
    pub watched_seconds: i64,
    pub completed: bool,
    pub started_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionRecord {
    pub item: ItemRef,
    pub position_seconds: i64,
    pub duration_seconds: Option<i64>,
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistRecord {
    pub item: ItemRef,
    pub reason: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileArchive {
    pub version: u32,
    pub profile_name: String,
    pub exported_at: String,
    pub watch_events: Vec<WatchRecord>,
    pub positions: Vec<PositionRecord>,
    pub watchlist: Vec<WatchlistRecord>,
    pub settings: Vec<(String, String)>,
    /// Items in the source installation that carried no external id at all, and so
    /// could not be referenced portably. Reported rather than dropped in silence —
    /// the user is entitled to know their archive is incomplete and by how much.
    pub unreferenceable_items: Vec<String>,
}

/// What an import did. Returned rather than logged so a UI can show it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportSummary {
    pub watch_events: usize,
    pub positions: usize,
    pub watchlist: usize,
    pub settings: usize,
    /// Referenced items this installation's catalogue does not contain. Expected
    /// and harmless — a smaller catalogue, or one ingested at a different date.
    pub unmatched: Vec<ItemRef>,
}

pub struct Archiver<'a> {
    db: &'a Db,
}

impl<'a> Archiver<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub async fn export(&self, profile_name: &str) -> Result<ProfileArchive, ArchiveError> {
        let profile_id: Option<i64> = sqlx::query_scalar("SELECT id FROM profiles WHERE name = ?")
            .bind(profile_name)
            .fetch_optional(self.db.pool())
            .await?;
        let profile_id =
            profile_id.ok_or_else(|| ArchiveError::NoSuchProfile(profile_name.to_string()))?;

        // Preferred id source in order. IMDb first because it is the only one
        // guaranteed present in the offline catalogue (ADR-0013 made TMDB
        // optional), so an archive stays portable with no API key ever used.
        //
        // Takes its table alias, because the same expression is used in the outer
        // query and inside the correlated subquery. Hard-coding `e.source` made
        // the subquery read the OUTER row inside MIN(), which SQLite rejects
        // outright as "misuse of aggregate".
        fn preference(alias: &str) -> String {
            format!(
                "CASE {alias}.source WHEN 'imdb' THEN 0 WHEN 'tmdb' THEN 1
                      WHEN 'anilist' THEN 2 WHEN 'tvdb' THEN 3 WHEN 'mal' THEN 4
                      ELSE 5 END"
            )
        }
        let outer = preference("e");
        let inner = preference("e2");
        // One external id per item: the most portable one available.
        let best = format!(
            "{outer} = (SELECT MIN({inner}) FROM external_ids e2
                        WHERE e2.media_item_id = m.id)"
        );

        let watch_events = sqlx::query(&format!(
            "SELECT e.source, e.external_id, m.primary_title,
                    w.watched_seconds, w.completed, w.started_at
             FROM watch_events w
             JOIN media_items m ON m.id = w.media_item_id
             JOIN external_ids e ON e.media_item_id = m.id
             WHERE w.profile_id = ?
               AND {best}
             ORDER BY w.started_at"
        ))
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|row| {
            Ok(WatchRecord {
                item: item_ref(&row)?,
                watched_seconds: row.try_get("watched_seconds")?,
                completed: row.try_get::<i64, _>("completed")? != 0,
                started_at: row.try_get("started_at")?,
            })
        })
        .collect::<Result<Vec<_>, ArchiveError>>()?;

        let positions = sqlx::query(&format!(
            "SELECT e.source, e.external_id, m.primary_title,
                    p.position_seconds, p.duration_seconds,
                    p.audio_language, p.subtitle_language
             FROM playback_positions p
             JOIN media_items m ON m.id = p.media_item_id
             JOIN external_ids e ON e.media_item_id = m.id
             WHERE p.profile_id = ?
               AND {best}"
        ))
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|row| {
            Ok(PositionRecord {
                item: item_ref(&row)?,
                position_seconds: row.try_get("position_seconds")?,
                duration_seconds: row.try_get("duration_seconds")?,
                audio_language: row.try_get("audio_language")?,
                subtitle_language: row.try_get("subtitle_language")?,
            })
        })
        .collect::<Result<Vec<_>, ArchiveError>>()?;

        let watchlist = sqlx::query(&format!(
            "SELECT e.source, e.external_id, m.primary_title, w.reason, w.added_at
             FROM watchlist_items w
             JOIN media_items m ON m.id = w.media_item_id
             JOIN external_ids e ON e.media_item_id = m.id
             WHERE w.profile_id = ?
               AND {best}"
        ))
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|row| {
            Ok(WatchlistRecord {
                item: item_ref(&row)?,
                reason: row.try_get("reason")?,
                added_at: row.try_get("added_at")?,
            })
        })
        .collect::<Result<Vec<_>, ArchiveError>>()?;

        let settings: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
                .fetch_all(self.db.pool())
                .await?;

        // Anything the user cared about that cannot be carried. Honest reporting
        // rather than a silently short file.
        let unreferenceable_items: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT m.primary_title
             FROM media_items m
             WHERE NOT EXISTS (SELECT 1 FROM external_ids e WHERE e.media_item_id = m.id)
               AND (
                   EXISTS (SELECT 1 FROM watch_events w
                           WHERE w.media_item_id = m.id AND w.profile_id = ?1)
                OR EXISTS (SELECT 1 FROM watchlist_items i
                           WHERE i.media_item_id = m.id AND i.profile_id = ?1)
                OR EXISTS (SELECT 1 FROM playback_positions p
                           WHERE p.media_item_id = m.id AND p.profile_id = ?1)
               )",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(ProfileArchive {
            version: ARCHIVE_VERSION,
            profile_name: profile_name.to_string(),
            exported_at: now(),
            watch_events,
            positions,
            watchlist,
            settings,
            unreferenceable_items,
        })
    }

    pub async fn export_to_file(
        &self,
        profile_name: &str,
        path: &Path,
    ) -> Result<ProfileArchive, ArchiveError> {
        let archive = self.export(profile_name).await?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&archive)?)?;
        Ok(archive)
    }

    /// Restore into `profile_id`, matching items by external id.
    ///
    /// Idempotent: importing the same archive twice does not double a watch
    /// history, because a re-import is a normal thing for a user to do after a
    /// failed one and losing the difference is worse than skipping a duplicate.
    pub async fn import(
        &self,
        profile_id: i64,
        archive: &ProfileArchive,
    ) -> Result<ImportSummary, ArchiveError> {
        if archive.version > ARCHIVE_VERSION {
            return Err(ArchiveError::UnknownVersion {
                found: archive.version,
            });
        }

        let mut summary = ImportSummary::default();
        let mut tx = self.db.pool().begin().await?;

        for record in &archive.watch_events {
            let Some(id) = resolve(&mut tx, &record.item).await? else {
                summary.unmatched.push(record.item.clone());
                continue;
            };
            // Keyed on (profile, item, started_at): the same event re-imported is
            // the same row, a genuinely repeated viewing is a different timestamp.
            let existing: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM watch_events
                 WHERE profile_id = ? AND media_item_id = ? AND started_at = ?",
            )
            .bind(profile_id)
            .bind(id)
            .bind(&record.started_at)
            .fetch_optional(&mut *tx)
            .await?;
            if existing.is_some() {
                continue;
            }

            sqlx::query(
                "INSERT INTO watch_events
                     (profile_id, media_item_id, watched_seconds, completed, started_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(profile_id)
            .bind(id)
            .bind(record.watched_seconds)
            .bind(i64::from(record.completed))
            .bind(&record.started_at)
            .execute(&mut *tx)
            .await?;
            summary.watch_events += 1;
        }

        for record in &archive.positions {
            let Some(id) = resolve(&mut tx, &record.item).await? else {
                summary.unmatched.push(record.item.clone());
                continue;
            };
            sqlx::query(
                "INSERT INTO playback_positions
                     (profile_id, media_item_id, position_seconds, duration_seconds,
                      audio_language, subtitle_language)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT (profile_id, media_item_id) DO UPDATE
                     SET position_seconds = MAX(position_seconds, excluded.position_seconds)",
            )
            .bind(profile_id)
            .bind(id)
            .bind(record.position_seconds)
            .bind(record.duration_seconds)
            .bind(&record.audio_language)
            .bind(&record.subtitle_language)
            .execute(&mut *tx)
            .await?;
            summary.positions += 1;
        }

        for record in &archive.watchlist {
            let Some(id) = resolve(&mut tx, &record.item).await? else {
                summary.unmatched.push(record.item.clone());
                continue;
            };
            sqlx::query(
                "INSERT INTO watchlist_items (profile_id, media_item_id, reason, added_at)
                 VALUES (?, ?, ?, ?) ON CONFLICT DO NOTHING",
            )
            .bind(profile_id)
            .bind(id)
            .bind(&record.reason)
            .bind(&record.added_at)
            .execute(&mut *tx)
            .await?;
            summary.watchlist += 1;
        }

        for (key, value) in &archive.settings {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?, ?)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(value)
            .execute(&mut *tx)
            .await?;
            summary.settings += 1;
        }

        tx.commit().await?;
        Ok(summary)
    }

    pub async fn import_from_file(
        &self,
        profile_id: i64,
        path: &Path,
    ) -> Result<ImportSummary, ArchiveError> {
        let archive: ProfileArchive = serde_json::from_slice(&std::fs::read(path)?)?;
        self.import(profile_id, &archive).await
    }
}

/// External id to internal id, in THIS installation. Returns `None` when the
/// catalogue does not have the item — never a guess.
async fn resolve(
    tx: &mut sqlx::SqliteConnection,
    item: &ItemRef,
) -> Result<Option<i64>, ArchiveError> {
    Ok(sqlx::query_scalar(
        "SELECT media_item_id FROM external_ids WHERE source = ? AND external_id = ?",
    )
    .bind(&item.source)
    .bind(&item.external_id)
    .fetch_optional(&mut *tx)
    .await?)
}

fn item_ref(row: &sqlx::sqlite::SqliteRow) -> Result<ItemRef, ArchiveError> {
    Ok(ItemRef {
        source: row.try_get("source")?,
        external_id: row.try_get("external_id")?,
        title: row.try_get("primary_title")?,
    })
}

fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("unknown"))
}

/// Convenience: the sources an archive prefers, in order.
pub fn preferred_sources() -> [IdSource; 5] {
    [
        IdSource::Imdb,
        IdSource::Tmdb,
        IdSource::Anilist,
        IdSource::Tvdb,
        IdSource::Mal,
    ]
}
