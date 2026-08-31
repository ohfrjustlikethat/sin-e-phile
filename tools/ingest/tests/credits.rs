//! Cast and crew loading.
//!
//! Every SQL statement in `credits.rs` runs against a migrated database here
//! (ADR-0026's standing requirement — the compensating control for not using
//! sqlx's compile-time macros).

use std::io::Write;
use std::sync::Arc;

use sinephile_ingest::credits;
use sinephile_ingest::imdb::CatalogueScope;
use sinephile_ingest::{load, Job};
use sinephile_persistence::Db;

/// IMDb's null. Built rather than written literally, because a bare `\N` in a Rust
/// string is an invalid escape and `\\N` is easy to get wrong when a fixture is
/// edited later.
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

/// Rows joined with real tabs, so the fixture cannot drift from what a TSV is.
fn tsv(rows: &[&[&str]]) -> String {
    rows.iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// tt1 is core (380,000 votes); tt2 is indexed but NOT core (two votes, 1913).
fn basics() -> String {
    let n = null();
    tsv(&[
        &[
            "tconst",
            "titleType",
            "primaryTitle",
            "originalTitle",
            "isAdult",
            "startYear",
            "endYear",
            "runtimeMinutes",
            "genres",
        ],
        &[
            "tt0000001",
            "movie",
            "Seven Samurai",
            "Shichinin no samurai",
            "0",
            "1954",
            &n,
            "207",
            "Action,Drama",
        ],
        &[
            "tt0000002",
            "short",
            "An Obscure Short",
            "An Obscure Short",
            "0",
            "1913",
            &n,
            "12",
            "Short",
        ],
    ])
}

fn ratings() -> String {
    tsv(&[
        &["tconst", "averageRating", "numVotes"],
        &["tt0000001", "8.6", "380000"],
        &["tt0000002", "5.5", "2"],
    ])
}

fn principals() -> String {
    let n = null();
    tsv(&[
        &[
            "tconst",
            "ordering",
            "nconst",
            "category",
            "job",
            "characters",
        ],
        // A character name, wrapped in IMDb's JSON array.
        &[
            "tt0000001",
            "1",
            "nm0000001",
            "actor",
            &n,
            "[\"Kambei Shimada\"]",
        ],
        &["tt0000001", "2", "nm0000002", "director", &n, &n],
        // An unmapped category — must be dropped, not coerced.
        &["tt0000001", "3", "nm0000003", "production_designer", &n, &n],
        // Belongs to a NON-core title — must not be loaded at all.
        &["tt0000002", "1", "nm0000009", "actor", &n, "[\"Nobody\"]"],
    ])
}

fn names() -> String {
    let n = null();
    tsv(&[
        &[
            "nconst",
            "primaryName",
            "birthYear",
            "deathYear",
            "primaryProfession",
            "knownForTitles",
        ],
        &[
            "nm0000001",
            "Toshiro Mifune",
            "1920",
            "1997",
            "actor",
            "tt0000001",
        ],
        &[
            "nm0000002",
            "Akira Kurosawa",
            "1910",
            "1998",
            "director",
            "tt0000001",
        ],
        &[
            "nm0000003",
            "A Designer",
            "1900",
            &n,
            "production_designer",
            "tt0000001",
        ],
        &[
            "nm0000009",
            "Nobody At All",
            "1880",
            &n,
            "actor",
            "tt0000002",
        ],
    ])
}

struct Fx {
    _dir: tempfile::TempDir,
    principals: std::path::PathBuf,
    names: std::path::PathBuf,
}

async fn loaded_db() -> (Db, Fx) {
    let dir = tempfile::tempdir().expect("tempdir");
    let basics_path = write_gz(dir.path(), "b.tsv.gz", &basics());
    let ratings_path = write_gz(dir.path(), "r.tsv.gz", &ratings());
    let principals_path = write_gz(dir.path(), "p.tsv.gz", &principals());
    let names_path = write_gz(dir.path(), "n.tsv.gz", &names());

    let db = Db::in_memory().await.expect("open");
    let mut job = Job::begin(&db, "imdb").await.expect("job");
    load::load_titles(
        &mut job,
        basics_path,
        Arc::new(load::load_votes(&ratings_path).expect("votes")),
        Arc::new(load::load_average_ratings(&ratings_path).expect("averages")),
        CatalogueScope::DEFAULT,
    )
    .await
    .expect("titles");

    (
        db,
        Fx {
            _dir: dir,
            principals: principals_path,
            names: names_path,
        },
    )
}

async fn load_all(db: &Db, fx: &Fx) {
    let core = Arc::new(credits::core_title_ids(db).await.expect("core ids"));
    let needed = Arc::new(credits::scan_needed_people(&fx.principals, &core).expect("scan"));
    let mut job = Job::begin(db, "imdb-credits").await.expect("job");
    credits::load_people(&mut job, fx.names.clone(), needed)
        .await
        .expect("people");
    credits::load_credits(&mut job, fx.principals.clone(), core)
        .await
        .expect("credits");
}

#[tokio::test]
async fn only_core_titles_get_credits() {
    // The whole point of the two-tier scope: the obscure short is in the catalogue
    // and simply has no cast.
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;

    let with_credits: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT m.primary_title FROM media_items m
         JOIN credits c ON c.media_item_id = m.id",
    )
    .fetch_all(db.pool())
    .await
    .expect("query");

    assert_eq!(with_credits, vec!["Seven Samurai"]);
}

