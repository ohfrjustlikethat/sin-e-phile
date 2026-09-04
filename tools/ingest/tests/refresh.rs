//! Incremental catalogue refresh (ADR-0030 layer 1).
//!
//! Every SQL statement in `refresh.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use sinephile_ingest::imdb::CatalogueScope;
use sinephile_ingest::{load, refresh, Job};
use sinephile_persistence::Db;

const NULL: char = '\\';

fn null() -> String {
    format!("{NULL}N")
}

fn write_gz(dir: &std::path::Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut encoder = flate2::write::GzEncoder::new(
        std::fs::File::create(&path).expect("create"),
        flate2::Compression::fast(),
    );
    encoder.write_all(contents.as_bytes()).expect("write");
    encoder.finish().expect("finish");
    path
}

/// `title.basics` rows for the given ids, all films with enough votes to be core.
fn basics(ids: &[u32]) -> String {
    let n = null();
    let mut rows = vec![
        "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres"
            .to_string(),
    ];
    for id in ids {
        rows.push(format!(
            "tt{id:07}\tmovie\tFilm {id}\tFilm {id}\t0\t2020\t{n}\t100\tDrama"
        ));
    }
    rows.join("\n") + "\n"
}

async fn ingest(db: &Db, path: &std::path::Path, ids: &[u32], job_name: &str) {
    let votes: HashMap<u32, i64> = ids.iter().map(|i| (*i, 5_000)).collect();
    let averages: HashMap<u32, i64> = ids.iter().map(|i| (*i, 70)).collect();
    let watermark = refresh::watermark(db).await.expect("watermark");

    let mut job = Job::begin(db, job_name).await.expect("job");
    load::load_titles(
        &mut job,
        path.to_path_buf(),
        Arc::new(votes),
        Arc::new(averages),
        CatalogueScope::DEFAULT,
        watermark,
    )
    .await
    .expect("load");
    job.finish().await.expect("finish");
}

async fn titles(db: &Db) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT e.external_id FROM external_ids e
           JOIN media_items m ON m.id = e.media_item_id
          WHERE e.source = 'imdb' AND m.kind <> 'episode' ORDER BY e.external_id",
    )
    .fetch_all(db.pool())
    .await
    .expect("titles")
}

#[tokio::test]
async fn a_refresh_adds_only_what_is_newer_than_the_watermark() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");

    let first = write_gz(dir.path(), "a.tsv.gz", &basics(&[100, 200, 300]));
    ingest(&db, &first, &[100, 200, 300], "imdb").await;
    assert_eq!(titles(&db).await.len(), 3);

    // IMDb republishes: the same three, plus two new ones with higher ids.
    let second = write_gz(dir.path(), "b.tsv.gz", &basics(&[100, 200, 300, 400, 500]));
    ingest(&db, &second, &[100, 200, 300, 400, 500], "refresh").await;

    assert_eq!(
        titles(&db).await,
        vec![
            "tt0000100",
            "tt0000200",
            "tt0000300",
            "tt0000400",
            "tt0000500"
        ],
        "the three already held are not inserted a second time"
    );
}

#[tokio::test]
async fn the_watermark_is_a_number_because_the_file_is_sorted_as_text() {
    // ADR-0032: title.basics is sorted LEXICOGRAPHICALLY. Numeric order first breaks at
    // row 967,458 (tt10001008 -> tt1000101) and the last row is tt9916880, which is not
    // the largest id. A watermark can only be a number to compare, never a position.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    assert_eq!(refresh::watermark(&db).await.expect("empty"), None);

    let path = write_gz(dir.path(), "a.tsv.gz", &basics(&[100, 9_916_880]));
    ingest(&db, &path, &[100, 9_916_880], "imdb").await;

    assert_eq!(
        refresh::watermark(&db).await.expect("watermark"),
        Some(9_916_880)
    );
}

#[tokio::test]
async fn a_new_id_sorted_before_ids_we_hold_is_still_found() {
    // The failure ADR-0030's mechanism produced against the real catalogue. In
    // LEXICOGRAPHIC order "tt10001008" sits between "tt1" and "tt2", so a new title
    // with a high numeric id lands in the MIDDLE of the file, ahead of ids already
    // held. Seeking to a position rather than filtering by number either misses it or
    // re-reads everything after it and dies on a UNIQUE violation. It died.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");

    // Written in the order IMDb writes it: sorted as text, not as number.
    let mut held = vec![1_000_101u32, 9_916_880];
    held.sort_by_key(|id| format!("tt{id:07}"));
    let first = write_gz(dir.path(), "a.tsv.gz", &basics(&held));
    ingest(&db, &first, &held, "imdb").await;
    assert_eq!(refresh::watermark(&db).await.expect("w"), Some(9_916_880));

    let mut with_new = vec![1_000_101u32, 9_916_880, 10_001_008];
    with_new.sort_by_key(|id| format!("tt{id:07}"));
    assert_eq!(
        with_new,
        vec![10_001_008, 1_000_101, 9_916_880],
        "sanity: this is row 967,458 of the real file — tt10001008 immediately          precedes tt1000101, and both precede tt9916880"
    );

    let second = write_gz(dir.path(), "b.tsv.gz", &basics(&with_new));
    ingest(&db, &second, &with_new, "refresh").await;

    assert!(
        titles(&db).await.contains(&"tt10001008".to_string()),
        "the new title must be found even though it sorts early"
    );
}

