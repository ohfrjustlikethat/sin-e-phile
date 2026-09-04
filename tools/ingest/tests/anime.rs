//! AniList → catalogue ingestion.
//!
//! Every SQL statement in `anime.rs` runs against a migrated database here
//! (ADR-0026's standing requirement), and the matcher's refusals are asserted as
//! outcomes rather than treated as absence — which is what exit criterion E5
//! measures.

use std::sync::Arc;

use sinephile_ingest::{anime, Job};
use sinephile_metadata_api::{AniList, FakeTransport, Response, Transport};
use sinephile_persistence::Db;

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    (dir, db)
}

/// Put a catalogue item in with its normalised title already written, the way
/// `ingest normalise` leaves it.
async fn item(db: &Db, kind: &str, title: &str, year: Option<i64>) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO media_items (kind, primary_title, release_year) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(kind)
    .bind(title)
    .bind(year)
    .fetch_one(db.pool())
    .await
    .expect("insert item");

    sqlx::query(
        "INSERT INTO titles (media_item_id, title, variant, normalised) VALUES (?, ?, 'primary', ?)",
    )
    .bind(id)
    .bind(title)
    .bind(sinephile_ingest::matching::normalise(title))
    .execute(db.pool())
    .await
    .expect("insert title");

    id
}

/// One AniList page, as the GraphQL endpoint would return it.
fn page(has_next: bool, media: &[String]) -> Response {
    Response::new(
        200,
        format!(
            r#"{{"data":{{"Page":{{"pageInfo":{{"hasNextPage":{has_next}}},"media":[{}]}}}}}}"#,
            media.to_vec().join(",")
        ),
    )
}

fn media(
    id: i64,
    id_mal: Option<i64>,
    romaji: &str,
    english: &str,
    native: &str,
    format: &str,
    year: Option<i64>,
) -> String {
    let id_mal = id_mal.map(|m| m.to_string()).unwrap_or("null".into());
    let year = year.map(|y| y.to_string()).unwrap_or("null".into());
    format!(
        r#"{{"id":{id},"idMal":{id_mal},"title":{{"romaji":"{romaji}","english":"{english}","native":"{native}"}},"format":"{format}","status":"FINISHED","episodes":26,"seasonYear":{year},"nextAiringEpisode":null}}"#
    )
}

/// A client owning the fake, plus the handle that records what it was asked.
fn client(transport: FakeTransport) -> Arc<dyn Transport> {
    Arc::new(transport)
}

#[tokio::test]
async fn a_matched_title_is_promoted_and_carries_both_ids() {
    let (_dir, db) = db().await;
    let bebop = item(&db, "series", "Cowboy Bebop", Some(1998)).await;

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            1,
            Some(1),
            "Cowboy Bebop",
            "Cowboy Bebop",
            "カウボーイビバップ",
            "TV",
            Some(1998),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");
    job.finish().await.expect("finish");

    assert_eq!(report.seen, 1);
    assert_eq!(report.matched, 1, "an exact title and year should match");

    // The kind is the whole point: IMDb cannot tell anime from any other animation.
    let kind: String = sqlx::query_scalar("SELECT kind FROM media_items WHERE id = ?")
        .bind(bebop)
        .fetch_one(db.pool())
        .await
        .expect("kind");
    assert_eq!(kind, "anime_series");

    // MAL's id comes free with AniList's, and nothing else supplies one.
    let ids: Vec<(String, String)> = sqlx::query_as(
        "SELECT source, external_id FROM external_ids WHERE media_item_id = ? ORDER BY source",
    )
    .bind(bebop)
    .fetch_all(db.pool())
    .await
    .expect("ids");
    assert_eq!(
        ids,
        vec![
            ("anilist".to_string(), "1".to_string()),
            ("mal".to_string(), "1".to_string())
        ]
    );

    // The native title is written as an asserted fact, with a normalised form — a
    // NULL one would be invisible to every later match.
    let native: (String, Option<String>) = sqlx::query_as(
        "SELECT title, normalised FROM titles WHERE media_item_id = ? AND variant = 'native'",
    )
    .bind(bebop)
    .fetch_one(db.pool())
    .await
    .expect("native title");
    assert_eq!(native.0, "カウボーイビバップ");
    assert!(
        native.1.is_some(),
        "a title written now must be matchable now"
    );
}

