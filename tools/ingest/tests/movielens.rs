//! MovieLens joining.
//!
//! Every SQL statement in `movielens.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).
//!
//! The archive is built in the test rather than downloaded. That is not only for
//! speed: `files.grouplens.org` served an expired certificate on 2026-09-04, and a
//! test suite that cannot run while a third party's certificate is broken is a test
//! suite that stops being run.

use std::io::Write;
use std::sync::Arc;

use sinephile_ingest::movielens as ml;
use sinephile_ingest::Job;
use sinephile_persistence::Db;

/// A MovieLens archive, shaped like the real one: everything nested under a directory
/// named after the release.
fn archive(dir: &std::path::Path, links: &str, ratings: &str) -> std::path::PathBuf {
    let path = dir.join("ml-test.zip");
    let file = std::fs::File::create(&path).expect("create");
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("ml-test/links.csv", options).expect("entry");
    zip.write_all(links.as_bytes()).expect("write");
    zip.start_file("ml-test/ratings.csv", options)
        .expect("entry");
    zip.write_all(ratings.as_bytes()).expect("write");
    zip.finish().expect("finish");
    path
}

fn links_csv() -> String {
    // imdbId has no `tt` and no zero padding, exactly as GroupLens writes it.
    "movieId,imdbId,tmdbId\n\
     1,114709,862\n\
     2,113497,8844\n\
     3,,\n\
     4,999999,111\n"
        .to_string()
}

fn ratings_csv() -> String {
    let mut out = String::from("userId,movieId,rating,timestamp\n");
    for user in 1..=3 {
        for movie in 1..=4 {
            out.push_str(&format!("{user},{movie},4.0,1112486027\n"));
        }
    }
    out
}

async fn film(db: &Db, tconst: &str, title: &str) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO media_items (kind, primary_title) VALUES ('film', ?) RETURNING id",
    )
    .bind(title)
    .fetch_one(db.pool())
    .await
    .expect("insert film");
    sqlx::query(
        "INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, 'imdb', ?)",
    )
    .bind(id)
    .bind(tconst)
    .execute(db.pool())
    .await
    .expect("insert id");
    id
}

#[tokio::test]
async fn links_are_read_with_imdb_ids_unpadded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = archive(dir.path(), &links_csv(), &ratings_csv());

    let links = ml::links(&path).expect("links");
    assert_eq!(links.len(), 3, "the row with a blank imdbId is skipped");
    assert_eq!(links[0].movielens_id, 1);
    assert_eq!(
        links[0].imdb_id, 114709,
        "links.csv stores 0114709 as 114709"
    );
}

#[tokio::test]
async fn a_changed_header_is_an_error_rather_than_silent_misparsing() {
    // If GroupLens reorders the columns, imdbId would silently become tmdbId and every
    // film would join to the wrong catalogue entry.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = archive(
        dir.path(),
        "movieId,tmdbId,imdbId\n1,862,114709\n",
        &ratings_csv(),
    );
    assert!(ml::links(&path).is_err());
}

#[tokio::test]
async fn only_films_we_hold_are_joined() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;
    film(&db, "tt0113497", "Jumanji").await;

    let path = archive(dir.path(), &links_csv(), &ratings_csv());
    let links = ml::links(&path).expect("links");
    let catalogue = ml::catalogue_by_imdb(&db).await.expect("catalogue");

    let pairs: Vec<(i64, i64)> = links
        .iter()
        .filter_map(|l| catalogue.get(&l.imdb_id).map(|id| (*id, l.movielens_id)))
        .collect();
    assert_eq!(pairs.len(), 2, "tt9999999 is not in the catalogue");

    let mut job = Job::begin(&db, "movielens").await.expect("begin");
    ml::load(&mut job, Arc::new(pairs)).await.expect("load");
    job.finish().await.expect("finish");

    assert_eq!(ml::mapped(&db).await.expect("mapped"), 2);
    let id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_ids WHERE media_item_id = ? AND source = 'movielens'",
    )
    .bind(toy)
    .fetch_one(db.pool())
    .await
    .expect("id");
    assert_eq!(id, "1");
}

#[tokio::test]
async fn two_movielens_ids_for_one_film_do_not_fail_the_batch() {
    // (source, external_id) is unique and (media_item_id, source) is the primary key.
    // MovieLens genuinely lists a film twice where IMDb later merged two entries, and
    // that must not take the whole load down.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;

    let mut job = Job::begin(&db, "movielens").await.expect("begin");
    ml::load(&mut job, Arc::new(vec![(toy, 1), (toy, 4242)]))
        .await
        .expect("load survives the collision");
    job.finish().await.expect("finish");

    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT external_id FROM external_ids WHERE source = 'movielens' ORDER BY external_id",
    )
    .fetch_all(db.pool())
    .await
    .expect("ids");
    assert_eq!(
        ids,
        vec!["1"],
        "the first one wins, and it is deterministic"
    );
}

#[tokio::test]
async fn the_load_is_re_runnable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let toy = film(&db, "tt0114709", "Toy Story").await;

    for _ in 0..2 {
        Job::reset(&db, "movielens").await.expect("reset");
        let mut job = Job::begin(&db, "movielens").await.expect("begin");
        ml::load(&mut job, Arc::new(vec![(toy, 1)]))
            .await
            .expect("load");
        job.finish().await.expect("finish");
    }
    assert_eq!(ml::mapped(&db).await.expect("mapped"), 1);
}

#[tokio::test]
async fn ratings_are_counted_without_being_stored() {
    // The ratings are an INPUT to the on-device matrix (ADR-0019), not catalogue data.
    // 25 million of them would not fit in R4's headroom and do not need to.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let path = archive(dir.path(), &links_csv(), &ratings_csv());

    let (count, seconds) = ml::scan_ratings(&path).expect("scan");
    assert_eq!(count, 12, "3 users x 4 films, header excluded");
    assert!(seconds >= 0.0);

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE '%rating%'",
    )
    .fetch_all(db.pool())
    .await
    .expect("tables");
    assert!(
        tables.is_empty(),
        "no ratings table exists to store them in"
    );
}
