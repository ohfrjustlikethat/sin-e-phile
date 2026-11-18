//! Meaning for what has none (ADR-0054 step 2, migration 0030): the core titles the published
//! meaning file does not hold (gained since, or without a synopsis when it was made), which
//! the launch job describes and embeds into the companion graph beside the published one.

use crate::db::{Db, DbError};
use crate::repositories::catalogue::{CORE_TIER, MIN_SYNOPSIS};

/// `(id, kind, imdb id, anilist id, described)` as `awaiting` reads it.
type AwaitingRow = (i64, String, Option<String>, Option<String>, bool);

pub struct MeaningRepository<'a> {
    db: &'a Db,
}

/// A title waiting for its meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awaiting {
    pub id: i64,
    pub kind: String,
    pub imdb_id: Option<String>,
    pub anilist_id: Option<i64>,
    /// It has a synopsis long enough to index already: embed it, ask nobody.
    pub described: bool,
}

/// When a title nobody could describe is asked about again: soon for a recent one (its
/// article is often written in the weeks after it comes out), rarely for an old one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retry {
    /// Released this year or later counts as recent.
    pub recent_year: i64,
    pub recent_days: i64,
    pub other_days: i64,
}

/// What the job has done, all launches together: for Settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeaningCounts {
    /// Titles asked for a synopsis.
    pub asked: i64,
    /// Titles a source described.
    pub described: i64,
    /// Titles given a vector.
    pub embedded: i64,
}

impl<'a> MeaningRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Core titles with no vector in either graph, worth a turn: described already, never
    /// asked, or last asked longer ago than `retry` allows. Most-voted first, so a launch's
    /// cap takes what people look for.
    ///
    /// The published graph holds the core titles up to `after` (the published file's last
    /// id) that were described when it was made; this job never touches those, so a title
    /// at or before `after`, described, with no row here, is in it.
    pub async fn awaiting(
        &self,
        after: i64,
        retry: Retry,
        limit: i64,
    ) -> Result<Vec<Awaiting>, DbError> {
        let described =
            format!("(m.synopsis IS NOT NULL AND length(trim(m.synopsis)) >= {MIN_SYNOPSIS})");
        let sql = format!(
            "SELECT m.id, m.kind, i.external_id, a.external_id, {described}
               FROM media_items m
               LEFT JOIN new_meaning n ON n.media_item_id = m.id
               LEFT JOIN external_ids i ON i.media_item_id = m.id AND i.source = 'imdb'
               LEFT JOIN external_ids a ON a.media_item_id = m.id AND a.source = 'anilist'
              WHERE {CORE_TIER}
                AND n.embedded_at IS NULL
                AND NOT (m.id <= ? AND {described} AND n.media_item_id IS NULL)
                AND ({described} OR n.asked_at IS NULL
                     OR n.asked_at < datetime('now', '-' ||
                          CASE WHEN m.release_year >= ? THEN ? ELSE ? END || ' days'))
              ORDER BY m.rating_votes DESC NULLS LAST, m.id
              LIMIT ?"
        );
        let rows: Vec<AwaitingRow> = sqlx::query_as(&sql)
            .bind(after)
            .bind(retry.recent_year)
            .bind(retry.recent_days)
            .bind(retry.other_days)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?;
        Ok(rows
            .into_iter()
            .map(|(id, kind, imdb_id, anilist, described)| Awaiting {
                id,
                kind,
                imdb_id,
                anilist_id: anilist.and_then(|a| a.parse().ok()),
                described,
            })
            .collect())
    }

    /// Record that these titles were asked for a synopsis now.
    pub async fn asked(&self, ids: &[i64]) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        for id in ids {
            sqlx::query(
                "INSERT INTO new_meaning (media_item_id, asked_at) VALUES (?, datetime('now'))
                 ON CONFLICT (media_item_id) DO UPDATE SET asked_at = excluded.asked_at",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Store a synopsis a source gave, and which source: one transaction, so a synopsis is
    /// never kept without its source or the reverse.
    pub async fn described(&self, id: i64, text: &str, source: &str) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("UPDATE media_items SET synopsis = ? WHERE id = ?")
            .bind(text)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO new_meaning (media_item_id, source) VALUES (?, ?)
             ON CONFLICT (media_item_id) DO UPDATE SET source = excluded.source",
        )
        .bind(id)
        .bind(source)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Those of `ids` whose synopsis the job wrote: their text is not what the published
    /// file was made from, knowingly, so a check of the file against the catalogue passes
    /// them by.
    pub async fn described_ids(
        &self,
        ids: &[i64],
    ) -> Result<std::collections::HashSet<i64>, DbError> {
        let mut out = std::collections::HashSet::new();
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT media_item_id FROM new_meaning
                  WHERE source IS NOT NULL AND media_item_id IN ({marks})"
            );
            let mut query = sqlx::query_scalar::<_, i64>(&sql);
            for id in chunk {
                query = query.bind(id);
            }
            out.extend(query.fetch_all(self.db.pool()).await?);
        }
        Ok(out)
    }

    /// Record that these titles' vectors are in the companion graph.
    pub async fn embedded(&self, ids: &[i64]) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        for id in ids {
            sqlx::query(
                "INSERT INTO new_meaning (media_item_id, embedded_at) VALUES (?, datetime('now'))
                 ON CONFLICT (media_item_id) DO UPDATE SET embedded_at = excluded.embedded_at",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// The companion graph is gone (deleted, or a new model): every vector it held is to be
    /// made again. Returns how many titles that is.
    pub async fn forget_embedded(&self) -> Result<u64, DbError> {
        Ok(
            sqlx::query("UPDATE new_meaning SET embedded_at = NULL WHERE embedded_at IS NOT NULL")
                .execute(self.db.pool())
                .await?
                .rows_affected(),
        )
    }

    pub async fn counts(&self) -> Result<MeaningCounts, DbError> {
        let (asked, described, embedded): (i64, i64, i64) = sqlx::query_as(
            "SELECT COUNT(asked_at), COUNT(source), COUNT(embedded_at) FROM new_meaning",
        )
        .fetch_one(self.db.pool())
        .await?;
        Ok(MeaningCounts {
            asked,
            described,
            embedded,
        })
    }
}