#[tokio::test]
async fn a_film_becomes_anime_film_not_anime_series() {
    let (_dir, db) = db().await;
    let id = item(&db, "film", "Akira", Some(1988)).await;

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            47,
            Some(47),
            "Akira",
            "Akira",
            "アキラ",
            "MOVIE",
            Some(1988),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    let kind: String = sqlx::query_scalar("SELECT kind FROM media_items WHERE id = ?")
        .bind(id)
        .fetch_one(db.pool())
        .await
        .expect("kind");
    assert_eq!(
        kind, "anime_film",
        "format MOVIE decides this, not the title"
    );
}

#[tokio::test]
async fn two_equally_good_candidates_are_refused_rather_than_guessed() {
    // The real catalogue has 568 items normalising to "home" and 486 to "alone".
    // Picking one would be wrong hundreds of times and would record each as a fact.
    let (_dir, db) = db().await;
    // Both series, both 2004: nothing distinguishes them, so nothing may choose.
    item(&db, "series", "Monster", Some(2004)).await;
    item(&db, "series", "Monster", Some(2004)).await;

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            19,
            Some(19),
            "Monster",
            "Monster",
            "モンスター",
            "TV",
            Some(2004),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(report.matched, 0);
    assert_eq!(report.ambiguous, 1);
    assert_eq!(report.unmatched.len(), 1, "E5 hand-checks these by name");

    // Nothing was written. An ambiguous match that promoted one of the two would be
    // invisible afterwards.
    let promoted: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE kind LIKE 'anime%'")
            .fetch_one(db.pool())
            .await
            .expect("count");
    assert_eq!(promoted, 0);
}

#[tokio::test]
async fn several_spellings_of_one_title_are_one_candidate_not_several() {
    // The catalogue carries a title row per spelling, and all of them normalise to the
    // same key. Counting rows rather than items made Death Note, One Piece, Naruto and
    // Attack on Titan all "ambiguous" against themselves — 97 of 250 refusals.
    let (_dir, db) = db().await;
    let id = item(&db, "series", "Death Note", Some(2006)).await;
    for spelling in ["DEATH NOTE", "Death note", "Death-Note"] {
        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, normalised)
             VALUES (?, ?, 'alternative', ?)",
        )
        .bind(id)
        .bind(spelling)
        .bind(sinephile_ingest::matching::normalise(spelling))
        .execute(db.pool())
        .await
        .expect("insert spelling");
    }

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            1535,
            Some(1535),
            "DEATH NOTE",
            "Death Note",
            "DEATH NOTE",
            "TV",
            Some(2006),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(
        report.matched, 1,
        "four spellings of one item are one candidate"
    );
    assert_eq!(report.ambiguous, 0);
}

#[tokio::test]
async fn the_anilist_format_breaks_a_tie_it_cannot_create() {
    // "Naruto" is one series and two unrelated films with no year at all. AniList says
    // TV, and that is a fact we already hold rather than a preference.
    let (_dir, db) = db().await;
    let series = item(&db, "series", "Naruto", Some(2002)).await;
    item(&db, "film", "Naruto", None).await;
    item(&db, "film", "Naruto", None).await;

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            20,
            Some(20),
            "NARUTO",
            "Naruto",
            "NARUTO",
            "TV",
            Some(2002),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(report.matched, 1);
    let claimed: i64 = sqlx::query_scalar(
        "SELECT media_item_id FROM external_ids WHERE source = 'anilist' AND external_id = '20'",
    )
    .fetch_one(db.pool())
    .await
    .expect("claimed");
    assert_eq!(claimed, series, "the series, not either film");
}

#[tokio::test]
async fn the_format_cannot_break_a_tie_between_two_of_the_same_shape() {
    // Two films, and AniList says MOVIE. The shape agrees with both, so it settles
    // nothing and the refusal stands.
    let (_dir, db) = db().await;
    item(&db, "film", "Gamera", Some(1995)).await;
    item(&db, "film", "Gamera", Some(1995)).await;

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            999,
            None,
            "Gamera",
            "Gamera",
            "\\u30ac\\u30e1\\u30e9",
            "MOVIE",
            Some(1995),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(report.matched, 0);
    assert_eq!(report.ambiguous, 1);
}

