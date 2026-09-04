//! The E5 fixture checker.
//!
//! Every SQL statement in `verify.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).

use sinephile_ingest::verify;
use sinephile_persistence::Db;

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    (dir, db)
}

/// A catalogue item carrying both ids, as the anime ingestion leaves it.
async fn mapped(db: &Db, title: &str, imdb: &str, anilist: Option<&str>) {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO media_items (kind, primary_title) VALUES ('anime_series', ?) RETURNING id",
    )
    .bind(title)
    .fetch_one(db.pool())
    .await
    .expect("insert");
    for (source, external) in [Some(("imdb", imdb)), anilist.map(|a| ("anilist", a))]
        .into_iter()
        .flatten()
    {
        sqlx::query(
            "INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(source)
        .bind(external)
        .execute(db.pool())
        .await
        .expect("insert id");
    }
}

fn fixture(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("e5.tsv");
    std::fs::write(
        &path,
        format!("# a comment\nanilist_id\timdb_id\texpect\ttitle\twhy\n{body}"),
    )
    .expect("write");
    path
}

#[tokio::test]
async fn a_correct_mapping_passes() {
    let (dir, db) = db().await;
    mapped(&db, "Cowboy Bebop", "tt0213338", Some("1")).await;

    let path = fixture(dir.path(), "1\ttt0213338\tmatch\tCowboy Bebop\tbaseline\n");
    let rows = verify::load(&path).expect("load");
    let outcomes = verify::run(&db, &rows).await.expect("run");

    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].passed);
    assert!(verify::report(&outcomes));
}

#[tokio::test]
async fn a_mapping_to_the_wrong_film_fails_and_says_which() {
    // The failure this whole fixture exists to catch: Fullmetal Alchemist (2003) and
    // Brotherhood (2009) mapped to each other would be invisible everywhere else.
    let (dir, db) = db().await;
    mapped(&db, "Fullmetal Alchemist", "tt1355642", Some("121")).await;

    let path = fixture(
        dir.path(),
        "121\ttt0421357\tmatch\tFullmetal Alchemist\tthe hardest pair\n",
    );
    let rows = verify::load(&path).expect("load");
    let outcomes = verify::run(&db, &rows).await.expect("run");

    assert!(!outcomes[0].passed);
    assert!(
        outcomes[0].detail.contains("tt0421357") && outcomes[0].detail.contains("tt1355642"),
        "the message must name both ids: {}",
        outcomes[0].detail
    );
    assert!(!verify::report(&outcomes), "reporting returns failure");
}

#[tokio::test]
async fn an_expected_refusal_passes_only_while_it_is_refused() {
    // If someone "improves" the matcher by breaking ties on popularity, this is the
    // row that goes red.
    let (dir, db) = db().await;
    mapped(&db, "Tokyo Ghoul", "tt3061046", None).await;

    let path = fixture(
        dir.path(),
        "20605\t-\tambiguous\tTokyo Ghoul\ttwo entries carry the title\n",
    );
    let rows = verify::load(&path).expect("load");
    assert!(verify::run(&db, &rows).await.expect("run")[0].passed);

    // Now let it be claimed, as a popularity tiebreak would.
    let id: i64 = sqlx::query_scalar("SELECT media_item_id FROM external_ids WHERE source='imdb'")
        .fetch_one(db.pool())
        .await
        .expect("id");
    sqlx::query("INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, 'anilist', '20605')")
        .bind(id)
        .execute(db.pool())
        .await
        .expect("claim");

    let outcomes = verify::run(&db, &rows).await.expect("run");
    assert!(!outcomes[0].passed, "a guessed match must fail the fixture");
    assert!(outcomes[0].detail.contains("tt3061046"));
}

#[tokio::test]
async fn an_unmapped_title_fails_rather_than_being_skipped() {
    // A row that quietly disappears is worse than one that fails: the fixture would
    // report 49/49 while covering 49 of 50 titles.
    let (dir, db) = db().await;
    let path = fixture(dir.path(), "1\ttt0213338\tmatch\tCowboy Bebop\tbaseline\n");
    let rows = verify::load(&path).expect("load");
    let outcomes = verify::run(&db, &rows).await.expect("run");

    assert_eq!(outcomes.len(), 1, "the row is still counted");
    assert!(!outcomes[0].passed);
    assert!(outcomes[0].detail.contains("nothing mapped"));
}

#[tokio::test]
async fn a_malformed_fixture_is_an_error_not_a_silent_skip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture(dir.path(), "1\ttt0213338\tnonsense\tCowboy Bebop\twhy\n");
    assert!(verify::load(&path).is_err(), "unknown expect value");

    let short = fixture(dir.path(), "1\ttt0213338\tmatch\n");
    assert!(verify::load(&short).is_err(), "missing columns");
}

#[tokio::test]
async fn the_committed_fixture_parses_and_covers_the_categories_e5_names() {
    // The real file, not a synthetic one — a fixture that stopped parsing would
    // otherwise only be discovered on a machine with the full catalogue.
    let path = std::path::Path::new("../../fixtures/anime/e5-hand-checked.tsv");
    let rows = verify::load(path).expect("the committed fixture parses");

    assert!(rows.len() >= 50, "E5 asks for 50, got {}", rows.len());
    assert!(
        rows.iter().any(|r| r.expect == verify::Expect::Ambiguous),
        "a fixture with no expected refusals cannot test the refusal design"
    );
    assert!(
        rows.iter().any(|r| r.expect == verify::Expect::Claimed),
        "split-cour seasons are a category E5 names"
    );

    let ids: std::collections::HashSet<i64> = rows.iter().map(|r| r.anilist_id).collect();
    assert_eq!(ids.len(), rows.len(), "an AniList id appears twice");

    for row in &rows {
        match row.expect {
            verify::Expect::Match => assert!(
                row.imdb_id.as_deref().is_some_and(|i| i.starts_with("tt")),
                "{} expects a match with no imdb id",
                row.title
            ),
            _ => assert!(
                row.imdb_id.is_none(),
                "{} expects a refusal but names an imdb id",
                row.title
            ),
        }
    }
}
