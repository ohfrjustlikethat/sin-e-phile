//! The item-item matrix (SPEC.md Phase 17, migration 0029): each title's best neighbours by
//! who loved both, written whole by the build (`crates/recommend`, ADR-0019) and read by the
//! ranker.

use crate::db::{Db, DbError};

pub struct NeighbourRepository<'a> {
    db: &'a Db,
}

/// Rows per INSERT: three values each, well inside SQLite's limit on bound values.
const CHUNK: usize = 300;

impl<'a> NeighbourRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Replace the whole matrix with `rows` (title, neighbour, score), in one transaction:
    /// a reader sees the old matrix or the new one, never half of each. Returns the rows
    /// written.
    pub async fn replace_all(&self, rows: &[(i64, i64, f32)]) -> Result<u64, DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM item_neighbours")
            .execute(&mut *tx)
            .await?;
        let mut written = 0;
        for chunk in rows.chunks(CHUNK) {
            let marks = vec!["(?, ?, ?)"; chunk.len()].join(", ");
            let sql = format!(
                "INSERT INTO item_neighbours (media_item_id, neighbour_id, score) VALUES {marks}"
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

    /// `item`'s best `limit` neighbours, best first.
    pub async fn of(&self, item: i64, limit: i64) -> Result<Vec<(i64, f32)>, DbError> {
        let rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT neighbour_id, score FROM item_neighbours
             WHERE media_item_id = ? ORDER BY score DESC, neighbour_id LIMIT ?",
        )
        .bind(item)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|(n, s)| (n, s as f32)).collect())
    }

    /// Every stored neighbour of each of `items`: (title, neighbour, score). What the ranker
    /// sums over a profile's liked titles.
    pub async fn of_many(&self, items: &[i64]) -> Result<Vec<(i64, i64, f32)>, DbError> {
        let mut out = Vec::new();
        for chunk in items.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT media_item_id, neighbour_id, score FROM item_neighbours
                 WHERE media_item_id IN ({marks})"
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

    /// How many titles have neighbours: 0 until the matrix is built.
    pub async fn titles(&self) -> Result<i64, DbError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(DISTINCT media_item_id) FROM item_neighbours")
                .fetch_one(self.db.pool())
                .await?,
        )
    }
}
