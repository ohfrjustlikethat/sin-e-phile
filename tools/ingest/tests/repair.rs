//! Repairing variants the region-over-language bug mislabelled.
//!
//! Every SQL statement in `repair.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).

use sinephile_ingest::{repair, Job};
use sinephile_persistence::Db;

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    (dir, db)
}

async fn item(db: &Db, title: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO media_items (kind, primary_title, release_year)
         VALUES ('film', ?, 2001) RETURNING id",
    )
    .bind(title)
    .fetch_one(db.pool())
    .await
    .expect("insert item")
}

async fn title(
    db: &Db,
    item: i64,
    text: &str,
    variant: &str,
    lang: Option<&str>,
    region: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO titles (media_item_id, title, variant, language, region)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(item)
    .bind(text)
    .bind(variant)
    .bind(lang)
    .bind(region)
    .execute(db.pool())
    .await
    .expect("insert title");
}

async fn variants(db: &Db, item: i64) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT variant, title FROM titles WHERE media_item_id = ? ORDER BY variant, title",
    )
    .bind(item)
    .fetch_all(db.pool())
    .await
    .expect("read titles")
}

#[tokio::test]
async fn a_french_title_stops_claiming_to_be_english() {
    // The real case: IMDb lists the French title of Spirited Away with region CA and
    // the Spanish one with region US. Reading the region before the language made both
    // of them the film's English title — 41,193 rows across the real catalogue.
    let (_dir, db) = db().await;
    let id = item(&db, "Spirited Away").await;
    title(&db, id, "Spirited Away", "english", Some("en"), Some("HK")).await;
    title(
        &db,
        id,
        "Le voyage de Chihiro",
        "english",
        Some("fr"),
        Some("CA"),
    )
    .await;
    title(
        &db,
        id,
        "El viaje de Chihiro",
        "english",
        Some("es"),
        Some("US"),
    )
    .await;

    let mut job = Job::begin(&db, "repair").await.expect("begin");
    repair::english_variants(&mut job).await.expect("repair");
    job.finish().await.expect("finish");

    assert_eq!(
        variants(&db, id).await,
        vec![
            ("alternative".to_string(), "El viaje de Chihiro".to_string()),
            (
                "alternative".to_string(),
                "Le voyage de Chihiro".to_string()
            ),
            ("english".to_string(), "Spirited Away".to_string()),
        ],
        "the genuinely English title is the one left standing"
    );
    assert_eq!(repair::mislabelled_english(&db).await.expect("count"), 0);
}

#[tokio::test]
async fn a_row_with_no_language_is_left_alone() {
    // The region IS the only evidence when the language is unknown, so those rows were
    // never wrong and the repair must not touch them.
    let (_dir, db) = db().await;
    let id = item(&db, "Some Film").await;
    title(&db, id, "Some Film", "english", None, Some("US")).await;

    let mut job = Job::begin(&db, "repair").await.expect("begin");
    repair::english_variants(&mut job).await.expect("repair");

    assert_eq!(
        variants(&db, id).await,
        vec![("english".to_string(), "Some Film".to_string())]
    );
}

#[tokio::test]
async fn a_corrected_row_that_collides_with_an_existing_one_leaves_a_single_row() {
    // The unique index is (media_item_id, variant, title). If the same text is already
    // stored under the corrected variant, the update collides — and silently dropping
    // either row would be worse than the bug.
    let (_dir, db) = db().await;
    let id = item(&db, "Le Film").await;
    title(&db, id, "Le Film", "english", Some("fr"), Some("CA")).await;
    title(&db, id, "Le Film", "alternative", Some("fr"), None).await;

    let mut job = Job::begin(&db, "repair").await.expect("begin");
    repair::english_variants(&mut job).await.expect("repair");

    assert_eq!(
        variants(&db, id).await,
        vec![("alternative".to_string(), "Le Film".to_string())],
        "one row survives, under the right variant"
    );
}

#[tokio::test]
async fn a_yiddish_title_in_hebrew_script_becomes_native_not_alternative() {
    // The correction is the real `variant()` function, not a blanket rewrite to
    // 'alternative' — script still decides between native and alternative.
    let (_dir, db) = db().await;
    let id = item(&db, "A Film").await;
    title(&db, id, "אַ פֿילם", "english", Some("yi"), Some("US")).await;

    let mut job = Job::begin(&db, "repair").await.expect("begin");
    repair::english_variants(&mut job).await.expect("repair");

    assert_eq!(
        variants(&db, id).await,
        vec![("native".to_string(), "אַ פֿילם".to_string())]
    );
}

#[tokio::test]
async fn the_repair_is_re_runnable() {
    // A repair that cannot be run twice is a repair nobody dares run once.
    let (_dir, db) = db().await;
    let id = item(&db, "Spirited Away").await;
    title(
        &db,
        id,
        "Le voyage de Chihiro",
        "english",
        Some("fr"),
        Some("CA"),
    )
    .await;

    for _ in 0..2 {
        let mut job = Job::begin(&db, "repair").await.expect("begin");
        repair::english_variants(&mut job).await.expect("repair");
        job.finish().await.expect("finish");
    }

    assert_eq!(
        variants(&db, id).await,
        vec![(
            "alternative".to_string(),
            "Le voyage de Chihiro".to_string()
        )]
    );
}
