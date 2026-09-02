//! The normalised-title backfill and candidate lookup (migration 0009).

use sinephile_ingest::matching::{match_title, title_forms, TitleIndex};
use sinephile_ingest::{normalise, Job};
use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::{Db, NewMediaItem};

async fn seeded() -> Db {
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    for (title, year) in [
        ("Fullmetal Alchemist: Brotherhood", 2009),
        ("Cowboy Bebop", 1998),
        ("Kaguya-sama: Love is War", 2019),
    ] {
        media
            .insert(&NewMediaItem::film(title, year))
            .await
            .expect("insert");
    }
    db
}

async fn backfilled() -> Db {
    let db = seeded().await;
    let mut job = Job::begin(&db, "titles-normalise").await.expect("job");
    normalise::backfill(&mut job).await.expect("backfill");
    db
}

#[tokio::test]
async fn every_title_gets_a_normalised_form() {
    let db = backfilled().await;
    assert_eq!(normalise::remaining(&db).await.expect("remaining"), 0);

    let value: String =
        sqlx::query_scalar("SELECT normalised FROM titles WHERE title LIKE 'Fullmetal%'")
            .fetch_one(db.pool())
            .await
            .expect("query");
    assert_eq!(
        value, "fullmetal alchemist brotherhood",
        "punctuation is gone"
    );
}

#[tokio::test]
async fn a_punctuation_difference_no_longer_hides_a_candidate() {
    // The whole reason for the column. Fetching by raw title would find only the
    // titles that already agree — the easy cases the normaliser exists because they
    // are not the whole problem.
    let db = backfilled().await;
    let forms = vec!["fullmetal alchemist brotherhood".to_string()];
    let found = normalise::candidates(&db, &forms)
        .await
        .expect("candidates");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Fullmetal Alchemist: Brotherhood");
    assert_eq!(found[0].year, Some(2009));
}

#[tokio::test]
async fn the_candidates_feed_straight_into_the_matcher() {
    // End to end: AniList's three forms, normalised, looked up, matched.
    let db = backfilled().await;

    let forms = title_forms(
        Some("Hagane no Renkinjutsushi: Fullmetal Alchemist"),
        Some("Fullmetal Alchemist: Brotherhood"),
        None,
    );
    let keys: Vec<String> = forms
        .iter()
        .map(|f| sinephile_ingest::matching::normalise(f))
        .collect();

    let mut index = TitleIndex::new();
    for candidate in normalise::candidates(&db, &keys).await.expect("candidates") {
        index.insert(candidate);
    }

    let found = match_title(&index, &forms, Some(2009)).expect("match");
    assert_eq!(found.matched_on, "Fullmetal Alchemist: Brotherhood");
}

#[tokio::test]
async fn a_lookup_with_no_forms_queries_nothing() {
    let db = backfilled().await;
    assert!(normalise::candidates(&db, &[])
        .await
        .expect("candidates")
        .is_empty());
}

#[tokio::test]
async fn the_backfill_resumes_without_redoing_or_skipping() {
    // The cursor is titles.id, so a resume is an index seek rather than an offset
    // scan over six million rows.
    let db = seeded().await;

    let mut job = Job::begin(&db, "titles-normalise").await.expect("job");
    normalise::backfill(&mut job).await.expect("first");
    job.finish().await.expect("finish");

    // A second run adopts nothing and must not double anything.
    let mut again = Job::begin(&db, "titles-normalise").await.expect("job");
    normalise::backfill(&mut again).await.expect("second");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM titles")
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(count, 3);
    assert_eq!(normalise::remaining(&db).await.expect("remaining"), 0);
}

#[tokio::test]
async fn a_title_loaded_after_the_backfill_is_reported_as_outstanding() {
    // Rather than silently missing from every future match.
    let db = backfilled().await;
    MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Something New", 2026))
        .await
        .expect("insert");

    assert_eq!(
        normalise::remaining(&db).await.expect("remaining"),
        1,
        "a new title has no normalised form until the backfill runs again"
    );
}

#[tokio::test]
async fn candidates_come_back_with_the_year_the_matcher_needs() {
    let db = backfilled().await;
    let forms = vec!["cowboy bebop".to_string()];
    let found = normalise::candidates(&db, &forms)
        .await
        .expect("candidates");
    assert_eq!(
        found[0].year,
        Some(1998),
        "a candidate with no year cannot be year-checked"
    );
}
