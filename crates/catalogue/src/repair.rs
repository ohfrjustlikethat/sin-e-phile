//! One-off repairs of data a fixed bug already wrote.
//!
//! Fixing the code that produces bad rows does not fix the rows. `ingest akas` cannot
//! do it either: its inserts are `ON CONFLICT DO NOTHING` against
//! `(media_item_id, variant, title)`, so a re-run would ADD the corrected row and
//! leave the wrong one beside it — the catalogue would then hold the same title under
//! two variants and be worse than before.
//!
//! So a repair is its own step, scoped to exactly the rows the bug could have
//! produced, and re-runnable.

use sinephile_persistence::Db;

use crate::akas::variant;
use crate::job::{Batch, Job, JobError};

const BATCH: i64 = 20_000;

/// Recompute `titles.variant` for rows the region-over-language bug mislabelled.
///
/// THE BUG. `variant()` consulted the release region before checking whether the
/// language was already known, so IMDb's Spanish title of *Spirited Away* (language
/// `es`, region `US`) and its French one (language `fr`, region `CA`) were both
/// recorded as the film's ENGLISH title. Measured over the real catalogue: 41,193
/// rows, 5.5% of every `english` title, almost all of them `fr`/CA and `es`/US.
///
/// The scope is deliberately narrow — only rows claiming to be English while carrying
/// a language that is not English. That set is exactly what the bug could produce, so
/// the repair cannot touch a row that was right.
pub async fn english_variants(job: &mut Job<'_>) -> Result<(), JobError> {
    job.run_step("repair.english-variants", move |tx, cursor| {
        Box::pin(async move {
            let after: i64 = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);

            let rows: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT id, title, language, region FROM titles
                  WHERE variant = 'english' AND language IS NOT NULL AND language <> 'en'
                    AND id > ?
                  ORDER BY id LIMIT ?",
            )
            .bind(after)
            .bind(BATCH)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| JobError::step("repair.english-variants", e.to_string()))?;

            if rows.is_empty() {
                return Ok(Batch::finished(0));
            }

            let last_id = rows.last().map(|(id, ..)| *id).unwrap_or(after);
            let count = rows.len() as i64;

            for (id, title, language, region) in &rows {
                // `is_original` is false by construction: an original title is stored
                // as variant 'original' and is therefore not in this set.
                let corrected = variant(false, language.as_deref(), region.as_deref(), title);

                // UPDATE OR REPLACE, because the unique index is
                // (media_item_id, variant, title) and the corrected variant may
                // already exist for this item — the same title having been loaded
                // once correctly and once through the bug. REPLACE removes the
                // duplicate and keeps one row, which is the intended end state.
                sqlx::query("UPDATE OR REPLACE titles SET variant = ? WHERE id = ?")
                    .bind(corrected)
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| JobError::step("repair.english-variants", e.to_string()))?;
            }

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

/// How many rows still claim to be English while carrying another language.
pub async fn mislabelled_english(db: &Db) -> Result<i64, JobError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM titles
          WHERE variant = 'english' AND language IS NOT NULL AND language <> 'en'",
    )
    .fetch_one(db.pool())
    .await?)
}
