//! Building the FTS5 index over the catalogue (Phase 5, subtask 5.1).
//!
//! `SearchRepository::index_item` writes one item per transaction, which is right for
//! the application re-indexing something a user changed and hopeless for 2.7 million
//! rows. This is the bulk path: one transaction per batch, one query per batch.
//!
//! # The two indexes are built separately, on purpose
//!
//! The trigram tokenizer indexes every three-character window of every string, so it
//! costs far more per row than the word tokenizer and answers a different question.
//! Migration 0011 keeps them in separate tables so that this module can build one,
//! measure it, build the other, measure that — and so the trigram index can be narrowed
//! to the core tier or dropped entirely without touching the main one.
//!
//! R4's headroom is 452 MB and the trigram index is the largest unknown in Phase 5.
//! Measuring the two together would tell us the total and nothing about which half to
//! cut.

use sinephile_persistence::Db;

use crate::job::{Batch, Job, JobError, SqliteTx};

/// Items per transaction.
const BATCH: i64 = 2_000;

/// What one item contributes to the index.
type Row = (i64, String, Option<String>, Option<String>, Option<String>);

/// Read a batch, assembling every column in ONE query.
///
/// Correlated `group_concat` subqueries rather than a query per item: at 2.7 million
/// items, three extra round trips each is eight million queries, and the whole job
/// becomes a benchmark of SQLite's statement overhead.
async fn read_batch(db: &Db, after: i64, limit: i64) -> Result<Vec<Row>, JobError> {
    Ok(sqlx::query_as(
        "SELECT m.id,
                m.primary_title,
                (SELECT group_concat(t.title, ' ')
                   FROM titles t
                  WHERE t.media_item_id = m.id AND t.variant <> 'primary'),
                (SELECT group_concat(p.name, ' ')
                   FROM credits c JOIN people p ON p.id = c.person_id
                  WHERE c.media_item_id = m.id),
                (SELECT group_concat(g.name, ' ')
                   FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                  WHERE mg.media_item_id = m.id)
           FROM media_items m
          WHERE m.id > ? AND m.kind <> 'episode'
          ORDER BY m.id
          LIMIT ?",
    )
    .bind(after)
    .bind(limit)
    .fetch_all(db.pool())
    .await?)
}

/// Build the main word index over every non-episode title.
///
/// Episodes are excluded: half a million rows named "Episode #1.7" would swamp the
/// index with text nobody searches for, and an episode is reached through its series.
pub async fn build_main(job: &mut Job<'_>, db: &Db) -> Result<i64, JobError> {
    let indexed = std::sync::Arc::new(std::sync::Mutex::new(0i64));
    let sink = std::sync::Arc::clone(&indexed);
    let db = db.clone();

    job.run_step("search.main", move |tx, cursor| {
        let db = db.clone();
        let sink = std::sync::Arc::clone(&sink);
        Box::pin(async move {
            let after: i64 = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
            let rows = read_batch(&db, after, BATCH).await?;
            if rows.is_empty() {
                return Ok(Batch::finished(0));
            }

            let last = rows.last().map(|r| r.0).unwrap_or(after);
            let count = rows.len() as i64;
            insert_main(tx, &rows).await?;
            *sink.lock().expect("count") += count;

            Ok(if count < BATCH {
                Batch::finished(count)
            } else {
                Batch::more(last.to_string(), count)
            })
        })
    })
    .await?;

    let n = *indexed.lock().expect("count");
    Ok(n)
}

