//! Regions of the catalogue (Phase 18, ADR-0055, migration 0033): the countries a title comes
//! from and the film movements it belongs to, as Wikidata names them, and which titles have
//! been asked. The discovery engine reads them through `MediaRepository::facts`.

use std::collections::HashSet;

use crate::db::{Db, DbError};

/// What one title's answer was: its countries and movements, either possibly empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Regions {
    pub media_item_id: i64,
    pub countries: Vec<String>,
    pub movements: Vec<String>,
}

/// How far the asking has got, for Settings' list of sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionCounts {
    pub asked: i64,
    pub with_country: i64,
    pub with_movement: i64,
    /// When the last title was asked (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub last_asked: Option<String>,
}

pub struct RegionRepository<'a> {
    db: &'a Db,
}

/// The kinds a region is asked for: what a person watches whole.
const KINDS: &str = "'film', 'anime_film', 'series', 'anime_series'";

impl<'a> RegionRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Titles some profile has met (rated, watched, started, saved, or given a signal) and
    /// that were never asked, with their IMDb ids: asked first, because a blind spot is
    /// judged against them. An episode counts as its series.
    pub async fn awaiting_met(&self, limit: i64) -> Result<Vec<(i64, String)>, DbError> {
        Ok(sqlx::query_as(&format!(
            "WITH touched(id) AS (
                 SELECT media_item_id FROM ratings
                 UNION SELECT media_item_id FROM watch_events
                 UNION SELECT media_item_id FROM playback_positions
                 UNION SELECT media_item_id FROM watchlist_items
                 UNION SELECT media_item_id FROM signals
             ),
             titles(id) AS (
                 SELECT COALESCE(e.series_id, t.id) FROM touched t
                 LEFT JOIN episodes e ON e.media_item_id = t.id
             )
             SELECT DISTINCT m.id, x.external_id
             FROM titles t
             JOIN media_items m ON m.id = t.id
             JOIN external_ids x ON x.media_item_id = m.id AND x.source = 'imdb'
             WHERE m.kind IN ({KINDS})
               AND NOT EXISTS (SELECT 1 FROM regions_asked a WHERE a.media_item_id = m.id)
             ORDER BY m.id
             LIMIT ?"
        ))
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Titles with at least `min_votes` that were never asked, most voted first, with their
    /// IMDb ids: the acclaimed and the widely seen are what the arms choose from.
    pub async fn awaiting(
        &self,
        min_votes: i64,
        limit: i64,
    ) -> Result<Vec<(i64, String)>, DbError> {
        Ok(sqlx::query_as(&format!(
            "SELECT m.id, x.external_id
             FROM media_items m
             JOIN external_ids x ON x.media_item_id = m.id AND x.source = 'imdb'
             WHERE m.rating_votes >= ? AND m.is_adult = 0 AND +m.kind IN ({KINDS})
               AND NOT EXISTS (SELECT 1 FROM regions_asked a WHERE a.media_item_id = m.id)
             ORDER BY m.rating_votes DESC, m.id
             LIMIT ?"
        ))
        .bind(min_votes)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Record what Wikidata said of each title in `answers` (an empty answer included, so it
    /// is not asked again), replacing anything known of it before, in one transaction.
    pub async fn record(&self, answers: &[Regions]) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        for a in answers {
            sqlx::query("DELETE FROM title_regions WHERE media_item_id = ?")
                .bind(a.media_item_id)
                .execute(&mut *tx)
                .await?;
            for (axis, names) in [("country", &a.countries), ("movement", &a.movements)] {
                for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
                    sqlx::query(
                        "INSERT OR IGNORE INTO title_regions (media_item_id, axis, name) VALUES (?, ?, ?)",
                    )
                    .bind(a.media_item_id)
                    .bind(axis)
                    .bind(name)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            sqlx::query(
                "INSERT INTO regions_asked (media_item_id) VALUES (?)
                 ON CONFLICT (media_item_id) DO UPDATE SET asked_at = datetime('now')",
            )
            .bind(a.media_item_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Which of `ids` were never asked: what a profile has met and Wikidata has not yet been
    /// asked about, so its countries are not known.
    pub async fn unasked(&self, ids: &[i64]) -> Result<HashSet<i64>, DbError> {
        let mut out: HashSet<i64> = ids.iter().copied().collect();
        for chunk in ids.chunks(500) {
            let sql = format!(
                "SELECT media_item_id FROM regions_asked WHERE media_item_id IN ({})",
                vec!["?"; chunk.len()].join(",")
            );
            let mut q = sqlx::query_scalar::<_, i64>(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            for asked in q.fetch_all(self.db.pool()).await? {
                out.remove(&asked);
            }
        }
        Ok(out)
    }

    pub async fn counts(&self) -> Result<RegionCounts, DbError> {
        let (asked, last_asked): (i64, Option<String>) =
            sqlx::query_as("SELECT COUNT(*), MAX(asked_at) FROM regions_asked")
                .fetch_one(self.db.pool())
                .await?;
        let (with_country, with_movement): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(DISTINCT CASE WHEN axis = 'country' THEN media_item_id END),
                    COUNT(DISTINCT CASE WHEN axis = 'movement' THEN media_item_id END)
             FROM title_regions",
        )
        .fetch_one(self.db.pool())
        .await?;
        Ok(RegionCounts {
            asked,
            with_country,
            with_movement,
            last_asked,
        })
    }
}
