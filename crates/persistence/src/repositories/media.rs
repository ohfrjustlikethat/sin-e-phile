//! Reads and writes for media items, titles, and external ids.
//!
//! Every SQL statement in the application lives in a repository like this one.
//! Callers get typed methods; nothing above this layer knows the schema exists.

use sqlx::Row;

use crate::db::{Db, DbError};
use crate::model::{IdSource, MediaItem, MediaKind, NewMediaItem, Title, TitleVariant};

pub struct MediaRepository<'a> {
    db: &'a Db,
}

impl<'a> MediaRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Insert an item and its primary title, returning the new id.
    ///
    /// One transaction, because an item without its title row is a half-created
    /// record that every later join has to defend against. Failing atomically is
    /// simpler than tolerating the intermediate state everywhere else.
    pub async fn insert(&self, item: &NewMediaItem) -> Result<i64, DbError> {
        let kind = item.kind.unwrap_or(MediaKind::Film);
        let mut tx = self.db.pool().begin().await?;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO media_items (
                 kind, primary_title, sort_title, release_year, release_date,
                 runtime_minutes, original_language, countries, synopsis,
                 rating, rating_votes, is_adult
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(kind.as_str())
        .bind(&item.primary_title)
        .bind(&item.sort_title)
        .bind(item.release_year)
        .bind(&item.release_date)
        .bind(item.runtime_minutes)
        .bind(&item.original_language)
        .bind(&item.countries)
        .bind(&item.synopsis)
        .bind(item.rating)
        .bind(item.rating_votes)
        .bind(i64::from(item.is_adult))
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, language)
             VALUES (?, ?, 'primary', ?)",
        )
        .bind(id)
        .bind(&item.primary_title)
        .bind(&item.original_language)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(id)
    }

    pub async fn by_id(&self, id: i64) -> Result<Option<MediaItem>, DbError> {
        let row = sqlx::query(
            "SELECT id, kind, primary_title, sort_title, release_year, release_date,
                    runtime_minutes, original_language, countries, synopsis,
                    rating, rating_votes, is_adult
             FROM media_items WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(map_media_item).transpose()
    }

    /// Resolve an external id to an internal item. The lookup ingestion performs
    /// constantly, and the reason `idx_external_ids_lookup` exists.
    pub async fn by_external_id(
        &self,
        source: IdSource,
        external_id: &str,
    ) -> Result<Option<MediaItem>, DbError> {
        let row = sqlx::query(
            "SELECT m.id, m.kind, m.primary_title, m.sort_title, m.release_year,
                    m.release_date, m.runtime_minutes, m.original_language,
                    m.countries, m.synopsis, m.rating, m.rating_votes, m.is_adult
             FROM media_items m
             JOIN external_ids e ON e.media_item_id = m.id
             WHERE e.source = ? AND e.external_id = ?",
        )
        .bind(source.as_str())
        .bind(external_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(map_media_item).transpose()
    }

    /// Exact title match across every variant.
    ///
    /// Phase 5's exit criterion is a 100% exact-title top-1 rate, and this is the
    /// short-circuit that guarantees it before any ranking runs. `COLLATE NOCASE`
    /// rather than `LOWER()` on both sides so the index is still usable.
    pub async fn by_exact_title(&self, title: &str) -> Result<Vec<MediaItem>, DbError> {
        let rows = sqlx::query(
            "SELECT DISTINCT m.id, m.kind, m.primary_title, m.sort_title, m.release_year,
                    m.release_date, m.runtime_minutes, m.original_language,
                    m.countries, m.synopsis, m.rating, m.rating_votes, m.is_adult
             FROM media_items m
             JOIN titles t ON t.media_item_id = m.id
             WHERE t.title = ? COLLATE NOCASE
             ORDER BY m.rating_votes DESC NULLS LAST",
        )
        .bind(title)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter().map(map_media_item).collect()
    }

    pub async fn add_external_id(
        &self,
        media_item_id: i64,
        source: IdSource,
        external_id: &str,
        confidence: f64,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO external_ids (media_item_id, source, external_id, confidence)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (media_item_id, source) DO UPDATE
                 SET external_id = excluded.external_id,
                     confidence  = excluded.confidence",
        )
        .bind(media_item_id)
        .bind(source.as_str())
        .bind(external_id)
        .bind(confidence)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn add_title(
        &self,
        media_item_id: i64,
        title: &str,
        variant: TitleVariant,
        language: Option<&str>,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, language)
             VALUES (?, ?, ?, ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(media_item_id)
        .bind(title)
        .bind(variant.as_str())
        .bind(language)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn titles_for(&self, media_item_id: i64) -> Result<Vec<Title>, DbError> {
        let rows = sqlx::query(
            "SELECT id, media_item_id, title, variant, language, region
             FROM titles WHERE media_item_id = ? ORDER BY variant",
        )
        .bind(media_item_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Title {
                    id: row.try_get("id")?,
                    media_item_id: row.try_get("media_item_id")?,
                    title: row.try_get("title")?,
                    variant: parse_variant(row.try_get::<String, _>("variant")?.as_str()),
                    language: row.try_get("language")?,
                    region: row.try_get("region")?,
                })
            })
            .collect()
    }

    pub async fn count(&self) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM media_items")
            .fetch_one(self.db.pool())
            .await?)
    }

    /// Bulk insert, for ingestion and for the 500,000-row benchmark.
    ///
    /// One transaction for the whole batch. SQLite commits are the cost here, not
    /// the inserts: committing per row turns a few seconds into tens of minutes,
    /// because each commit is a durability barrier.
    pub async fn insert_many(&self, items: &[NewMediaItem]) -> Result<Vec<i64>, DbError> {
        let mut tx = self.db.pool().begin().await?;
        let mut ids = Vec::with_capacity(items.len());

        for item in items {
            let kind = item.kind.unwrap_or(MediaKind::Film);
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO media_items (
                     kind, primary_title, sort_title, release_year, runtime_minutes,
                     original_language, synopsis, rating, rating_votes, is_adult
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING id",
            )
            .bind(kind.as_str())
            .bind(&item.primary_title)
            .bind(&item.sort_title)
            .bind(item.release_year)
            .bind(item.runtime_minutes)
            .bind(&item.original_language)
            .bind(&item.synopsis)
            .bind(item.rating)
            .bind(item.rating_votes)
            .bind(i64::from(item.is_adult))
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO titles (media_item_id, title, variant) VALUES (?, ?, 'primary')",
            )
            .bind(id)
            .bind(&item.primary_title)
            .execute(&mut *tx)
            .await?;

            ids.push(id);
        }

        tx.commit().await?;
        Ok(ids)
    }
}

