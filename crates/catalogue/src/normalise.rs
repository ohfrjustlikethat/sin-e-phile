//! Backfilling `titles.normalised` (migration 0009).
//!
//! The rule lives in [`crate::matching::normalise`] and is applied here, in Rust,
//! rather than duplicated in SQL — two definitions of "the same title" would drift
//! apart and the drift would be invisible until a match silently stopped happening.
//!
//! Runs as a resumable step like everything else, because it touches six million
//! rows and a laptop that sleeps should not mean starting again.

use sinephile_persistence::Db;

use crate::job::{Batch, Job, JobError, SqliteTx};
use crate::matching::normalise;

/// Rows per transaction. Each is a tiny UPDATE, so the commit dominates and a large
/// batch costs nothing extra on a crash beyond redoing cheap work.
const BATCH: i64 = 50_000;

/// Backfill every title's normalised form.
pub async fn backfill(job: &mut Job<'_>) -> Result<(), JobError> {
    job.run_step("titles.normalised", move |tx, cursor| {
        Box::pin(async move {
            // The cursor is the last id processed. `titles.id` is the primary key,
            // so this is an index scan rather than an offset, and resuming near the
            // end costs a seek instead of re-reading six million rows.
            let after: i64 = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);

            let rows: Vec<(i64, String)> =
                sqlx::query_as("SELECT id, title FROM titles WHERE id > ? ORDER BY id LIMIT ?")
                    .bind(after)
                    .bind(BATCH)
                    .fetch_all(&mut **tx)
                    .await
                    .map_err(|e| JobError::step("titles.normalised", e.to_string()))?;

            if rows.is_empty() {
                return Ok(Batch::finished(0));
            }

            let last_id = rows.last().map(|(id, _)| *id).unwrap_or(after);
            let count = rows.len() as i64;
            update(tx, &rows).await?;

            Ok(if count < BATCH {
                Batch::finished(count)
            } else {
                Batch::more(last_id.to_string(), count)
            })
        })
    })
    .await?;
    Ok(())
}

async fn update(tx: &mut SqliteTx<'_>, rows: &[(i64, String)]) -> Result<(), JobError> {
    // A CASE expression updates the whole chunk in one statement. One UPDATE per row
    // would be fifty thousand round trips per batch.
    for chunk in rows.chunks(200) {
        let mut sql = String::from("UPDATE titles SET normalised = CASE id ");
        for _ in chunk {
            sql.push_str("WHEN ? THEN ? ");
        }
        sql.push_str("END WHERE id IN (");
        sql.push_str(&vec!["?"; chunk.len()].join(", "));
        sql.push(')');

        let mut query = sqlx::query(&sql);
        for (id, title) in chunk {
            query = query.bind(id).bind(normalise(title));
        }
        for (id, _) in chunk {
            query = query.bind(id);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("titles.normalised", e.to_string()))?;
    }
    Ok(())
}

/// How many titles still have no normalised form.
pub async fn remaining(db: &Db) -> Result<i64, JobError> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM titles WHERE normalised IS NULL")
            .fetch_one(db.pool())
            .await?,
    )
}

/// Catalogue rows whose normalised title matches any of `forms`.
///
/// One query for every form at once rather than one per form: AniList gives three,
/// and three round trips per title over twenty thousand titles is sixty thousand
/// queries where twenty thousand will do.
pub async fn candidates(
    db: &Db,
    forms: &[String],
) -> Result<Vec<crate::matching::Candidate>, JobError> {
    if forms.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = vec!["?"; forms.len()].join(", ");
    let sql = format!(
        "SELECT DISTINCT t.media_item_id, t.title, m.release_year, m.kind
         FROM titles t
         JOIN media_items m ON m.id = t.media_item_id
         WHERE t.normalised IN ({placeholders})"
    );

    let mut query = sqlx::query_as::<_, (i64, String, Option<i64>, String)>(&sql);
    for form in forms {
        query = query.bind(form);
    }

    Ok(query
        .fetch_all(db.pool())
        .await?
        .into_iter()
        .map(
            |(media_item_id, title, year, kind)| crate::matching::Candidate {
                media_item_id,
                title,
                year,
                kind,
            },
        )
        .collect())
}
