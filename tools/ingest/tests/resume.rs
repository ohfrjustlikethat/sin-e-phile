//! Resumability — `SPEC.md` Phase 4, exit criterion E2:
//! "Ingestion killed mid-run resumes correctly."
//!
//! "Correctly" is doing the work here, and it means three things that are easy to
//! confuse:
//!
//!   1. it carries on rather than starting over — otherwise resumption is a word;
//!   2. it does not repeat work already committed — duplicated catalogue rows;
//!   3. it does not skip work that was not committed — a catalogue quietly missing
//!      records, which nothing will ever notice.
//!
//! (2) and (3) are the same bug from opposite sides, and both come from the
//! checkpoint being written in a different transaction from the work it describes.
//! Every test below is really a test of that one property.
//!
//! A crash is simulated by returning an error from a batch, which leaves exactly the
//! state a killed process leaves: whatever committed, committed; the job row still
//! says `running` because nothing got the chance to say otherwise.

use sinephile_ingest::{Batch, Job, JobError, StepOutcome, StepStatus};
use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::Db;

/// A step that inserts films named `item-<n>`, `batch` at a time, and can be told to
/// die at a given item. The cursor is the last index written.
async fn insert_films(
    db: &Db,
    job_name: &str,
    step_name: &str,
    total: i64,
    per_batch: i64,
    die_at: Option<i64>,
) -> Result<StepOutcome, JobError> {
    let mut job = Job::begin(db, job_name).await?;
    job.run_step(step_name, move |tx, cursor| {
        Box::pin(async move {
            let start: i64 = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
            let end = (start + per_batch).min(total);

            for n in start..end {
                if Some(n) == die_at {
                    // Everything committed before this batch stays; this batch rolls
                    // back with the transaction. Exactly what a kill -9 leaves.
                    return Err(JobError::step("insert_films", format!("killed at {n}")));
                }
                sqlx::query("INSERT INTO media_items (kind, primary_title) VALUES ('film', ?)")
                    .bind(format!("item-{n}"))
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| JobError::step("insert_films", e.to_string()))?;
            }

            Ok(if end >= total {
                Batch::finished(end - start).with_total(total)
            } else {
                Batch::more(end.to_string(), end - start).with_total(total)
            })
        })
    })
    .await
}

async fn count(db: &Db) -> i64 {
    MediaRepository::new(db).count().await.expect("count")
}

#[tokio::test]
async fn a_clean_run_processes_everything_once() {
    let db = Db::in_memory().await.expect("open");
    let outcome = insert_films(&db, "imdb", "basics", 50, 10, None)
        .await
        .expect("run");
    assert_eq!(outcome, StepOutcome::Ran);
    assert_eq!(count(&db).await, 50);
}

#[tokio::test]
async fn a_killed_run_resumes_and_repeats_nothing() {
    // The E2 test.
    let db = Db::in_memory().await.expect("open");

    // Die at item 25: batches 0-9, 10-19 committed; the batch containing 25 rolls back.
    let result = insert_films(&db, "imdb", "basics", 50, 10, Some(25)).await;
    assert!(result.is_err(), "the run was supposed to die");

    let after_crash = count(&db).await;
    assert_eq!(
        after_crash, 20,
        "two batches committed and the third rolled back whole"
    );

    // Restart, as the user re-running the command would.
    let outcome = insert_films(&db, "imdb", "basics", 50, 10, None)
        .await
        .expect("resume");
    assert_eq!(outcome, StepOutcome::Ran);

    assert_eq!(count(&db).await, 50, "every item exists");

    // The real assertion: EXACTLY once each. A cursor written outside the work's
    // transaction shows up here as 60 rows, or as 50 rows missing items 20-24.
    let titles: Vec<String> =
        sqlx::query_scalar("SELECT primary_title FROM media_items ORDER BY id")
            .fetch_all(db.pool())
            .await
            .expect("titles");
    let expected: Vec<String> = (0..50).map(|n| format!("item-{n}")).collect();
    assert_eq!(titles, expected, "no duplicates, no gaps, in order");
}

#[tokio::test]
async fn resuming_reports_that_it_is_resuming() {
    let db = Db::in_memory().await.expect("open");
    let _ = insert_films(&db, "imdb", "basics", 30, 10, Some(15)).await;

    let job = Job::begin(&db, "imdb").await.expect("adopt");
    assert!(
        !job.is_resuming().await.expect("query"),
        "no step finished before the crash, so there is nothing to resume past"
    );

    // Now finish it, then start a third run.
    insert_films(&db, "imdb", "basics", 30, 10, None)
        .await
        .expect("finish");
    let job = Job::begin(&db, "imdb").await.expect("adopt again");
    assert!(
        job.is_resuming().await.expect("query"),
        "a completed step exists"
    );
}

#[tokio::test]
async fn a_completed_step_is_skipped_not_redone() {
    // What makes a resumed job cheap rather than merely correct.
    let db = Db::in_memory().await.expect("open");
    insert_films(&db, "imdb", "basics", 20, 10, None)
        .await
        .expect("first");
    assert_eq!(count(&db).await, 20);

    let outcome = insert_films(&db, "imdb", "basics", 20, 10, None)
        .await
        .expect("second");
    assert_eq!(outcome, StepOutcome::Skipped);
    assert_eq!(count(&db).await, 20, "the step did not run again");
}

