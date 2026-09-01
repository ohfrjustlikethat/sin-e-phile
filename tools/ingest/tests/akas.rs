//! Alternative title loading.
//!
//! Every SQL statement in `akas.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).

use std::io::Write;
use std::sync::Arc;

use sinephile_ingest::imdb::CatalogueScope;
use sinephile_ingest::{akas, credits, load, Job};
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

fn tsv(rows: &[&[&str]]) -> String {
    rows.iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// tt1 is core; tt2 is indexed but not core.
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
            "Seven Samurai",
            "0",
            "1954",
            &n,
            "207",
            "Action",
        ],
        &[
            "tt0000002",
            "short",
            "Obscure",
            "Obscure",
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

fn akas_tsv() -> String {
    let n = null();
    tsv(&[
        &[
            "titleId",
            "ordering",
            "title",
            "region",
            "language",
            "types",
            "attributes",
            "isOriginalTitle",
        ],
        // Native script, asserted original.
        &["tt0000001", "1", "七人の侍", "JP", "ja", &n, &n, "1"],
        // The same film transliterated — the §6.2 case.
        &[
            "tt0000001",
            "2",
            "Shichinin no samurai",
            &n,
            "ja",
            &n,
            &n,
            "0",
        ],
        &["tt0000001", "3", "Seven Samurai", "US", "en", &n, &n, "0"],
        &[
            "tt0000001",
            "4",
            "Les sept samouraïs",
            "FR",
            "fr",
            &n,
            &n,
            "0",
        ],
        // Belongs to a NON-core title — must not be loaded.
        &["tt0000002", "1", "Une obscurité", "FR", "fr", &n, &n, "0"],
    ])
}

struct Fx {
    _dir: tempfile::TempDir,
    akas: std::path::PathBuf,
}

async fn loaded() -> (Db, Fx) {
    let dir = tempfile::tempdir().expect("tempdir");
    let basics_path = write_gz(dir.path(), "b.tsv.gz", &basics());
    let ratings_path = write_gz(dir.path(), "r.tsv.gz", &ratings());
    let akas_path = write_gz(dir.path(), "a.tsv.gz", &akas_tsv());

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
            akas: akas_path,
        },
    )
}

async fn load_akas(db: &Db, fx: &Fx) {
    let core = Arc::new(credits::core_title_ids(db).await.expect("core"));
    let mut job = Job::begin(db, "imdb-akas").await.expect("job");
    akas::load_akas(&mut job, fx.akas.clone(), core)
        .await
        .expect("akas");
}

async fn variants_for(db: &Db, external_id: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT t.variant, t.title FROM titles t
         JOIN external_ids e ON e.media_item_id = t.media_item_id
         WHERE e.external_id = ? ORDER BY t.variant, t.title",
    )
    .bind(external_id)
    .fetch_all(db.pool())
    .await
    .expect("query")
}

#[tokio::test]
async fn romaji_and_native_are_distinguished_by_script() {
    // SPEC.md §6.2 names romaji, native and english explicitly, and IMDb has no
    // romaji flag — so the script of the text decides.
    let (db, fx) = loaded().await;
    load_akas(&db, &fx).await;

    let variants = variants_for(&db, "tt0000001").await;
    assert!(
        variants.contains(&("romaji".into(), "Shichinin no samurai".into())),
        "the transliteration is romaji: {variants:?}"
    );
    assert!(
        variants.contains(&("original".into(), "七人の侍".into())),
        "isOriginalTitle outranks the script guess: {variants:?}"
    );
    assert!(variants.contains(&("english".into(), "Seven Samurai".into())));
    assert!(variants.contains(&("alternative".into(), "Les sept samouraïs".into())));
}

