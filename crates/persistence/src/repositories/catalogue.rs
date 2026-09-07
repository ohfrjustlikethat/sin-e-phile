//! Is the catalogue ready, and if not, what can be searched right now?
//!
//! `SPEC.md` §15 Phase 4: *"a first-run flow that either ships a prebuilt index or
//! builds it in the background with a good progress UI (the app is usable during the
//! build, searching what's ingested so far)"*.
//!
//! Two things follow, and the second is the one that is easy to skip.
//!
//! 1. **Progress has to be readable without the ingester running in-process.** It is
//!    in the database already — migration 0005 put it there deliberately — so this is
//!    a repository, and the application never depends on `tools/ingest` to find out
//!    how far along it is.
//!
//! 2. **A partial catalogue must never present itself as a complete one.** "No results"
//!    means something entirely different at 3% ingested than at 100%, and a search
//!    screen that cannot tell the difference will confidently tell a user their film
//!    does not exist. Every variant below carries the count that is searchable *now*,
//!    so a caller cannot render results without having been handed the context.

use crate::db::{Db, DbError};

/// A `running` job whose progress has not moved for this long is reported as stalled
/// rather than as building.
///
/// Without it, a crashed ingestion leaves `status = 'running'` in the table forever and
/// the app cheerfully shows a progress bar for a process that no longer exists. The
/// runner adopts and resumes such a job on the next launch (`Job::begin`), so this is
/// only about not lying in the meantime — which is why it is generous rather than
/// tight. A slow step on a slow disk must not be declared dead.
pub const STALE_AFTER_SECONDS: i64 = 300;

/// What one step of an ingestion is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepProgress {
    pub name: String,
    pub items_done: i64,
    /// `None` for a streamed file, which cannot know its own length. A caller must
    /// render an indeterminate state rather than inventing a denominator — migration
    /// 0005 says as much where the column is defined.
    pub items_total: Option<i64>,
}

impl StepProgress {
    pub fn percent(&self) -> Option<f64> {
        match self.items_total {
            Some(total) if total > 0 => Some((self.items_done as f64 / total as f64) * 100.0),
            _ => None,
        }
    }
}

/// Whether the catalogue can be searched, and how much of it exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// Nothing has been ingested. The app runs; there is simply nothing to find yet.
    Empty,
    /// An ingestion is running. `titles` is what is searchable at this moment.
    Building {
        titles: i64,
        job: String,
        step: Option<StepProgress>,
    },
    /// An ingestion stopped without finishing — it failed, or its process died and the
    /// row was left saying `running`. Distinguished from `Building` because the honest
    /// thing to show is "resume", not a progress bar that will never move.
    Interrupted {
        titles: i64,
        job: String,
        step: Option<String>,
        error: Option<String>,
    },
    /// An ingestion has completed.
    Ready { titles: i64 },
}

impl Readiness {
    /// How many titles can be searched right now.
    pub fn searchable_titles(&self) -> i64 {
        match self {
            Readiness::Empty => 0,
            Readiness::Building { titles, .. }
            | Readiness::Interrupted { titles, .. }
            | Readiness::Ready { titles } => *titles,
        }
    }

    /// Is the catalogue still incomplete?
    ///
    /// **A search screen must consult this before saying "no results".** At 3%
    /// ingested that phrase is a lie; at 100% it is the answer.
    pub fn is_partial(&self) -> bool {
        !matches!(self, Readiness::Ready { .. })
    }

    /// Can the user search at all yet?
    pub fn is_searchable(&self) -> bool {
        self.searchable_titles() > 0
    }
}

/// Which titles the embedding artefact covers, written once, in the crate both the
/// producer and the application depend on.
///
/// The artefact is **positional**: vector `n` belongs to the nth core title in id
/// order, and the file records no ids of its own. Everything that walks the core tier
/// — the producer's count, its batch reader, its resume cursor, and
/// [`CatalogueRepository::core_ids`] — is part of one definition between them. A
/// predicate that drifted in any of them would not fail; it would silently return a
/// different film for every position after the drift.
pub const CORE_TIER: &str = "in_core = 1 AND kind <> 'episode'";