#[tokio::test]
async fn a_second_season_does_not_overwrite_the_first() {
    // AniList lists every season separately; IMDb lists one series, dated by its
    // FIRST air year. So "Fruits Basket 2nd Season" (2020) season-strips onto IMDb's
    // "Fruits Basket" (2019), one year apart and so inside the year tolerance —
    // correctly, it is the same show.
    //
    // But the schema holds one AniList id per item, so a naive write would replace
    // season 1's id AND add season 2's name as another romaji title of the series.
    //
    // (Seasons further apart never get here: Attack on Titan's season 2 is four years
    // after its first, and the year rule refuses it before this check is reached.)
    let (_dir, db) = db().await;
    let id = item(&db, "series", "Fruits Basket", Some(2019)).await;

    let transport = FakeTransport::new();
    // FakeTransport pops from the end, so the first page is queued last.
    transport.push(page(
        false,
        &[media(
            110354,
            Some(39533),
            "Fruits Basket 2nd Season",
            "Fruits Basket 2nd Season",
            "フルーツバスケット 2nd season",
            "TV",
            Some(2020),
        )],
    ));
    transport.push(page(
        true,
        &[media(
            105334,
            Some(38680),
            "Fruits Basket",
            "Fruits Basket",
            "フルーツバスケット",
            "TV",
            Some(2019),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(2), None)
        .await
        .expect("ingest");

    assert_eq!(report.matched, 1);
    assert_eq!(
        report.already_claimed, 1,
        "season 2 resolves, but cannot claim"
    );

    let anilist_id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_ids WHERE media_item_id = ? AND source = 'anilist'",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .expect("id");
    assert_eq!(anilist_id, "105334", "the popular entry keeps the mapping");

    let romaji: Vec<String> = sqlx::query_scalar(
        "SELECT title FROM titles WHERE media_item_id = ? AND variant = 'romaji'",
    )
    .bind(id)
    .fetch_all(db.pool())
    .await
    .expect("romaji");
    assert_eq!(
        romaji,
        vec!["Fruits Basket".to_string()],
        "season 2 must not add itself as another name for the series"
    );
}

#[tokio::test]
async fn a_year_conflict_is_reported_separately_from_absence() {
    // Same title, wrong decade: almost always a different entry in a franchise, and a
    // different problem from a title the catalogue has never heard of.
    let (_dir, db) = db().await;
    item(&db, "series", "Fullmetal Alchemist", Some(2003)).await;

    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            5114,
            Some(5114),
            "Fullmetal Alchemist",
            "Fullmetal Alchemist",
            "鋼の錬金術師",
            "TV",
            Some(2009),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(report.matched, 0);
    assert_eq!(report.year_conflict, 1);
    assert_eq!(
        report.not_in_catalogue, 0,
        "it IS in the catalogue, as a different entry"
    );
}

#[tokio::test]
async fn an_interrupted_run_resumes_at_the_next_page() {
    let (_dir, db) = db().await;
    item(&db, "series", "Cowboy Bebop", Some(1998)).await;
    item(&db, "film", "Akira", Some(1988)).await;

    // First run: one page, then the transport runs dry, which fails the step.
    let transport = FakeTransport::new();
    transport.push(page(
        true,
        &[media(
            1,
            Some(1),
            "Cowboy Bebop",
            "Cowboy Bebop",
            "カウボーイビバップ",
            "TV",
            Some(1998),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    let first = anime::ingest(&mut job, anilist, None, None).await;
    assert!(first.is_err(), "running out of pages mid-run is a failure");

    // Second run: the checkpoint says page 1 is done, so this must ask for page 2.
    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            47,
            Some(47),
            "Akira",
            "Akira",
            "アキラ",
            "MOVIE",
            Some(1988),
        )],
    ));
    let requests = Arc::clone(&transport.requests);

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("resume");
    assert!(job.is_resuming().await.expect("resuming"));
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(report.seen, 1, "page 1 is not re-fetched");
    let body = requests.lock().expect("lock")[0]
        .body
        .clone()
        .expect("graphql body");
    assert!(
        body.contains(r#""page":2"#),
        "resumed at page 2, got: {body}"
    );
}

#[tokio::test]
async fn every_unmatched_entry_is_written_to_the_hand_check_file() {
    // E5 hand-checks fifty titles. A count in a terminal that has since scrolled away
    // is not something anyone can check, so the list is a file.
    let (_dir, db) = db().await;
    item(&db, "series", "Monster", Some(2004)).await;
    item(&db, "series", "Monster", Some(2004)).await;

    let out = _dir.path().join("unmatched.tsv");
    let transport = FakeTransport::new();
    transport.push(page(
        false,
        &[media(
            19,
            Some(19),
            "Monster",
            "Monster",
            "モンスター",
            "TV",
            Some(2004),
        )],
    ));

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    anime::ingest(&mut job, anilist, Some(1), Some(&out))
        .await
        .expect("ingest");

    let written = std::fs::read_to_string(&out).expect("unmatched file");
    let mut lines = written.lines();
    assert_eq!(
        lines.next().expect("header"),
        "anilist_id\tromaji\tenglish\tnative\tyear\tformat\treason"
    );
    let row = lines.next().expect("one unmatched row");
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(fields[0], "19");
    assert_eq!(fields[1], "Monster");
    assert_eq!(
        fields[3], "モンスター",
        "the native form is what a human checks"
    );
    assert_eq!(fields[4], "2004");
    assert!(fields[6].starts_with("ambiguous"), "got {:?}", fields[6]);
    assert!(lines.next().is_none(), "one entry, one row");
}

#[tokio::test]
async fn the_printed_sample_is_spread_not_truncated() {
    // The sweep is year-ascending, so taking the first n unmatched entries samples the
    // sweep order rather than the catalogue — every one of them from the 1950s.
    let report = anime::Report {
        unmatched: (0..100)
            .map(|i| anime::Unmatched {
                anilist_id: i,
                romaji: format!("Title {i}"),
                english: String::new(),
                native: String::new(),
                year: Some(1950 + i),
                format: "TV".into(),
                reason: "not in catalogue".into(),
            })
            .collect(),
        ..Default::default()
    };

    let spread = report.unmatched_spread(5);
    let ids: Vec<i64> = spread.iter().map(|u| u.anilist_id).collect();
    assert_eq!(ids, vec![0, 20, 40, 60, 80]);

    // Asking for more than there are returns everything, rather than panicking or
    // repeating entries to pad the list.
    assert_eq!(report.unmatched_spread(500).len(), 100);
}

#[tokio::test]
async fn a_finished_year_advances_to_the_next_one() {
    // AniList refuses to paginate past 5,000 entries, so the sweep is partitioned by
    // seasonYear. A year that runs out must move to the next one — NOT end the sweep,
    // because most years before the 1960s are empty and stopping at the first empty
    // year would stop at 1940.
    let (_dir, db) = db().await;

    let transport = FakeTransport::new();
    // Queued last-first: two empty years in a row.
    transport.push(page(false, &[]));
    transport.push(page(false, &[]));
    let requests = Arc::clone(&transport.requests);

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    anime::ingest(&mut job, anilist, Some(2), None)
        .await
        .expect("ingest");

    let years: Vec<String> = requests
        .lock()
        .expect("lock")
        .iter()
        .map(|r| {
            let body = r.body.clone().expect("graphql body");
            let at = body
                .find("\"seasonYear\":")
                .expect("seasonYear in variables");
            body[at + 13..].trim_end_matches('}').to_string()
        })
        .collect();
    assert_eq!(years, vec!["1940".to_string(), "1941".to_string()]);
}

#[tokio::test]
async fn max_pages_bounds_a_run() {
    let (_dir, db) = db().await;
    item(&db, "series", "Cowboy Bebop", Some(1998)).await;

    let transport = FakeTransport::new();
    transport.push(page(
        true,
        &[media(
            1,
            Some(1),
            "Cowboy Bebop",
            "Cowboy Bebop",
            "カウボーイビバップ",
            "TV",
            Some(1998),
        )],
    ));
    let requests = Arc::clone(&transport.requests);

    let anilist = Arc::new(AniList::owned(client(transport)).await);
    let mut job = Job::begin(&db, "anilist").await.expect("begin");
    // hasNextPage is true, so only the bound stops this — and the transport has just
    // one response queued, so an unbounded run would error rather than pass.
    let report = anime::ingest(&mut job, anilist, Some(1), None)
        .await
        .expect("ingest");

    assert_eq!(report.seen, 1);
    assert_eq!(requests.lock().expect("lock").len(), 1);
}
