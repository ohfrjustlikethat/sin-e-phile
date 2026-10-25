//! Resumable jobs (`SPEC.md` Phase 4, exit criterion E2: "ingestion killed mid-run
//! resumes correctly").
//!
//! # The one idea this file is built around
//!
//! **A checkpoint must commit in the same transaction as the work it describes.**
//!
//! Everything else here follows from that. If the work commits and then the cursor
//! is written separately, a crash in between re-does the batch — which for
//! ingestion means duplicated rows. If the cursor commits first, a crash skips the
//! batch, and the catalogue is quietly missing records that nothing will ever
//! notice. Both are silent, and the second is worse.
//!
//! So `run_step` hands the caller a **transaction**, the caller does its work
//! inside it, returns the new cursor, and the runner writes the checkpoint into
//! that same transaction before committing. There is no window.
//!
//! # Why the cursor is opaque
//!
//! A byte offset into a TSV, a last-seen IMDb id, a GraphQL page token — these have
//! nothing in common. The runner stores whatever the step gives it and never looks
//! inside. Anything else would mean changing the runner for every new source.
//!
//! # Why a step, not a row, is the unit of resumption
//!
//! Per-row checkpointing would be correct and unusably slow: SQLite's cost here is
//! commits, not inserts. A step processes a batch, checkpoints once, and repeats.
//! The batch size is the step's choice, and it is the knob that trades "work lost
//! on a crash" against throughput.

use std::future::Future;
use std::pin::Pin;

use sinephile_persistence::{Db, DbError};
use sqlx::{Sqlite, Transaction};

pub type SqliteTx<'a> = Transaction<'a, Sqlite>;

/// `(name, status, items_done, items_total, cursor)` as `ingest_steps` stores it.
type ProgressRow = (String, String, i64, Option<i64>, Option<String>);

/// A future returned by a batch closure, borrowing the transaction it writes to.
pub type BatchFuture<'a> = Pin<Box<dyn Future<Output = Result<Batch, JobError>> + Send + 'a>>;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("database: {0}")]
    Db(#[from] DbError),
    #[error("database: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("step '{step}' failed: {message}")]
    Step { step: String, message: String },
}

impl JobError {
    pub fn step(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Step {
            step: name.into(),
            message: message.into(),
        }
    }
}

/// What one batch of a step did, and whether there is more.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// Where to carry on from. `None` means "start from the beginning" and is only
    /// meaningful when `finished` is true.
    pub cursor: Option<String>,
    /// Items processed in THIS batch, for progress reporting.
    pub items: i64,
    /// Total items, if the step knows. Frequently unknowable for a streamed file.
    pub total: Option<i64>,
    /// No more work in this step.
    pub finished: bool,
}

impl Batch {
    /// More work remains; carry on from `cursor` next time.
    pub fn more(cursor: impl Into<String>, items: i64) -> Self {
        Self {
            cursor: Some(cursor.into()),
            items,
            total: None,
            finished: false,
        }
    }

    /// The step is done.
    pub fn finished(items: i64) -> Self {
        Self {
            cursor: None,
            items,
            total: None,
            finished: true,
        }
    }

    /// Attach a known total, for progress reporting.
    pub fn with_total(mut self, total: i64) -> Self {
        self.total = Some(total);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

impl StepStatus {
    /// The stored spelling, so a caller can report the same names the schema uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "complete" => Self::Complete,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

/// What a step did when the runner reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// It ran, in whole or in part, and completed.
    Ran,
    /// A previous run had already finished it. Nothing was done.
    Skipped,
}

/// Progress, for a caller that wants to report it.
#[derive(Debug, Clone, PartialEq)]
pub struct StepProgress {
    pub name: String,
    pub status: StepStatus,
    pub items_done: i64,
    pub items_total: Option<i64>,
    pub cursor: Option<String>,
}

/// One run of one ingestion job.
pub struct Job<'a> {
    db: &'a Db,
    id: i64,
    name: String,
    ordinal: i64,
}

impl<'a> Job<'a> {
    /// Start a new job, or **adopt the unfinished one** if a previous run died.
    ///
    /// Adopting rather than always starting fresh is the whole feature. A job that
    /// crashed is left `running` — nothing gets the chance to mark it otherwise —
    /// so an unfinished row of the same name IS the thing to resume.
    pub async fn begin(db: &'a Db, name: &str) -> Result<Self, JobError> {
        // `failed` as well as `running`. A job that died mid-batch records the error
        // and is marked failed — and that is precisely the job to resume. Looking
        // only for `running` meant every crash started over from zero while
        // reporting success, which is the worst of both: slow AND duplicated.
        // Only a `complete` job is left alone; the next run of that is a new run.
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM ingest_jobs
             WHERE name = ? AND status IN ('running', 'failed')
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(name)
        .fetch_optional(db.pool())
        .await?;

        let id = match existing {
            Some(id) => {
                // A step left mid-flight belongs to a process that no longer exists.
                // Put it back to pending so it is retried from its last checkpoint
                // rather than being treated as someone else's work in progress.
                sqlx::query(
                    "UPDATE ingest_steps SET status = 'pending'
                     WHERE job_id = ? AND status = 'running'",
                )
                .bind(id)
                .execute(db.pool())
                .await?;
                id
            }
            None => {
                sqlx::query_scalar("INSERT INTO ingest_jobs (name) VALUES (?) RETURNING id")
                    .bind(name)
                    .fetch_one(db.pool())
                    .await?
            }
        };

        Ok(Self {
            db,
            id,
            name: name.to_string(),
            ordinal: 0,
        })
    }

