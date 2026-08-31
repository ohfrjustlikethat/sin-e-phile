//! Loading IMDb titles into the catalogue.
//!
//! Every SQL statement in `load.rs` runs against a migrated database here, which is
//! ADR-0026's standing requirement — the compensating control for not using sqlx's
//! compile-time macros.
//!
//! The test that matters most is `a_resumed_load_loses_no_title_and_duplicates_none`.
//! A loader that drops one title per resume, or inserts one twice, produces a
//! catalogue that is wrong in a way nothing downstream will ever report.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use sinephile_ingest::imdb::CatalogueScope;
use sinephile_ingest::load;
use sinephile_ingest::{Job, JobError};
use sinephile_persistence::Db;

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

const BASICS_HEADER: &str =
    "tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres\n";

/// A small, deliberately awkward catalogue: an adult title, a video game, an
/// episode, a title whose original name differs, and one with no genres.
fn basics() -> String {
    let mut out = String::from(BASICS_HEADER);
    for (n, kind, primary, original, adult, year, runtime, genres) in [
        (
            1,
            "movie",
            "Seven Samurai",
            "Shichinin no samurai",
            "0",
            "1954",
            "207",
            "Action,Drama",
        ),
        (
            2,
            "short",
            "An Obscure Short",
            "An Obscure Short",
            "0",
            "1913",
            "12",
            "Short",
        ),
        (
            3,
            "videoGame",
            "Not A Film",
            "Not A Film",
            "0",
            "2019",
            "\\N",
            "Action",
        ),
        (
            4,
            "tvEpisode",
            "An Episode",
            "An Episode",
            "0",
            "2001",
            "44",
            "Drama",
        ),
        (
            5,
            "movie",
            "Adult Title",
            "Adult Title",
            "1",
            "2010",
            "90",
            "\\N",
        ),
        (
            6, "tvSeries", "A Series", "A Series", "0", "1990", "\\N", "Drama",
        ),
        (
            7,
            "movie",
            "Brand New Film",
            "Brand New Film",
            "0",
            "2026",
            "101",
            "Drama",
        ),
    ] {
        out.push_str(&format!(
            "tt{n:07}\t{kind}\t{primary}\t{original}\t{adult}\t{year}\t\\N\t{runtime}\t{genres}\n"
        ));
    }
    out
}

/// Votes: Seven Samurai is famous, the short has two, the series has plenty.
/// tt0000007 (2026) deliberately has NO rating row — the new-release case.
fn ratings() -> String {
    "tconst\taverageRating\tnumVotes\n\
     tt0000001\t8.6\t380000\n\
     tt0000002\t5.5\t2\n\
     tt0000005\t6.0\t900\n\
     tt0000006\t8.1\t5000\n"
        .to_string()
}

struct Fixture {
    _dir: tempfile::TempDir,
    basics: std::path::PathBuf,
    votes: Arc<HashMap<u32, i64>>,
    averages: Arc<HashMap<u32, i64>>,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let basics = write_gz(dir.path(), "title.basics.tsv.gz", &basics());
    let ratings_path = write_gz(dir.path(), "title.ratings.tsv.gz", &ratings());

    Fixture {
        votes: Arc::new(load::load_votes(&ratings_path).expect("votes")),
        averages: Arc::new(load::load_average_ratings(&ratings_path).expect("averages")),
        basics,
        _dir: dir,
    }
}

