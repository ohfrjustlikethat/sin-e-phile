//! The neighbour graph (SPEC.md Phase 17, migrations 0029 and 0031): each title's best
//! neighbours by who loved both, by source. MovieLens' matrix is written whole by its build
//! (`crates/recommend`, ADR-0019); AniList's and TMDB's recommendations are written a title
//! at a time as each is asked (ADR-0054 step 3). The ranker reads them together.

use std::collections::HashSet;

use crate::db::{Db, DbError};

/// Where a row came from (migration 0031's `source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NeighbourSource {
    MovieLens,
    AniList,
    Tmdb,
}

impl NeighbourSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MovieLens => "movielens",
            Self::AniList => "anilist",
            Self::Tmdb => "tmdb",
        }
    }
}

pub struct NeighbourRepository<'a> {
    db: &'a Db,
}

/// Rows per INSERT: three values each, well inside SQLite's limit on bound values.
const CHUNK: usize = 300;

impl<'a> NeighbourRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Replace MovieLens' whole matrix with `rows` (title, neighbour, score), in one
    /// transaction: a reader sees the old matrix or the new one, never half of each. The live
    /// sources' rows stay. Returns the rows written.
    pub async fn replace_all(&self, rows: &[(i64, i64, f32)]) -> Result<u64, DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM item_neighbours WHERE source = 'movielens'")
            .execute(&mut *tx)
            .await?;
        let mut written = 0;
        for chunk in rows.chunks(CHUNK) {
            let marks = vec!["(?, 'movielens', ?, ?)"; chunk.len()].join(", ");
            let sql = format!(
                "INSERT INTO item_neighbours (media_item_id, source, neighbour_id, score) \
                 VALUES {marks}"
            );
            let mut query = sqlx::query(&sql);
            for &(item, neighbour, score) in chunk {
                query = query.bind(item).bind(neighbour).bind(f64::from(score));
            }
            written += query.execute(&mut *tx).await?.rows_affected();
        }
        tx.commit().await?;
        Ok(written)
    }

    /// One source's rows for `item` replaced with `rows` (neighbour, score), and the title
    /// recorded as asked of it now, in one transaction. A title asked and not answered is
    /// recorded too, so it waits its week like the rest.
    pub async fn replace_for(
        &self,
        item: i64,
        source: NeighbourSource,
        rows: &[(i64, f32)],
    ) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM item_neighbours WHERE media_item_id = ? AND source = ?")
            .bind(item)
            .bind(source.as_str())
            .execute(&mut *tx)
            .await?;
        let mut seen = HashSet::new();
        for &(neighbour, score) in rows {
            // A title is not its own neighbour, and a list that repeats one keeps its first.
            if neighbour == item || !seen.insert(neighbour) {
                continue;
            }
            sqlx::query(
                "INSERT INTO item_neighbours (media_item_id, source, neighbour_id, score)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(item)
            .bind(source.as_str())
            .bind(neighbour)
            .bind(f64::from(score))
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            "INSERT INTO neighbours_asked (media_item_id, source, asked_at)
             VALUES (?, ?, datetime('now'))
             ON CONFLICT (media_item_id, source) DO UPDATE SET asked_at = excluded.asked_at",
        )
        .bind(item)
        .bind(source.as_str())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Those of `items` not asked of `source` in the last `days`, in the order given.
    pub async fn due(
        &self,
        source: NeighbourSource,
        items: &[i64],
        days: i64,
    ) -> Result<Vec<i64>, DbError> {
        let mut fresh = HashSet::new();
        for chunk in items.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT media_item_id FROM neighbours_asked
                 WHERE source = ? AND asked_at >= datetime('now', '-' || ? || ' days')
                   AND media_item_id IN ({marks})"
            );
            let mut query = sqlx::query_scalar::<_, i64>(&sql)
                .bind(source.as_str())
                .bind(days);
            for id in chunk {
                query = query.bind(id);
            }
            fresh.extend(query.fetch_all(self.db.pool()).await?);
        }
        Ok(items
            .iter()
            .copied()
            .filter(|id| !fresh.contains(id))
            .collect())
    }

    /// MovieLens' mean score at each rank, 1 to `ranks`: what a live source's k-th
    /// recommendation is worth beside the matrix, so neither scale outweighs the other.
    /// Empty before the matrix is built.
    pub async fn typical_scores(&self, ranks: i64) -> Result<Vec<f32>, DbError> {
        let rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT rank, AVG(score) FROM (
                 SELECT score, ROW_NUMBER() OVER (
                     PARTITION BY media_item_id ORDER BY score DESC) AS rank
                   FROM item_neighbours WHERE source = 'movielens')
              WHERE rank <= ? GROUP BY rank ORDER BY rank",
        )
        .bind(ranks)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|(_, s)| s as f32).collect())
    }

    /// When `source` last answered anything, and how many titles it has rows for: what
    /// Settings shows (ADR-0054).
    pub async fn last_asked(
        &self,
        source: NeighbourSource,
    ) -> Result<(Option<String>, i64), DbError> {
        let at: Option<String> =
            sqlx::query_scalar("SELECT MAX(asked_at) FROM neighbours_asked WHERE source = ?")
                .bind(source.as_str())
                .fetch_one(self.db.pool())
                .await?;
        let titles: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT media_item_id) FROM item_neighbours WHERE source = ?",
        )
        .bind(source.as_str())
        .fetch_one(self.db.pool())
        .await?;
        Ok((at, titles))
    }

    /// `item`'s best `limit` neighbours, best first, any source (a pair two sources agree on
    /// counts once, at its best score).
    pub async fn of(&self, item: i64, limit: i64) -> Result<Vec<(i64, f32)>, DbError> {
        let rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT neighbour_id, MAX(score) AS best FROM item_neighbours
             WHERE media_item_id = ? GROUP BY neighbour_id
             ORDER BY best DESC, neighbour_id LIMIT ?",
        )
        .bind(item)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|(n, s)| (n, s as f32)).collect())
    }

    /// Every stored neighbour of each of `items`: (title, neighbour, score), any source, a
    /// pair two sources agree on once at its best score. What the ranker sums over a
    /// profile's liked titles.
    pub async fn of_many(&self, items: &[i64]) -> Result<Vec<(i64, i64, f32)>, DbError> {
        let mut out = Vec::new();
        for chunk in items.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT media_item_id, neighbour_id, MAX(score) FROM item_neighbours
                 WHERE media_item_id IN ({marks}) GROUP BY media_item_id, neighbour_id"
            );
            let mut query = sqlx::query_as::<_, (i64, i64, f64)>(&sql);
            for id in chunk {
                query = query.bind(id);
            }
            out.extend(
                query
                    .fetch_all(self.db.pool())
                    .await?
                    .into_iter()
                    .map(|(i, n, s)| (i, n, s as f32)),
            );
        }
        Ok(out)
    }

    /// How many titles have MovieLens neighbours: 0 until the matrix is built (what decides
    /// whether a launch must build it, so the live sources' rows are not counted).
    pub async fn titles(&self) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(DISTINCT media_item_id) FROM item_neighbours WHERE source = 'movielens'",
        )
        .fetch_one(self.db.pool())
        .await?)
    }
}
