//! `Db::compact` — the VACUUM that D29 and D47 asked ingestion to run when it finishes.

use sinephile_persistence::Db;

#[tokio::test]
async fn compaction_recovers_the_slack_that_deleted_rows_leave() {
    // Incremental ingestion writes and rewrites; deleted and moved rows leave pages a
    // plain file size keeps counting. Make some on purpose, then require them back.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open(&dir.path().join("c.db")).await.expect("open");

    sqlx::query("CREATE TABLE scratch (body TEXT NOT NULL)")
        .execute(db.pool())
        .await
        .expect("create");
    let body = "x".repeat(4_000);
    for _ in 0..500 {
        sqlx::query("INSERT INTO scratch (body) VALUES (?)")
            .bind(&body)
            .execute(db.pool())
            .await
            .expect("insert");
    }
    sqlx::query("DELETE FROM scratch")
        .execute(db.pool())
        .await
        .expect("delete");

    let compacted = db.compact().await.expect("compact");
    assert!(
        compacted.recovered() > 1_000_000,
        "500 deleted 4 KB rows should give back well over a megabyte: {compacted:?}"
    );

    // And the database is still the database.
    assert_eq!(
        db.schema_version().await.expect("open after compact"),
        Some(Db::latest_schema_version())
    );
}

#[tokio::test]
async fn a_compact_database_stays_the_same_size() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open(&dir.path().join("c.db")).await.expect("open");
    db.compact().await.expect("first");
    let again = db.compact().await.expect("second");
    assert_eq!(again.recovered(), 0, "nothing left to recover: {again:?}");
}