/// How much descriptive text an item needs before it belongs in the vector index.
///
/// # Measured, and it is not about truncation
///
/// *Fish Hooky*'s entire synopsis is "…a 1933 Our Gang short comedy film directed by
/// Robert F. McGowan. It was the 120th Our Gang short to be released" — 126 characters
/// of production trivia. Against the query "a grieving janitor becomes guardian of his
/// teenage nephew in a Massachusetts fishing town" it scores **0.4194**, while
/// *Manchester by the Sea*, whose synopsis describes that exact plot, scores **0.1944**
/// and only reaches 0.2689 given its whole text.
///
/// Short generic documents sit near the middle of the embedding space and are therefore
/// close to everything — the hubness problem. Feeding them more text does not help,
/// because they have no more text; the fix is to keep them out of the index, where the
/// exact-title and BM25 tiers already cover them properly.
///
/// 300 characters keeps 119,874 of 189,470 items. It is a threshold and it is arbitrary
/// in the way thresholds are, but it is chosen from the distribution rather than from
/// taste, and `eval search --query` is how a different value would be argued for.
pub const MIN_SYNOPSIS: usize = 300;

pub struct CatalogueRepository<'a> {
    db: &'a Db,
}

impl<'a> CatalogueRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Every core title's id, in artefact order.
    ///
    /// This is the mapping the artefact does not carry: position → `media_items.id`.
    /// Deriving it means re-running the ordering the producer walked, which is sound
    /// only because [`CORE_TIER`] is shared and `ORDER BY id` is total.
    ///
    /// The consumer checks this list's length against the artefact header before
    /// trusting any of it (`sinephile_vector_index::VectorIndex::build`) — that check
    /// is what catches a catalogue which has grown since the artefact was built.
    pub async fn core_ids(&self) -> Result<Vec<i64>, DbError> {
        Ok(sqlx::query_scalar(&format!(
            "SELECT id FROM media_items WHERE {CORE_TIER} ORDER BY id"
        ))
        .fetch_all(self.db.pool())
        .await?)
    }

    /// Core ids in artefact order, `None` where the item has no descriptive text.
    ///
    /// # Why an item without a synopsis must not be in the vector index
    ///
    /// Measured 2026-09-07, and it is the opposite of what I expected. After loading
    /// 233,114 Wikipedia extracts, "films about grief that aren't depressing" STILL
    /// returned ten obscure films titled *Grief* — every one of them with no synopsis.
    ///
    /// A document that is only `Grief (1921), drama short film` is almost entirely the
    /// word "grief", so its vector sits nearly on top of any query containing that word.
    /// *Manchester by the Sea*, with four hundred characters describing a man returning
    /// home after a death, spreads its vector across many concepts and scores lower.
    /// **Sparse documents are louder than rich ones**, and they drown out exactly the
    /// items the enrichment was for.
    ///
    /// So the vector half indexes only what has something to say. Nothing is lost: an
    /// item with no description is still found by exact title and by BM25, which handle
    /// titles better and more predictably than an embedding ever will.
    pub async fn core_ids_for_vectors(&self) -> Result<Vec<Option<i64>>, DbError> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(&format!(
            "SELECT id,
                    CASE WHEN synopsis IS NOT NULL
                          AND length(trim(synopsis)) >= {MIN_SYNOPSIS}
                         THEN 1 ELSE 0 END
               FROM media_items WHERE {CORE_TIER} ORDER BY id"
        ))
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, has_text)| (has_text == 1).then_some(id))
            .collect())
    }

    /// Titles a search could return. Episodes are excluded: they are not what a first
    /// search is for, and counting them would overstate readiness by half a million.
    pub async fn searchable_titles(&self) -> Result<i64, DbError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE kind <> 'episode'")
                .fetch_one(self.db.pool())
                .await?,
        )
    }

    /// The state of the catalogue, for the first-run screen and for search.
    pub async fn readiness(&self) -> Result<Readiness, DbError> {
        let titles = self.searchable_titles().await?;

        // The most recent job of any name. Ingestion is a sequence of them — imdb,
        // credits, akas, anilist — and what the user cares about is whether anything
        // is happening now, not which dataset it happens to be.
        let job: Option<(i64, String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT id, name, status, error,
                    CAST(strftime('%s', 'now') - strftime('%s', updated_at) AS INTEGER)
               FROM ingest_jobs
              ORDER BY started_at DESC, id DESC
              LIMIT 1",
        )
        .fetch_optional(self.db.pool())
        .await?;

        let Some((id, name, status, error, idle_seconds)) = job else {
            // No job has ever run. Either a first launch, or a catalogue that arrived
            // prebuilt — and a prebuilt catalogue with titles in it is ready.
            return Ok(if titles > 0 {
                Readiness::Ready { titles }
            } else {
                Readiness::Empty
            });
        };

        match status.as_str() {
            "complete" => Ok(Readiness::Ready { titles }),
            "running" if idle_seconds <= STALE_AFTER_SECONDS => Ok(Readiness::Building {
                titles,
                job: name,
                step: self.current_step(id).await?,
            }),
            // Either `failed`, or `running` with nothing moving for five minutes.
            _ => Ok(Readiness::Interrupted {
                titles,
                job: name,
                step: self.current_step(id).await?.map(|s| s.name),
                error,
            }),
        }
    }

    /// The step a job is on: the furthest one that is not finished.
    async fn current_step(&self, job_id: i64) -> Result<Option<StepProgress>, DbError> {
        let row: Option<(String, i64, Option<i64>)> = sqlx::query_as(
            "SELECT name, items_done, items_total
               FROM ingest_steps
              WHERE job_id = ? AND status <> 'complete'
              ORDER BY ordinal
              LIMIT 1",
        )
        .bind(job_id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row.map(|(name, items_done, items_total)| StepProgress {
            name,
            items_done,
            items_total,
        }))
    }

    /// Every step of the newest job, for a detailed progress panel.
    pub async fn steps(&self) -> Result<Vec<StepProgress>, DbError> {
        Ok(sqlx::query_as::<_, (String, i64, Option<i64>)>(
            "SELECT s.name, s.items_done, s.items_total
               FROM ingest_steps s
               JOIN ingest_jobs j ON j.id = s.job_id
              WHERE j.id = (SELECT id FROM ingest_jobs ORDER BY started_at DESC, id DESC LIMIT 1)
              ORDER BY s.ordinal",
        )
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|(name, items_done, items_total)| StepProgress {
            name,
            items_done,
            items_total,
        })
        .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_total_yields_no_percentage_rather_than_a_made_up_one() {
        // Migration 0005: items_total is NULL for a streamed file and stays NULL.
        let streaming = StepProgress {
            name: "title.basics".into(),
            items_done: 1_000_000,
            items_total: None,
        };
        assert_eq!(streaming.percent(), None);

        let known = StepProgress {
            name: "title.akas".into(),
            items_done: 25,
            items_total: Some(100),
        };
        assert_eq!(known.percent(), Some(25.0));

        // A zero total is a denominator waiting to divide by zero.
        let zero = StepProgress {
            name: "empty".into(),
            items_done: 0,
            items_total: Some(0),
        };
        assert_eq!(zero.percent(), None);
    }

    #[test]
    fn only_a_completed_ingestion_is_not_partial() {
        // The check a search screen makes before it dares say "no results".
        assert!(Readiness::Empty.is_partial());
        assert!(Readiness::Building {
            titles: 10,
            job: "imdb".into(),
            step: None
        }
        .is_partial());
        assert!(Readiness::Interrupted {
            titles: 10,
            job: "imdb".into(),
            step: None,
            error: None
        }
        .is_partial());
        assert!(!Readiness::Ready { titles: 10 }.is_partial());
    }

    #[test]
    fn searchability_is_about_titles_not_about_completeness() {
        // A half-built catalogue is still worth searching — that is the entire point
        // of the requirement.
        assert!(!Readiness::Empty.is_searchable());
        assert!(Readiness::Building {
            titles: 1,
            job: "imdb".into(),
            step: None
        }
        .is_searchable());
        assert!(!Readiness::Building {
            titles: 0,
            job: "imdb".into(),
            step: None
        }
        .is_searchable());
    }
}