#[tokio::test]
async fn a_non_core_title_gets_no_alternative_titles() {
    let (db, fx) = loaded().await;
    load_akas(&db, &fx).await;

    let variants = variants_for(&db, "tt0000002").await;
    assert!(
        !variants.iter().any(|(_, t)| t == "Une obscurité"),
        "an aka for a non-core title must not be loaded: {variants:?}"
    );
    // It keeps the primary title the index-tier load gave it.
    assert_eq!(variants.len(), 1);
}

#[tokio::test]
async fn a_rerun_adds_nothing() {
    // idx_titles_unique is (media_item_id, variant, language, region); the insert
    // relies on ON CONFLICT DO NOTHING against it.
    let (db, fx) = loaded().await;
    load_akas(&db, &fx).await;
    let first: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM titles")
        .fetch_one(db.pool())
        .await
        .expect("count");

    load_akas(&db, &fx).await;
    let second: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM titles")
        .fetch_one(db.pool())
        .await
        .expect("count");

    assert_eq!(first, second);
}

#[tokio::test]
async fn the_language_and_region_survive() {
    // Phase 9's language selector reads these, so they cannot be dropped on the way
    // in.
    let (db, fx) = loaded().await;
    load_akas(&db, &fx).await;

    let row: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT language, region FROM titles WHERE title = 'Les sept samouraïs'")
            .fetch_one(db.pool())
            .await
            .expect("query");
    assert_eq!(row, (Some("fr".into()), Some("FR".into())));
}

#[tokio::test]
async fn the_same_text_is_stored_once_per_variant_whatever_the_region() {
    // Migration 0007. The old index was (item, variant, language, region), so
    // "Seven Samurai" under en/US, en/GB and a null region was three distinct keys
    // and all three were stored — 27% of the real catalogue's title rows carried no
    // information. A genuinely DIFFERENT regional title is different text and is
    // still its own row.
    let dir = tempfile::tempdir().expect("tempdir");
    let n = null();
    let akas = write_gz(
        dir.path(),
        "a.tsv.gz",
        &tsv(&[
            &[
                "titleId",
                "ordering",
                "title",
                "region",
                "language",
                "types",
                "attributes",
                "isOriginalTitle",
            ],
            &["tt0000001", "1", "Seven Samurai", "US", "en", &n, &n, "0"],
            &["tt0000001", "2", "Seven Samurai", "GB", "en", &n, &n, "0"],
            &["tt0000001", "3", "Seven Samurai", "CA", "en", &n, &n, "0"],
            &["tt0000001", "4", "Seven Samurai", &n, "en", &n, &n, "0"],
            // A genuinely different UK title must survive as its own row.
            &[
                "tt0000001",
                "5",
                "The Magnificent Seven",
                "GB",
                "en",
                &n,
                &n,
                "0",
            ],
        ]),
    );

    let db = Db::in_memory().await.expect("open");
    let mut job = Job::begin(&db, "imdb").await.expect("job");
    let basics_path = write_gz(dir.path(), "b.tsv.gz", &basics());
    let ratings_path = write_gz(dir.path(), "r.tsv.gz", &ratings());
    load::load_titles(
        &mut job,
        basics_path,
        Arc::new(load::load_votes(&ratings_path).expect("votes")),
        Arc::new(load::load_average_ratings(&ratings_path).expect("averages")),
        CatalogueScope::DEFAULT,
    )
    .await
    .expect("titles");

    let core = Arc::new(credits::core_title_ids(&db).await.expect("core"));
    let mut job = Job::begin(&db, "imdb-akas").await.expect("job");
    akas::load_akas(&mut job, akas, core).await.expect("akas");

    let english: Vec<String> = sqlx::query_scalar(
        "SELECT t.title FROM titles t
         JOIN external_ids e ON e.media_item_id = t.media_item_id
         WHERE e.external_id = 'tt0000001' AND t.variant = 'english'
         ORDER BY t.title",
    )
    .fetch_all(db.pool())
    .await
    .expect("query");

    assert_eq!(
        english,
        vec!["Seven Samurai", "The Magnificent Seven"],
        "four identical rows collapse to one; a different title survives"
    );
}