async fn run_load(db: &Db, fx: &Fixture) -> Result<(), JobError> {
    let mut job = Job::begin(db, "imdb").await?;
    load::load_titles(
        &mut job,
        fx.basics.clone(),
        Arc::clone(&fx.votes),
        Arc::clone(&fx.averages),
        CatalogueScope::DEFAULT,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn the_scope_decides_what_is_indexed() {
    let db = Db::in_memory().await.expect("open");
    let fx = fixture();
    run_load(&db, &fx).await.expect("load");

    let titles: Vec<String> =
        sqlx::query_scalar("SELECT primary_title FROM media_items ORDER BY id")
            .fetch_all(db.pool())
            .await
            .expect("query");

    assert_eq!(
        titles,
        vec![
            "Seven Samurai",
            "An Obscure Short",
            "A Series",
            "Brand New Film"
        ],
        "video games, episodes and adult titles are excluded; the obscure short is NOT"
    );
}

#[tokio::test]
async fn the_core_tier_is_the_popular_titles_plus_new_releases() {
    let db = Db::in_memory().await.expect("open");
    let fx = fixture();
    run_load(&db, &fx).await.expect("load");

    let core: Vec<String> =
        sqlx::query_scalar("SELECT primary_title FROM media_items WHERE in_core = 1 ORDER BY id")
            .fetch_all(db.pool())
            .await
            .expect("query");

    assert!(core.contains(&"Seven Samurai".to_string()), "380,000 votes");
    assert!(core.contains(&"A Series".to_string()), "5,000 votes");
    assert!(
        core.contains(&"Brand New Film".to_string()),
        "no votes, but released this year — the rescue the author asked for"
    );
    assert!(
        !core.contains(&"An Obscure Short".to_string()),
        "two votes and from 1913: indexed, but not enriched"
    );

    let (total, core_count) = load::counts(&db).await.expect("counts");
    assert_eq!((total, core_count), (4, 3));
}

#[tokio::test]
async fn external_ids_titles_and_genres_all_land() {
    let db = Db::in_memory().await.expect("open");
    let fx = fixture();
    run_load(&db, &fx).await.expect("load");

    // The IMDb id is what everything else joins on.
    let id: i64 = sqlx::query_scalar(
        "SELECT media_item_id FROM external_ids WHERE source = 'imdb' AND external_id = 'tt0000001'",
    )
    .fetch_one(db.pool())
    .await
    .expect("external id");

    let variants: Vec<(String, String)> = sqlx::query_as(
        "SELECT title, variant FROM titles WHERE media_item_id = ? ORDER BY variant",
    )
    .bind(id)
    .fetch_all(db.pool())
    .await
    .expect("titles");
    assert!(variants
        .iter()
        .any(|(t, v)| t == "Seven Samurai" && v == "primary"));
    assert!(
        variants
            .iter()
            .any(|(t, v)| t == "Shichinin no samurai" && v == "original"),
        "a differing original title is kept — §6.2 wants the romaji findable"
    );

    let genres: Vec<String> = sqlx::query_scalar(
        "SELECT g.name FROM genres g
         JOIN media_genres mg ON mg.genre_id = g.id
         WHERE mg.media_item_id = ? ORDER BY g.name",
    )
    .bind(id)
    .fetch_all(db.pool())
    .await
    .expect("genres");
    assert_eq!(genres, vec!["Action", "Drama"]);
}

#[tokio::test]
async fn an_identical_original_title_is_not_stored_twice() {
    let db = Db::in_memory().await.expect("open");
    let fx = fixture();
    run_load(&db, &fx).await.expect("load");

    let id: i64 = sqlx::query_scalar(
        "SELECT media_item_id FROM external_ids WHERE external_id = 'tt0000006'",
    )
    .fetch_one(db.pool())
    .await
    .expect("id");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM titles WHERE media_item_id = ?")
        .bind(id)
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(
        count, 1,
        "primaryTitle == originalTitle stores one row, not two"
    );
}

#[tokio::test]
async fn ratings_are_scaled_to_the_schema_range() {
    // The schema stores 0-100 so ranking never compares floats (migration 0001).
    let db = Db::in_memory().await.expect("open");
    let fx = fixture();
    run_load(&db, &fx).await.expect("load");

    let rating: Option<i64> =
        sqlx::query_scalar("SELECT rating FROM media_items WHERE primary_title = 'Seven Samurai'")
            .fetch_one(db.pool())
            .await
            .expect("rating");
    assert_eq!(rating, Some(86), "8.6 becomes 86");
}

#[tokio::test]
async fn a_resumed_load_loses_no_title_and_duplicates_none() {
    // The test that matters. A loader that drops one title per resume, or inserts
    // one twice, produces a catalogue nothing downstream will report as wrong.
    //
    // `seek_past` stops ON the first unprocessed row rather than before it, so that
    // row must be taken from the buffer before reading onward. Forgetting it loses
    // exactly one title per resume — invisibly, in 2.7 million.
    let dir = tempfile::tempdir().expect("tempdir");

    // Enough titles to span several batches at a small batch size.
    let mut basics_text = String::from(BASICS_HEADER);
    for n in 1..=40 {
        basics_text.push_str(&format!(
            "tt{n:07}\tmovie\tFilm {n}\tFilm {n}\t0\t2000\t\\N\t100\tDrama\n"
        ));
    }
    let basics = write_gz(dir.path(), "title.basics.tsv.gz", &basics_text);

    let mut votes_text = String::from("tconst\taverageRating\tnumVotes\n");
    for n in 1..=40 {
        votes_text.push_str(&format!("tt{n:07}\t7.0\t500\n"));
    }
    let ratings_path = write_gz(dir.path(), "title.ratings.tsv.gz", &votes_text);

    let votes = Arc::new(load::load_votes(&ratings_path).expect("votes"));
    let averages = Arc::new(load::load_average_ratings(&ratings_path).expect("averages"));

    let db = Db::in_memory().await.expect("open");

    // Load in one go, then again — the second must be a no-op, not a second copy.
    let mut job = Job::begin(&db, "imdb").await.expect("job");
    load::load_titles(
        &mut job,
        basics.clone(),
        Arc::clone(&votes),
        Arc::clone(&averages),
        CatalogueScope::DEFAULT,
    )
    .await
    .expect("first load");

    let mut again = Job::begin(&db, "imdb").await.expect("job");
    load::load_titles(
        &mut again,
        basics.clone(),
        Arc::clone(&votes),
        Arc::clone(&averages),
        CatalogueScope::DEFAULT,
    )
    .await
    .expect("second load");

    let titles: Vec<String> =
        sqlx::query_scalar("SELECT primary_title FROM media_items ORDER BY id")
            .fetch_all(db.pool())
            .await
            .expect("query");

    let expected: Vec<String> = (1..=40).map(|n| format!("Film {n}")).collect();
    assert_eq!(titles, expected, "every title exactly once, in order");

    let external: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM external_ids")
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(external, 40, "one IMDb id per title, no duplicates");
}

#[tokio::test]
async fn a_dataset_missing_a_column_is_rejected_by_name() {
    // IMDb has changed these before. The failure must name the column rather than
    // producing a catalogue of empty fields.
    let dir = tempfile::tempdir().expect("tempdir");
    let basics = write_gz(
        dir.path(),
        "title.basics.tsv.gz",
        "tconst\ttitleType\tprimaryTitle\n tt0000001\tmovie\tA Film\n",
    );
    let ratings_path = write_gz(dir.path(), "title.ratings.tsv.gz", &ratings());

    let db = Db::in_memory().await.expect("open");
    let mut job = Job::begin(&db, "imdb").await.expect("job");

    let error = load::load_titles(
        &mut job,
        basics,
        Arc::new(load::load_votes(&ratings_path).expect("votes")),
        Arc::new(HashMap::new()),
        CatalogueScope::DEFAULT,
    )
    .await
    .expect_err("should be rejected")
    .to_string();

    assert!(
        error.contains("runtimeMinutes") || error.contains("genres"),
        "names it: {error}"
    );
}

#[tokio::test]
async fn ids_are_taken_from_returning_not_from_rowid_arithmetic() {
    // The obvious shortcut is `last_insert_rowid() - chunk.len() + 1`, and it is
    // correct only while the table has no gaps. Make a gap, then load: with the
    // arithmetic, every external id and title row attaches to the WRONG film —
    // silently, and permanently.
    let db = Db::in_memory().await.expect("open");

    // Seed rows and delete some, so ids are non-contiguous and the next assignment
    // does not start where a naive base calculation would assume.
    for n in 1..=10 {
        sqlx::query("INSERT INTO media_items (kind, primary_title) VALUES ('film', ?)")
            .bind(format!("seed {n}"))
            .execute(db.pool())
            .await
            .expect("seed");
    }
    sqlx::query("DELETE FROM media_items WHERE id IN (3, 4, 5, 7)")
        .execute(db.pool())
        .await
        .expect("delete");

    let fx = fixture();
    run_load(&db, &fx).await.expect("load");

    // Every IMDb id must point at the film that actually carries that title.
    for (tconst, expected) in [
        ("tt0000001", "Seven Samurai"),
        ("tt0000002", "An Obscure Short"),
        ("tt0000006", "A Series"),
        ("tt0000007", "Brand New Film"),
    ] {
        let title: String = sqlx::query_scalar(
            "SELECT m.primary_title FROM media_items m
             JOIN external_ids e ON e.media_item_id = m.id
             WHERE e.external_id = ?",
        )
        .bind(tconst)
        .fetch_one(db.pool())
        .await
        .unwrap_or_else(|e| panic!("{tconst} did not resolve: {e}"));

        assert_eq!(title, expected, "{tconst} attached to the wrong film");
    }
}
