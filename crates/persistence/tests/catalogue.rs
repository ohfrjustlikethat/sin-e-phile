//! Catalogue readiness, against a migrated database.
//!
//! Every SQL statement in `repositories/catalogue.rs` runs here (ADR-0026).

use sinephile_persistence::repositories::{CatalogueRepository, Readiness};
use sinephile_persistence::{Db, MediaKind, NewMediaItem};

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    (dir, db)
}

async fn add_titles(db: &Db, count: usize) {
    let media = sinephile_persistence::repositories::MediaRepository::new(db);
    for i in 0..count {
        media
            .insert(&NewMediaItem::film(format!("Film {i}"), 1950 + i as i64))
            .await
            .expect("insert");
    }
}

/// A job row, with `updated_at` pushed into the past by `idle_seconds`.
async fn job(db: &Db, name: &str, status: &str, idle_seconds: i64) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO ingest_jobs (name, status, started_at, updated_at)
         VALUES (?, ?, datetime('now', ?), datetime('now', ?)) RETURNING id",
    )
    .bind(name)
    .bind(status)
    .bind(format!("-{idle_seconds} seconds"))
    .bind(format!("-{idle_seconds} seconds"))
    .fetch_one(db.pool())
    .await
    .expect("job");
    id
}

async fn step(
    db: &Db,
    job_id: i64,
    name: &str,
    ordinal: i64,
    status: &str,
    done: i64,
    total: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO ingest_steps (job_id, name, ordinal, status, items_done, items_total)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(job_id)
    .bind(name)
    .bind(ordinal)
    .bind(status)
    .bind(done)
    .bind(total)
    .execute(db.pool())
    .await
    .expect("step");
}

#[tokio::test]
async fn a_fresh_install_is_empty() {
    let (_dir, db) = db().await;
    let catalogue = CatalogueRepository::new(&db);

    assert_eq!(
        catalogue.readiness().await.expect("readiness"),
        Readiness::Empty
    );
    assert!(!Readiness::Empty.is_searchable());
    assert!(Readiness::Empty.is_partial());
}

#[tokio::test]
async fn a_prebuilt_catalogue_with_no_job_is_ready() {
    // SPEC.md Phase 4 allows shipping a prebuilt index. Nothing ingested it here, so
    // there is no job row to read — and reporting that as "empty" or "building" would
    // be wrong in opposite directions.
    let (_dir, db) = db().await;
    add_titles(&db, 5).await;

    assert_eq!(
        CatalogueRepository::new(&db)
            .readiness()
            .await
            .expect("readiness"),
        Readiness::Ready { titles: 5 }
    );
}

#[tokio::test]
async fn a_running_job_reports_what_is_searchable_right_now() {
    // The whole requirement: the app is usable DURING the build.
    let (_dir, db) = db().await;
    add_titles(&db, 3).await;
    let id = job(&db, "imdb", "running", 2).await;
    step(&db, id, "title.basics", 1, "running", 1_234, None).await;

    let readiness = CatalogueRepository::new(&db)
        .readiness()
        .await
        .expect("readiness");
    match readiness {
        Readiness::Building { titles, job, step } => {
            assert_eq!(titles, 3, "three titles are searchable already");
            assert_eq!(job, "imdb");
            let step = step.expect("a step in progress");
            assert_eq!(step.name, "title.basics");
            assert_eq!(step.items_done, 1_234);
            assert_eq!(
                step.percent(),
                None,
                "a streamed file has no total, so no percentage may be invented"
            );
        }
        other => panic!("expected Building, got {other:?}"),
    }
}

#[tokio::test]
async fn the_current_step_is_the_first_unfinished_one() {
    let (_dir, db) = db().await;
    let id = job(&db, "imdb", "running", 1).await;
    step(&db, id, "title.ratings", 1, "complete", 100, Some(100)).await;
    step(&db, id, "title.basics", 2, "running", 40, Some(200)).await;
    step(&db, id, "title.akas", 3, "pending", 0, None).await;

    let readiness = CatalogueRepository::new(&db)
        .readiness()
        .await
        .expect("readiness");
    let Readiness::Building {
        step: Some(step), ..
    } = readiness
    else {
        panic!("expected Building with a step");
    };
    assert_eq!(step.name, "title.basics");
    assert_eq!(step.percent(), Some(20.0));
}