fn map_media_item(row: sqlx::sqlite::SqliteRow) -> Result<MediaItem, DbError> {
    Ok(MediaItem {
        id: row.try_get("id")?,
        kind: parse_kind(row.try_get::<String, _>("kind")?.as_str()),
        primary_title: row.try_get("primary_title")?,
        sort_title: row.try_get("sort_title")?,
        release_year: row.try_get("release_year")?,
        release_date: row.try_get("release_date").unwrap_or(None),
        runtime_minutes: row.try_get("runtime_minutes")?,
        original_language: row.try_get("original_language")?,
        countries: row.try_get("countries").unwrap_or(None),
        synopsis: row.try_get("synopsis")?,
        rating: row.try_get("rating")?,
        rating_votes: row.try_get("rating_votes")?,
        is_adult: row.try_get::<i64, _>("is_adult")? != 0,
    })
}

/// The schema's `CHECK` constraint is the gate, so anything reaching here is
/// already one of the eight. Falling back to `Film` rather than panicking keeps a
/// corrupted row from taking down a library scan.
fn parse_kind(s: &str) -> MediaKind {
    MediaKind::ALL
        .into_iter()
        .find(|k| k.as_str() == s)
        .unwrap_or(MediaKind::Film)
}

fn parse_variant(s: &str) -> TitleVariant {
    TitleVariant::ALL
        .into_iter()
        .find(|v| v.as_str() == s)
        .unwrap_or(TitleVariant::Alternative)
}