    pub fn id(&self) -> i64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// True if this job adopted a previous unfinished run.
    pub async fn is_resuming(&self) -> Result<bool, JobError> {
        // A COMPLETED STEP IS NOT THE ONLY EVIDENCE OF PROGRESS, and it is not even
        // the common one. A crash lands mid-step, leaving a cursor and a count on a
        // step that never completed — exactly the case a caller most wants to be told
        // about. Counting only `complete` reported "fresh run" on every real resume.
        let progressed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ingest_steps
             WHERE job_id = ? AND (status = 'complete' OR cursor IS NOT NULL OR items_done > 0)",
        )
        .bind(self.id)
        .fetch_one(self.db.pool())
        .await?;
        Ok(progressed > 0)
    }

    /// Run one step to completion, batch by batch, resuming from its last checkpoint.
    ///
    /// `batch` is called repeatedly. It receives the open transaction and the cursor
    /// from the last committed batch, and returns what it did. **Write everything
    /// through the transaction it is given** — work written any other way is not
    /// covered by the checkpoint, and the guarantee this module exists to provide
    /// does not apply to it.
    pub async fn run_step<F>(&mut self, name: &str, mut batch: F) -> Result<StepOutcome, JobError>
    where
        F: for<'t> FnMut(&'t mut SqliteTx<'_>, Option<String>) -> BatchFuture<'t>,
    {
        self.ordinal += 1;
        let ordinal = self.ordinal;

        // READ BEFORE WRITE. The first version upserted the step to `running` and
        // then read its status back — so a completed step was marked running by the
        // very query that was about to ask whether it had completed, and every
        // resumed run re-did everything. Read first; only then claim the step.
        let row: Option<(String, Option<String>, i64)> = sqlx::query_as(
            "SELECT status, cursor, items_done FROM ingest_steps WHERE job_id = ? AND name = ?",
        )
        .bind(self.id)
        .bind(name)
        .fetch_optional(self.db.pool())
        .await?;

        let (mut cursor, mut done) = match row {
            // A completed step is not re-run. This is what makes a resumed job cheap
            // rather than merely correct.
            Some((status, _, _)) if StepStatus::parse(&status) == StepStatus::Complete => {
                return Ok(StepOutcome::Skipped);
            }
            Some((_, cursor, done)) => (cursor, done),
            None => (None, 0),
        };

        // Claim it, without disturbing the cursor a previous run left behind.
        sqlx::query(
            "INSERT INTO ingest_steps (job_id, name, ordinal, status, started_at)
             VALUES (?, ?, ?, 'running', datetime('now'))
             ON CONFLICT (job_id, name) DO UPDATE
                 SET status = 'running', updated_at = datetime('now')",
        )
        .bind(self.id)
        .bind(name)
        .bind(ordinal)
        .execute(self.db.pool())
        .await?;

        loop {
            let mut tx = self.db.pool().begin().await?;

            let result = batch(&mut tx, cursor.clone()).await;
            let outcome = match result {
                Ok(outcome) => outcome,
                Err(error) => {
                    // Roll back this batch and leave the checkpoint where it was, so
                    // a retry starts from the last batch that fully succeeded.
                    drop(tx);
                    self.fail_step(name, &error.to_string()).await?;
                    return Err(error);
                }
            };

            done += outcome.items;

            // THE CHECKPOINT, in the same transaction as the work above.
            sqlx::query(
                "UPDATE ingest_steps
                 SET cursor = ?, items_done = ?, items_total = COALESCE(?, items_total),
                     status = ?, updated_at = datetime('now')
                 WHERE job_id = ? AND name = ?",
            )
            .bind(&outcome.cursor)
            .bind(done)
            .bind(outcome.total)
            .bind(if outcome.finished {
                "complete"
            } else {
                "running"
            })
            .bind(self.id)
            .bind(name)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            if outcome.finished {
                self.touch().await?;
                return Ok(StepOutcome::Ran);
            }

            // A step that reports unfinished without moving the cursor would spin
            // forever. Caught here rather than left to a wedged ingestion at 3am.
            if outcome.cursor == cursor {
                let message = format!(
                    "step reported more work but did not advance the cursor ({:?})",
                    cursor
                );
                self.fail_step(name, &message).await?;
                return Err(JobError::step(name, message));
            }
            cursor = outcome.cursor;
        }
    }

