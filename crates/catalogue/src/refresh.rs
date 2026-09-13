//! Incremental catalogue refresh — ADR-0030 layer 1.
//!
//! IMDb republishes daily and we ingest once, so the catalogue is a snapshot that
//! ages. A film released after ingestion is not merely stale, it is **absent** — it
//! cannot be searched for at all.
//!
//! # ADR-0030's mechanism does not work, and the file says so
//!
//! The ADR specified `seek_past(the highest id we already hold)` and inserting the
//! tail, on the premise that "IMDb's files are sorted by id and new titles get higher
//! ids". **`title.basics` is sorted LEXICOGRAPHICALLY, not numerically**, and the two
//! stopped being the same thing the moment IMDb issued its eight-millionth id:
//!
//! ```text
//! sorted lexicographically: true
//! sorted numerically      : false
//! first numeric decrease at row 967,458:  tt10001008 -> tt1000101
//! last id in the file:                    tt9916880   (not the largest)
//! ```
//!
//! So a new `tt45000000` lands in the middle of the file, in the `tt4…` block — not at
//! the end. Seeking past our numeric maximum walks straight into the `tt5…`–`tt9…`
//! rows we already hold, and the first insert dies on a UNIQUE violation. It did.
//!
//! # What is done instead
//!
//! Scan the whole file and **skip by numeric id**. The saving ADR-0030 was actually
//! after is still there: the expensive half is inserting 2.7 million rows, not reading
//! them, and nothing already held is inserted or even parsed into a row. What is lost
//! is the seek, which was never the costly part.
//!
//! `seek_past` remains exactly where it was for crash resumption, where it is still
//! correct: a job cursor is the last id processed *in file order*.
//!
//! # What it deliberately does not do
//!
//! It does not catch a *revision* to an existing title: a corrected year, a runtime
//! that changed. `title.ratings` is only 8 MB so ratings are re-applied in full, and
//! everything else is accepted as stale until a full re-ingest. Saying so is the point
//! — an "incremental refresh" that quietly missed corrections while implying otherwise
//! would be worse than one with a documented limit.

use std::collections::HashMap;

use sinephile_persistence::Db;

use crate::job::{Batch, Job, JobError, SqliteTx};

/// Rows per transaction when re-applying ratings.
const BATCH: usize = 20_000;

/// The highest IMDb title id the catalogue already holds, as a number.
///
/// A NUMBER, not a `tt` string, and that is the whole correction: the id is compared
/// arithmetically against every row, never used as a position to seek to.
///
/// **Episodes are excluded deliberately.** They carry IMDb ids from the same space and
/// are loaded only for series in scope, so the highest stored id is easily an episode
/// far above the highest *title* the scan considered. Including them would raise the
/// floor past titles a refresh exists to find.
///
/// `None` when the catalogue is empty, in which case a refresh is a first ingestion.
pub async fn watermark(db: &Db) -> Result<Option<u32>, JobError> {
    let highest: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(CAST(SUBSTR(e.external_id, 3) AS INTEGER))
           FROM external_ids e
           JOIN media_items m ON m.id = e.media_item_id
          WHERE e.source = 'imdb' AND m.kind <> 'episode'",
    )
    .fetch_one(db.pool())
    .await?;
    Ok(highest.map(|n| n as u32))
}

/// How many titles the catalogue holds, for reporting what a refresh added.
pub async fn title_count(db: &Db) -> Result<i64, JobError> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE kind <> 'episode'")
            .fetch_one(db.pool())
            .await?,
    )
}

/// Re-apply every rating and vote count.
///
/// Ratings move constantly and a stale rating is a wrong sort order, not merely an old
/// number. `title.ratings` is 8 MB, so the download is trivial and the only real cost
/// is the UPDATE — which is why this is a separate, resumable step rather than folded
/// into the title load.
pub async fn ratings(
    job: &mut Job<'_>,
    votes: std::sync::Arc<HashMap<u32, i64>>,
    averages: std::sync::Arc<HashMap<u32, i64>>,
) -> Result<i64, JobError> {
    // A stable order, so the cursor means the same thing across runs. A HashMap's
    // iteration order does not survive a restart, and a cursor into an unstable order
    // is a cursor that silently skips rows.
    let mut ordered: Vec<u32> = votes.keys().copied().collect();
    ordered.sort_unstable();
    let ordered = std::sync::Arc::new(ordered);
    let updated = std::sync::Arc::new(std::sync::Mutex::new(0i64));
    let sink = std::sync::Arc::clone(&updated);

    job.run_step("title.ratings.refresh", move |tx, cursor| {
        let ordered = std::sync::Arc::clone(&ordered);
        let votes = std::sync::Arc::clone(&votes);
        let averages = std::sync::Arc::clone(&averages);
        let sink = std::sync::Arc::clone(&sink);
        Box::pin(async move {
            let from: usize = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
            if from >= ordered.len() {
                return Ok(Batch::finished(0));
            }
            let end = (from + BATCH).min(ordered.len());
            let chunk = &ordered[from..end];

            let n = apply(tx, chunk, &votes, &averages).await?;
            *sink.lock().expect("count lock") += n;

            Ok(if end >= ordered.len() {
                Batch::finished(chunk.len() as i64)
            } else {
                Batch::more(end.to_string(), chunk.len() as i64)
            })
        })
    })
    .await?;

    let n = *updated.lock().expect("count lock");
    Ok(n)
}

async fn apply(
    tx: &mut SqliteTx<'_>,
    chunk: &[u32],
    votes: &HashMap<u32, i64>,
    averages: &HashMap<u32, i64>,
) -> Result<i64, JobError> {
    let mut updated = 0i64;
    for group in chunk.chunks(200) {
        // Joined through external_ids rather than updating by id: a title we never
        // ingested (wrong type, adult, out of scope) simply matches nothing, which is
        // what should happen. IMDb rates far more titles than we keep.
        let values: Vec<String> = group
            .iter()
            .map(|id| {
                format!(
                    "('tt{id:07}', {}, {})",
                    averages.get(id).copied().unwrap_or(0),
                    votes.get(id).copied().unwrap_or(0)
                )
            })
            .collect();

        let sql = format!(
            "WITH incoming(tconst, rating, votes) AS (VALUES {})
             UPDATE media_items
                SET rating = (SELECT rating FROM incoming
                               JOIN external_ids e ON e.external_id = incoming.tconst
                              WHERE e.source = 'imdb' AND e.media_item_id = media_items.id),
                    rating_votes = (SELECT votes FROM incoming
                                     JOIN external_ids e ON e.external_id = incoming.tconst
                                    WHERE e.source = 'imdb' AND e.media_item_id = media_items.id),
                    updated_at = datetime('now')
              WHERE id IN (SELECT e.media_item_id FROM external_ids e
                            JOIN incoming ON incoming.tconst = e.external_id
                           WHERE e.source = 'imdb')",
            values.join(", ")
        );

        updated += sqlx::query(&sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.ratings.refresh", e.to_string()))?
            .rows_affected() as i64;
    }
    Ok(updated)
}