async fn insert_main(tx: &mut SqliteTx<'_>, rows: &[Row]) -> Result<(), JobError> {
    for chunk in rows.chunks(200) {
        // DELETE first, so a re-run replaces rather than duplicating. Safe on a missing
        // rowid only because migration 0011 declares contentless_delete=1 — without it
        // this silently corrupts the index.
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let delete_sql = format!("DELETE FROM search_index WHERE rowid IN ({placeholders})");
        let mut delete = sqlx::query(&delete_sql);
        for row in chunk {
            delete = delete.bind(row.0);
        }
        delete
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("search.main", e.to_string()))?;

        let values = vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ");
        let insert_sql = format!(
            "INSERT INTO search_index(rowid, title, alternative_titles, people, keywords)
             VALUES {values}"
        );
        let mut insert = sqlx::query(&insert_sql);
        for (id, title, alternatives, people, genres) in chunk {
            insert = insert
                .bind(id)
                .bind(title)
                .bind(alternatives.as_deref().unwrap_or(""))
                .bind(people.as_deref().unwrap_or(""))
                .bind(genres.as_deref().unwrap_or(""));
        }
        insert
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("search.main", e.to_string()))?;

        let values = vec!["(?, 1)"; chunk.len()].join(", ");
        let mark_sql = format!(
            "INSERT INTO search_indexed (media_item_id, generation) VALUES {values}
             ON CONFLICT (media_item_id) DO NOTHING"
        );
        let mut mark = sqlx::query(&mark_sql);
        for row in chunk {
            mark = mark.bind(row.0);
        }
        mark.execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("search.main", e.to_string()))?;
    }
    Ok(())
}

/// Build the trigram index over titles only.
///
/// `core_only` exists because this is the index most likely to be cut: if it costs more
/// than R4 can spare over 2.7 million titles, 855,703 core ones still give typo
/// tolerance everywhere a user is likely to be typing.
pub async fn build_trigram(job: &mut Job<'_>, db: &Db, core_only: bool) -> Result<i64, JobError> {
    let indexed = std::sync::Arc::new(std::sync::Mutex::new(0i64));
    let sink = std::sync::Arc::clone(&indexed);
    let db = db.clone();

    job.run_step("search.trigram", move |tx, cursor| {
        let db = db.clone();
        let sink = std::sync::Arc::clone(&sink);
        Box::pin(async move {
            let after: i64 = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
            let sql = if core_only {
                "SELECT id, primary_title FROM media_items
                  WHERE id > ? AND kind <> 'episode' AND in_core = 1
                  ORDER BY id LIMIT ?"
            } else {
                "SELECT id, primary_title FROM media_items
                  WHERE id > ? AND kind <> 'episode'
                  ORDER BY id LIMIT ?"
            };
            let rows: Vec<(i64, String)> = sqlx::query_as(sql)
                .bind(after)
                .bind(BATCH)
                .fetch_all(db.pool())
                .await?;
            if rows.is_empty() {
                return Ok(Batch::finished(0));
            }

            let last = rows.last().map(|r| r.0).unwrap_or(after);
            let count = rows.len() as i64;

            for chunk in rows.chunks(200) {
                let placeholders = vec!["?"; chunk.len()].join(", ");
                let delete_sql =
                    format!("DELETE FROM search_trigram WHERE rowid IN ({placeholders})");
                let mut delete = sqlx::query(&delete_sql);
                for (id, _) in chunk {
                    delete = delete.bind(id);
                }
                delete
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| JobError::step("search.trigram", e.to_string()))?;

                let values = vec!["(?, ?)"; chunk.len()].join(", ");
                let insert_sql =
                    format!("INSERT INTO search_trigram(rowid, title) VALUES {values}");
                let mut insert = sqlx::query(&insert_sql);
                for (id, title) in chunk {
                    insert = insert.bind(id).bind(title);
                }
                insert
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| JobError::step("search.trigram", e.to_string()))?;
            }
            *sink.lock().expect("count") += count;

            Ok(if count < BATCH {
                Batch::finished(count)
            } else {
                Batch::more(last.to_string(), count)
            })
        })
    })
    .await?;

    let n = *indexed.lock().expect("count");
    Ok(n)
}

/// How much disk each index occupies.
///
/// FTS5 stores its data in shadow tables named `<table>_data`, `_idx`, `_docsize` and
/// `_config`. `dbstat` is the only way to ask SQLite what a table actually costs; a row
/// count says nothing, since the whole question is how many index entries each row
/// produces.
pub async fn index_bytes(db: &Db, table: &str) -> Result<i64, JobError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(SUM(pgsize), 0) FROM dbstat
          WHERE name = ? OR name LIKE ? || '\\_%' ESCAPE '\\'",
    )
    .bind(table)
    .bind(table)
    .fetch_one(db.pool())
    .await
    .unwrap_or(0))
}