    async fn fail_step(&self, name: &str, message: &str) -> Result<(), JobError> {
        sqlx::query(
            "UPDATE ingest_steps SET status = 'failed', updated_at = datetime('now')
             WHERE job_id = ? AND name = ?",
        )
        .bind(self.id)
        .bind(name)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            "UPDATE ingest_jobs SET status = 'failed', error = ?, updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(message)
        .bind(self.id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    async fn touch(&self) -> Result<(), JobError> {
        sqlx::query("UPDATE ingest_jobs SET updated_at = datetime('now') WHERE id = ?")
            .bind(self.id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Mark the job complete. Only call this when every step has run.
    /// Record progress for a step whose work is NOT a database write.
    ///
    /// [`Self::run_step`] commits the checkpoint in the same transaction as the work it
    /// describes, which is what makes a resumed job exactly correct. That guarantee is
    /// unavailable when the work is a file — the embedding artefact is written to disk,
    /// and no transaction spans a database and a file.
    ///
    /// So the weaker invariant is stated instead, and the caller must uphold it: **flush
    /// the file before checkpointing.** Then the cursor can only ever lag what is
    /// durably on disk, never lead it, and a resume redoes a little work rather than
    /// skipping some. Leading would silently lose vectors from the middle of an
    /// artefact, which nothing downstream could detect.
    pub async fn checkpoint(
        &self,
        step: &str,
        cursor: &str,
        items_done: i64,
    ) -> Result<(), JobError> {
        sqlx::query(
            "INSERT INTO ingest_steps (job_id, name, ordinal, status, cursor, items_done,
                                       started_at, updated_at)
             VALUES (?, ?, ?, 'running', ?, ?, datetime('now'), datetime('now'))
             ON CONFLICT (job_id, name) DO UPDATE
                 SET cursor = excluded.cursor,
                     items_done = excluded.items_done,
                     status = 'running',
                     updated_at = datetime('now')",
        )
        .bind(self.id)
        .bind(step)
        .bind(self.ordinal + 1)
        .bind(cursor)
        .bind(items_done)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Mark a step complete when its work was a file rather than a transaction.
    pub async fn complete_step(&self, step: &str, items_done: i64) -> Result<(), JobError> {
        sqlx::query(
            "UPDATE ingest_steps SET status = 'complete', items_done = ?,
                                     updated_at = datetime('now')
              WHERE job_id = ? AND name = ?",
        )
        .bind(items_done)
        .bind(self.id)
        .bind(step)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn finish(&self) -> Result<(), JobError> {
        sqlx::query(
            "UPDATE ingest_jobs
             SET status = 'complete', finished_at = datetime('now'),
                 updated_at = datetime('now'), error = NULL
             WHERE id = ?",
        )
        .bind(self.id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Every step's progress, in run order.
    pub async fn progress(&self) -> Result<Vec<StepProgress>, JobError> {
        let rows: Vec<ProgressRow> = sqlx::query_as(
            "SELECT name, status, items_done, items_total, cursor
             FROM ingest_steps WHERE job_id = ? ORDER BY ordinal",
        )
        .bind(self.id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(name, status, items_done, items_total, cursor)| StepProgress {
                    name,
                    status: StepStatus::parse(&status),
                    items_done,
                    items_total,
                    cursor,
                },
            )
            .collect())
    }

    /// Discard all state for this job's name, so the next run starts clean.
    ///
    /// Exists because "resume" and "start over" are different intentions and the
    /// user must be able to express the second one. Deleting the job cascades to
    /// its steps; it does not touch anything the ingestion wrote.
    pub async fn reset(db: &Db, name: &str) -> Result<u64, JobError> {
        let result = sqlx::query("DELETE FROM ingest_jobs WHERE name = ?")
            .bind(name)
            .execute(db.pool())
            .await?;
        Ok(result.rows_affected())
    }
}