#[tokio::test]
async fn an_episode_id_does_not_drag_the_watermark_past_unread_titles() {
    // Episodes carry IMDb ids from the same space and are loaded only for series in
    // scope, so the highest STORED id is easily an episode far above the highest title
    // the scan considered. Seeking title.basics past that would skip the range between
    // them — silently, and only for the titles a refresh exists to find.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");

    let path = write_gz(dir.path(), "a.tsv.gz", &basics(&[100, 200]));
    ingest(&db, &path, &[100, 200], "imdb").await;

    // An episode with a much higher id, as `ingest episodes` would leave it.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO media_items (kind, primary_title) VALUES ('episode', 'An Episode') RETURNING id",
    )
    .fetch_one(db.pool())
    .await
    .expect("insert episode");
    sqlx::query("INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, 'imdb', 'tt9999999')")
        .bind(id)
        .execute(db.pool())
        .await
        .expect("insert id");

    assert_eq!(
        refresh::watermark(&db).await.expect("watermark"),
        Some(200),
        "the episode must not become the title watermark"
    );

    // And the refresh therefore still finds a title between the two.
    let second = write_gz(dir.path(), "b.tsv.gz", &basics(&[100, 200, 300]));
    ingest(&db, &second, &[100, 200, 300], "refresh").await;
    assert!(titles(&db).await.contains(&"tt0000300".to_string()));
}

#[tokio::test]
async fn ratings_are_re_applied_over_the_whole_catalogue() {
    // A stale rating is a wrong sort order, not merely an old number — and ratings
    // move constantly while title.ratings is only 8 MB.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let path = write_gz(dir.path(), "a.tsv.gz", &basics(&[100, 200]));
    ingest(&db, &path, &[100, 200], "imdb").await;

    let before: Vec<(Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT rating, rating_votes FROM media_items ORDER BY id")
            .fetch_all(db.pool())
            .await
            .expect("before");
    assert_eq!(
        before,
        vec![(Some(70), Some(5_000)), (Some(70), Some(5_000))]
    );

    let votes: HashMap<u32, i64> = [(100u32, 9_999i64), (200, 12)].into_iter().collect();
    let averages: HashMap<u32, i64> = [(100u32, 91i64), (200, 33)].into_iter().collect();

    let mut job = Job::begin(&db, "refresh").await.expect("job");
    let updated = refresh::ratings(&mut job, Arc::new(votes), Arc::new(averages))
        .await
        .expect("ratings");
    job.finish().await.expect("finish");

    assert_eq!(updated, 2);
    let after: Vec<(Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT rating, rating_votes FROM media_items ORDER BY id")
            .fetch_all(db.pool())
            .await
            .expect("after");
    assert_eq!(after, vec![(Some(91), Some(9_999)), (Some(33), Some(12))]);
}

#[tokio::test]
async fn a_rating_for_a_title_we_never_ingested_matches_nothing() {
    // IMDb rates far more titles than we keep. Those must simply not match, rather
    // than erroring or creating a row.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let path = write_gz(dir.path(), "a.tsv.gz", &basics(&[100]));
    ingest(&db, &path, &[100], "imdb").await;

    let votes: HashMap<u32, i64> = [(100u32, 42i64), (777, 1)].into_iter().collect();
    let averages: HashMap<u32, i64> = [(100u32, 80i64), (777, 10)].into_iter().collect();

    let mut job = Job::begin(&db, "refresh").await.expect("job");
    let updated = refresh::ratings(&mut job, Arc::new(votes), Arc::new(averages))
        .await
        .expect("ratings");

    assert_eq!(updated, 1, "only the title we hold");
    assert_eq!(
        refresh::title_count(&db).await.expect("count"),
        1,
        "no row was invented for tt0000777"
    );
}

#[tokio::test]
async fn a_refresh_with_nothing_new_adds_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let path = write_gz(dir.path(), "a.tsv.gz", &basics(&[100, 200]));
    ingest(&db, &path, &[100, 200], "imdb").await;

    ingest(&db, &path, &[100, 200], "refresh").await;
    assert_eq!(refresh::title_count(&db).await.expect("count"), 2);
}