#[tokio::test]
async fn a_later_step_resumes_without_redoing_an_earlier_one() {
    let db = Db::in_memory().await.expect("open");
    let mut job = Job::begin(&db, "imdb").await.expect("job");

    // Step one completes.
    job.run_step("first", |tx, _| {
        Box::pin(async move {
            sqlx::query("INSERT INTO media_items (kind, primary_title) VALUES ('film', 'one')")
                .execute(&mut **tx)
                .await
                .map_err(|e| JobError::step("first", e.to_string()))?;
            Ok(Batch::finished(1))
        })
    })
    .await
    .expect("first step");

    // Step two dies.
    let died = job
        .run_step("second", |_tx, _| {
            Box::pin(async move { Err(JobError::step("second", "killed")) })
        })
        .await;
    assert!(died.is_err());
    assert_eq!(count(&db).await, 1, "step one's work survived");

    // A fresh process adopts the job. Step one must be skipped.
    let mut resumed = Job::begin(&db, "imdb").await.expect("adopt");
    assert_eq!(
        resumed
            .run_step("first", |_tx, _| Box::pin(async move {
                panic!("a completed step was re-entered")
            }))
            .await
            .expect("skip"),
        StepOutcome::Skipped
    );

    resumed
        .run_step("second", |tx, _| {
            Box::pin(async move {
                sqlx::query("INSERT INTO media_items (kind, primary_title) VALUES ('film', 'two')")
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| JobError::step("second", e.to_string()))?;
                Ok(Batch::finished(1))
            })
        })
        .await
        .expect("second step");

    assert_eq!(count(&db).await, 2);
}

#[tokio::test]
async fn a_step_that_does_not_advance_its_cursor_is_stopped() {
    // Otherwise it spins forever, at 3am, having written nothing.
    let db = Db::in_memory().await.expect("open");
    let mut job = Job::begin(&db, "imdb").await.expect("job");

    let result = job
        .run_step("stuck", |_tx, _cursor| {
            Box::pin(async move { Ok(Batch::more("always-the-same", 1)) })
        })
        .await;

    let message = result.expect_err("should have been stopped").to_string();
    assert!(
        message.contains("did not advance the cursor"),
        "unhelpful error: {message}"
    );
}

#[tokio::test]
async fn a_failed_step_records_why() {
    let db = Db::in_memory().await.expect("open");
    let mut job = Job::begin(&db, "imdb").await.expect("job");

    let _ = job
        .run_step("doomed", |_tx, _| {
            Box::pin(async move { Err(JobError::step("doomed", "the dataset moved")) })
        })
        .await;

    let progress = job.progress().await.expect("progress");
    let step = progress
        .iter()
        .find(|s| s.name == "doomed")
        .expect("recorded");
    assert_eq!(step.status, StepStatus::Failed);

    let error: Option<String> = sqlx::query_scalar("SELECT error FROM ingest_jobs WHERE id = ?")
        .bind(job.id())
        .fetch_one(db.pool())
        .await
        .expect("query");
    assert!(
        error.unwrap_or_default().contains("the dataset moved"),
        "a resumed run must be able to say what it is resuming from"
    );
}

#[tokio::test]
async fn progress_survives_a_restart() {
    let db = Db::in_memory().await.expect("open");
    let _ = insert_films(&db, "imdb", "basics", 100, 10, Some(45)).await;

    let job = Job::begin(&db, "imdb").await.expect("adopt");
    let progress = job.progress().await.expect("progress");
    let step = &progress[0];

    assert_eq!(step.items_done, 40, "four batches committed");
    assert_eq!(step.items_total, Some(100));
    assert_eq!(
        step.cursor.as_deref(),
        Some("40"),
        "the checkpoint is where the work stopped"
    );
}

#[tokio::test]
async fn reset_discards_progress_but_not_the_data() {
    // "Resume" and "start over" are different intentions and the user must be able
    // to express the second.
    let db = Db::in_memory().await.expect("open");
    insert_films(&db, "imdb", "basics", 10, 5, None)
        .await
        .expect("run");

    let removed = Job::reset(&db, "imdb").await.expect("reset");
    assert_eq!(removed, 1);

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_jobs")
        .fetch_one(db.pool())
        .await
        .expect("count jobs");
    let steps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_steps")
        .fetch_one(db.pool())
        .await
        .expect("count steps");
    assert_eq!((jobs, steps), (0, 0), "the job and its steps cascaded away");
    assert_eq!(count(&db).await, 10, "what was ingested was not touched");
}

#[tokio::test]
async fn finish_marks_the_job_complete_and_a_later_run_starts_fresh() {
    let db = Db::in_memory().await.expect("open");
    let job = Job::begin(&db, "imdb").await.expect("job");
    let first_id = job.id();
    job.finish().await.expect("finish");

    // A completed job is not adopted — the next run is a new run.
    let second = Job::begin(&db, "imdb").await.expect("second");
    assert_ne!(second.id(), first_id, "a finished job is not resumed");
}
