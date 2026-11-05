//! Search-triggered backfill (ADR-0030 layer 3, migration 0017).
//!
//! A backfilled row is the minimum that makes the next search find it: a media item, its
//! primary title with the SAME normalisation the exact-title tier queries, and an entry in
//! both full-text indexes. It is marked with the backend that reported it.

use crate::db::{Db, DbError};
use crate::repositories::search::normalise_query;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfilledRow {
    pub id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub from: String,
}

pub struct BackfillRepository<'a> {
    db: &'a Db,
}

impl<'a> BackfillRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// An item already titled this (normalised) in this year, if there is one — any
    /// item, backfilled or not. Backfill must never add a second copy of a film the
    /// catalogue has; a search can miss for reasons other than absence.
    pub async fn existing(&self, title: &str, year: Option<i64>) -> Result<Option<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT m.id FROM titles t JOIN media_items m ON m.id = t.media_item_id
             WHERE t.normalised = ?1 AND m.kind = 'film'
               AND (?2 IS NULL OR m.release_year IS NULL OR m.release_year = ?2)
             LIMIT 1",
        )
        .bind(normalise_query(title))
        .bind(year)
        .fetch_optional(self.db.pool())
        .await?)
    }

    /// Create a film row from what a backend reported, findable at once.
    pub async fn create_film(
        &self,
        title: &str,
        year: Option<i64>,
        backend: &str,
    ) -> Result<i64, DbError> {
        let normalised = normalise_query(title);
        if normalised.is_empty() {
            return Err(DbError::Invalid(
                "a backfilled title cannot be empty".into(),
            ));
        }
        let mut tx = self.db.pool().begin().await?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO media_items (kind, primary_title, release_year, backfilled_from)
             VALUES ('film', ?1, ?2, ?3) RETURNING id",
        )
        .bind(title)
        .bind(year)
        .bind(backend)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, normalised)
             VALUES (?1, ?2, 'primary', ?3)",
        )
        .bind(id)
        .bind(title)
        .bind(&normalised)
        .execute(&mut *tx)
        .await?;
        // Both indexes, as `SearchRepository::index_item` and `index_trigram` write them,
        // in the same transaction so a half-indexed row cannot exist.
        sqlx::query(
            "INSERT INTO search_index (rowid, title, alternative_titles, people, keywords)
             VALUES (?1, ?2, '', '', '')",
        )
        .bind(id)
        .bind(title)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO search_indexed (media_item_id, generation) VALUES (?1, 1)")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO search_trigram (rowid, title) VALUES (?1, ?2)")
            .bind(id)
            .bind(title)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Every backfilled row, for reconciliation (Phase 12) and for the record.
    pub async fn all(&self) -> Result<Vec<BackfilledRow>, DbError> {
        let rows: Vec<(i64, String, Option<i64>, String)> = sqlx::query_as(
            "SELECT id, primary_title, release_year, backfilled_from
             FROM media_items WHERE backfilled_from IS NOT NULL ORDER BY id",
        )
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, title, year, from)| BackfilledRow {
                id,
                title,
                year,
                from,
            })
            .collect())
    }
}