#[tokio::test]
async fn a_job_whose_process_died_stops_claiming_to_be_building() {
    // `status` stays 'running' when a process is killed. Showing a progress bar for a
    // process that no longer exists is the lie this guards against — the honest thing
    // is "resume", and the runner will adopt the job on the next launch.
    let (_dir, db) = db().await;
    add_titles(&db, 7).await;
    let id = job(&db, "imdb", "running", 6_000).await;
    step(&db, id, "title.basics", 1, "running", 900, None).await;

    let readiness = CatalogueRepository::new(&db)
        .readiness()
        .await
        .expect("readiness");
    match readiness {
        Readiness::Interrupted {
            titles, job, step, ..
        } => {
            assert_eq!(
                titles, 7,
                "what was ingested before the crash is still searchable"
            );
            assert_eq!(job, "imdb");
            assert_eq!(step.as_deref(), Some("title.basics"));
        }
        other => panic!("expected Interrupted, got {other:?}"),
    }
}

#[tokio::test]
async fn a_job_still_moving_is_not_declared_dead() {
    // The other half of the pair. A slow step on a slow disk must not be reported as
    // a crash, which is why the threshold is generous.
    let (_dir, db) = db().await;
    let id = job(&db, "imdb", "running", 60).await;
    step(&db, id, "title.basics", 1, "running", 10, None).await;

    assert!(matches!(
        CatalogueRepository::new(&db)
            .readiness()
            .await
            .expect("readiness"),
        Readiness::Building { .. }
    ));
}

#[tokio::test]
async fn a_failed_job_carries_its_error_forward() {
    let (_dir, db) = db().await;
    add_titles(&db, 2).await;
    let id = job(&db, "anilist", "failed", 10).await;
    sqlx::query("UPDATE ingest_jobs SET error = ? WHERE id = ?")
        .bind("anilist returned 400")
        .bind(id)
        .execute(db.pool())
        .await
        .expect("error");

    let readiness = CatalogueRepository::new(&db)
        .readiness()
        .await
        .expect("readiness");
    match readiness {
        Readiness::Interrupted { error, .. } => {
            assert_eq!(error.as_deref(), Some("anilist returned 400"))
        }
        other => panic!("expected Interrupted, got {other:?}"),
    }
}

#[tokio::test]
async fn the_newest_job_is_the_one_that_counts() {
    // Ingestion is a sequence of jobs. A completed `imdb` followed by a running
    // `anilist` is a catalogue still being built, not a finished one.
    let (_dir, db) = db().await;
    add_titles(&db, 4).await;
    job(&db, "imdb", "complete", 500).await;
    let newer = job(&db, "anilist", "running", 1).await;
    step(&db, newer, "anilist.catalogue", 1, "running", 50, None).await;

    assert!(matches!(
        CatalogueRepository::new(&db).readiness().await.expect("readiness"),
        Readiness::Building { job, .. } if job == "anilist"
    ));
}

#[tokio::test]
async fn episodes_do_not_inflate_the_searchable_count() {
    // Half a million episode rows would make a barely-started catalogue look complete.
    let (_dir, db) = db().await;
    add_titles(&db, 2).await;
    let media = sinephile_persistence::repositories::MediaRepository::new(&db);
    media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Episode),
            primary_title: "An Episode".into(),
            ..NewMediaItem::film("An Episode", 2020)
        })
        .await
        .expect("episode");

    assert_eq!(
        CatalogueRepository::new(&db)
            .searchable_titles()
            .await
            .expect("count"),
        2
    );
}

#[tokio::test]
async fn every_step_is_available_for_a_detailed_panel() {
    let (_dir, db) = db().await;
    let id = job(&db, "imdb", "running", 1).await;
    step(&db, id, "title.ratings", 1, "complete", 100, Some(100)).await;
    step(&db, id, "title.basics", 2, "running", 40, None).await;

    let steps = CatalogueRepository::new(&db).steps().await.expect("steps");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].name, "title.ratings", "ordered by ordinal");
    assert_eq!(steps[1].name, "title.basics");
}