#[tokio::test]
async fn only_the_people_a_core_title_references_are_loaded() {
    // Loading all of name.basics would be most of a gigabyte of people who appear
    // in nothing that was kept.
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;

    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM people ORDER BY id")
        .fetch_all(db.pool())
        .await
        .expect("query");

    assert_eq!(names, vec!["Toshiro Mifune", "Akira Kurosawa"]);
    assert!(
        !names.iter().any(|n| n == "Nobody At All"),
        "a person appearing only in a non-core title is not loaded"
    );
    assert!(
        !names.iter().any(|n| n == "A Designer"),
        "nor is one whose only credit is an unmapped category"
    );
}

#[tokio::test]
async fn characters_are_unwrapped_from_imdbs_json_array() {
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;

    let character: String = sqlx::query_scalar(
        "SELECT character FROM credits WHERE role = 'actor' AND character != ''",
    )
    .fetch_one(db.pool())
    .await
    .expect("query");

    assert_eq!(character, "Kambei Shimada", "not the raw JSON array");
}

#[tokio::test]
async fn an_unmapped_category_is_dropped_rather_than_failing_the_batch() {
    // The CHECK constraint in migration 0002 would reject it and take 20,000 good
    // rows down with it.
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;

    let roles: Vec<String> = sqlx::query_scalar("SELECT DISTINCT role FROM credits ORDER BY role")
        .fetch_all(db.pool())
        .await
        .expect("query");
    assert_eq!(roles, vec!["actor", "director"]);
}

#[tokio::test]
async fn a_rerun_adds_nothing() {
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;
    let before = credits::counts(&db).await.expect("counts");

    load_all(&db, &fx).await;
    let after = credits::counts(&db).await.expect("counts");

    assert_eq!(before, after);
}

#[tokio::test]
async fn person_ids_are_the_imdb_number_so_credits_join_on_an_integer() {
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;

    let id: i64 = sqlx::query_scalar("SELECT id FROM people WHERE name = 'Toshiro Mifune'")
        .fetch_one(db.pool())
        .await
        .expect("query");
    assert_eq!(id, 1, "nm0000001 becomes 1");
}

#[tokio::test]
async fn billing_order_is_kept_so_a_cast_list_reads_correctly() {
    let (db, fx) = loaded_db().await;
    load_all(&db, &fx).await;

    let billing: Vec<i64> = sqlx::query_scalar(
        "SELECT billing FROM credits WHERE billing IS NOT NULL ORDER BY billing",
    )
    .fetch_all(db.pool())
    .await
    .expect("query");
    assert_eq!(billing, vec![1, 2], "IMDb's ordering column survives");
}