/// The Wikipedia mapping and the text it produces (ADR-0033, migration 0014).
///
/// Separate from [`CatalogueRepository`] because it is a loader's surface rather than
/// the application's: nothing in the running app reads these, and the only consumer is
/// `ingest wikipedia` plus the document builder that later reads `media_items.synopsis`.
pub struct WikipediaRepository<'a> {
    db: &'a Db,
}

/// One article waiting to be fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingArticle {
    pub media_item_id: i64,
    pub article: String,
}

impl<'a> WikipediaRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Record that an IMDb id has an article, resolving it to a catalogue item.
    ///
    /// Returns whether anything was stored. A mapping for a title we do not hold is not
    /// an error — Wikidata knows about 509,464 IMDb ids and this catalogue holds a
    /// subset — it is simply nothing to do.
    ///
    /// `ON CONFLICT DO UPDATE` on the article but **not** on `fetched_at`: re-running
    /// the mapping after an article was moved should point at the new title and leave
    /// the text already fetched in place, rather than silently queueing a re-fetch of
    /// everything.
    pub async fn map(&self, imdb_id: &str, article: &str) -> Result<bool, DbError> {
        let media_item_id: Option<i64> = sqlx::query_scalar(
            "SELECT media_item_id FROM external_ids WHERE source = 'imdb' AND external_id = ?",
        )
        .bind(imdb_id)
        .fetch_optional(self.db.pool())
        .await?;

        let Some(media_item_id) = media_item_id else {
            return Ok(false);
        };

        sqlx::query(
            "INSERT INTO wikipedia_article (media_item_id, article) VALUES (?, ?)
             ON CONFLICT (media_item_id) DO UPDATE SET article = excluded.article",
        )
        .bind(media_item_id)
        .bind(article)
        .execute(self.db.pool())
        .await?;
        Ok(true)
    }

    /// Articles mapped but not yet fetched, **most-voted first**.
    ///
    /// The order is the scoping decision: a full run is hours, so a bounded one must
    /// cover what people actually search for rather than an arbitrary slice. Same
    /// reasoning as `ingest anime --pages`.
    pub async fn pending(&self, limit: i64) -> Result<Vec<PendingArticle>, DbError> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT w.media_item_id, w.article
               FROM wikipedia_article w
               JOIN media_items m ON m.id = w.media_item_id
              WHERE w.fetched_at IS NULL
              ORDER BY m.rating_votes DESC NULLS LAST
              LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(media_item_id, article)| PendingArticle {
                media_item_id,
                article,
            })
            .collect())
    }

    /// Store an extract as the item's synopsis and mark the article fetched.
    ///
    /// Both writes in one transaction: a synopsis without a `fetched_at` would be
    /// re-fetched forever, and a `fetched_at` without a synopsis would be lost forever.
    pub async fn store(&self, media_item_id: i64, text: &str) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;

        sqlx::query("UPDATE media_items SET synopsis = ? WHERE id = ?")
            .bind(text)
            .bind(media_item_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE wikipedia_article SET fetched_at = datetime('now') WHERE media_item_id = ?",
        )
        .bind(media_item_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Mark an article fetched **without** a synopsis.
    ///
    /// A title Wikipedia reports as missing, or whose lead is empty, must not be asked
    /// for again on every subsequent run — an unanswerable request is still a request,
    /// and there are enough of them to matter over a catalogue this size.
    pub async fn mark_empty(&self, media_item_id: i64) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE wikipedia_article SET fetched_at = datetime('now') WHERE media_item_id = ?",
        )
        .bind(media_item_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// (mapped, fetched, with a synopsis) — for reporting progress and for the eval.
    pub async fn counts(&self) -> Result<(i64, i64, i64), DbError> {
        let mapped: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wikipedia_article")
            .fetch_one(self.db.pool())
            .await?;
        let fetched: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wikipedia_article WHERE fetched_at IS NOT NULL",
        )
        .fetch_one(self.db.pool())
        .await?;
        let with_text: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM media_items
              WHERE synopsis IS NOT NULL AND length(trim(synopsis)) > 0",
        )
        .fetch_one(self.db.pool())
        .await?;
        Ok((mapped, fetched, with_text))
    }
}
