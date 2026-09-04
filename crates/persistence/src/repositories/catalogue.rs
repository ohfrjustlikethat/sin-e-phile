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

pub struct CatalogueRepository<'a> {
    db: &'a Db,
}

impl<'a> CatalogueRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
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
