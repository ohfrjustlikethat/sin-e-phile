//! ADR-0054 step 2: a launch gives the titles the catalogue gained a synopsis and a vector in
//! the companion graph, from the first source that can describe each.

use std::sync::Arc;

use sinephile_catalogue::embed::DocumentEmbedder;
use sinephile_catalogue::meaning::{self, Report, Sources};
use sinephile_catalogue::JobError;
use sinephile_metadata_api::anilist::AniList;
use sinephile_metadata_api::tmdb::Tmdb;
use sinephile_metadata_api::wikipedia::Wikipedia;
use sinephile_metadata_api::{FakeTransport, Response, Transport};
use sinephile_persistence::repositories::{MeaningCounts, MeaningRepository, MediaRepository};
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_vector_index::{added_path, Companion};

/// Obviously not a key: valid in shape only.
const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Deterministic unit vectors from the text.
struct Fake;

impl DocumentEmbedder for Fake {
    fn identity(&self) -> &str {
        "fake-model-int8"
    }

    fn dimension(&self) -> u16 {
        8
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let raw: Vec<f32> = (0..8u64)
            .map(|i| {
                let mut z = hash ^ (i + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z ^= z >> 31;
                (z >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
            })
            .collect();
        let norm = raw.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
        Ok(raw.into_iter().map(|v| v / norm).collect())
    }
}

fn long(what: &str) -> String {
    format!(
        "{what} {}",
        "It follows a long night across the city. ".repeat(9)
    )
}

async fn film(db: &Db, title: &str, votes: i64, imdb: Option<&str>, anilist: Option<&str>) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem::film(title, 2026))
        .await
        .expect("insert");
    sqlx::query("UPDATE media_items SET in_core = 1, rating_votes = ? WHERE id = ?")
        .bind(votes)
        .bind(id)
        .execute(db.pool())
        .await
        .expect("core");
    for (source, value) in [("imdb", imdb), ("anilist", anilist)] {
        if let Some(value) = value {
            sqlx::query(
                "INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, ?, ?)",
            )
            .bind(id)
            .bind(source)
            .bind(value)
            .execute(db.pool())
            .await
            .expect("external id");
        }
    }
    id
}

async fn synopsis(db: &Db, id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT synopsis FROM media_items WHERE id = ?")
        .bind(id)
        .fetch_one(db.pool())
        .await
        .expect("synopsis")
}

#[tokio::test]
async fn new_titles_are_described_by_the_first_source_that_can_and_embedded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    // In the published file already (described when it was made): never touched.
    let published = film(&db, "Published", 9_000, Some("tt0100000"), None).await;
    let wiki_film = film(&db, "Wiki Film", 600, Some("tt0100001"), None).await;
    let tmdb_film = film(&db, "Overview Film", 500, Some("tt0100005"), None).await;
    let anime = film(&db, "A Door to the Sea", 400, None, Some("2001")).await;
    let described = film(&db, "Described", 300, Some("tt0100003"), None).await;
    let short_lead = film(&db, "Short Lead", 250, Some("tt0100006"), None).await;
    let nobody = film(&db, "Nobody Wrote It Up", 200, Some("tt0100004"), None).await;
    // `described` has a synopsis already; `published` had one when the file was made.
    for (id, text) in [
        (described, "Already described."),
        (published, "In the file."),
    ] {
        sqlx::query("UPDATE media_items SET synopsis = ? WHERE id = ?")
            .bind(long(text))
            .bind(id)
            .execute(db.pool())
            .await
            .expect("synopsis");
    }

    // TMDB is asked first, for each undescribed title with an IMDb id, in vote order. The
    // fakes answer last pushed first.
    let tmdb_fake = Arc::new(FakeTransport::new());
    for _ in [nobody, short_lead] {
        tmdb_fake.push(Response::new(
            200,
            r#"{"movie_results":[],"tv_results":[]}"#,
        ));
    }
    // `long` has no quotes or backslashes, so it goes into JSON as it is.
    tmdb_fake.push(Response::new(
        200,
        format!(
            r#"{{"movie_results":[{{"poster_path":null,"overview":"{}"}}]}}"#,
            long("An overview.")
        ),
    ));
    tmdb_fake.push(Response::new(
        200,
        r#"{"movie_results":[{"poster_path":null,"overview":"Too short."}]}"#,
    ));
    let anilist_fake = FakeTransport::new();
    anilist_fake.push(Response::new(
        200,
        format!(
            r#"{{"data":{{"Page":{{"media":[{{"id":2001,"description":"A boy finds a door.<br><br>{}"}}]}}}}}}"#,
            long("")
        ),
    ));
    // Wikipedia: the mappings, the leads, then the opening of the article whose lead was a
    // sentence (asked last, so pushed first).
    let wiki_fake = FakeTransport::new();
    wiki_fake.push(Response::new(
        200,
        format!(
            r#"{{"query":{{"pages":[{{"title":"Short Lead","extract":"{}"}}]}}}}"#,
            long("Short Lead is a 2026 film. Plot.")
        ),
    ));
    wiki_fake.push(Response::new(
        200,
        format!(
            r#"{{"batchcomplete":true,"query":{{"pages":[{{"title":"Wiki Film","extract":"{}"}},{{"title":"Short Lead","extract":"Short Lead is a 2026 film."}}]}}}}"#,
            long("Wiki Film is a 2026 film.")
        ),
    ));
    wiki_fake.push(Response::new(
        200,
        r#"{"results":{"bindings":[
            {"imdb":{"value":"tt0100001"},"article":{"value":"https://en.wikipedia.org/wiki/Wiki_Film"}},
            {"imdb":{"value":"tt0100006"},"article":{"value":"https://en.wikipedia.org/wiki/Short_Lead"}}
        ]}}"#,
    ));

    let tmdb = Tmdb::new(tmdb_fake.clone() as Arc<dyn Transport>, KEY);
    let anilist = AniList::new(&anilist_fake).await;
    let wikipedia = Wikipedia::new(&wiki_fake).await;
    let sources = Sources {
        tmdb: Some(&tmdb),
        anilist: Some(&anilist),
        wikipedia: Some(&wikipedia),
    };
    let index_path = dir.path().join("vector-index-fake.usearch");
    let mut companion = Companion::open(&index_path, 8).expect("companion");

    let report = meaning::keep_current(&db, &mut Fake, &mut companion, published, &sources, 100)
        .await
        .expect("keep current");
    assert_eq!(
        report,
        Report {
            waiting: 6,
            asked: 5,
            tmdb: 1,
            anilist: 1,
            wikipedia: 2,
            embedded: 5,
        }
    );
    for id in [wiki_film, tmdb_film, anime, described, short_lead] {
        assert!(companion.contains(id), "{id} has its vector");
    }
    assert!(!companion.contains(nobody), "nothing to embed");
    assert!(!companion.contains(published), "the published file has it");
    assert!(
        added_path(&index_path).is_file(),
        "saved beside the published graph"
    );

    // Wikipedia's lead, not TMDB's too-short overview; AniList's text without its tags.
    // A lead too short to index, then the article's opening, which is long enough.
    assert!(synopsis(&db, short_lead)
        .await
        .unwrap()
        .starts_with("Short Lead is a 2026 film. Plot."));
    assert_eq!(wiki_fake.requests.lock().unwrap().len(), 3);
    assert!(synopsis(&db, wiki_film)
        .await
        .unwrap()
        .starts_with("Wiki Film is"));
    assert!(synopsis(&db, tmdb_film)
        .await
        .unwrap()
        .starts_with("An overview."));
    assert!(synopsis(&db, anime)
        .await
        .unwrap()
        .starts_with("A boy finds a door. "));
    assert_eq!(synopsis(&db, nobody).await, None);
    // Wikidata was asked only about the two TMDB could not describe.
    let wdqs = wiki_fake.requests.lock().unwrap()[0].url.clone();
    assert!(
        wdqs.contains("tt0100001") && wdqs.contains("tt0100004"),
        "{wdqs}"
    );
    assert!(
        !wdqs.contains("tt0100005") && !wdqs.contains("tt0100003"),
        "{wdqs}"
    );
    assert_eq!(
        MeaningRepository::new(&db).counts().await.unwrap(),
        MeaningCounts {
            asked: 5,
            described: 4,
            embedded: 5
        }
    );

    // The next launch: nothing waits (the undescribed one was asked this week), nothing is
    // asked.
    let quiet = FakeTransport::new();
    let anilist = AniList::new(&quiet).await;
    let wikipedia = Wikipedia::new(&quiet).await;
    let sources = Sources {
        tmdb: None,
        anilist: Some(&anilist),
        wikipedia: Some(&wikipedia),
    };
    let again = meaning::keep_current(&db, &mut Fake, &mut companion, published, &sources, 100)
        .await
        .expect("again");
    assert_eq!(again, Report::default());
    assert!(quiet.requests.lock().unwrap().is_empty());

    // The companion lost (deleted): every described title is embedded again, offline.
    std::fs::remove_file(added_path(&index_path)).expect("delete");
    let mut fresh = Companion::open(&index_path, 8).expect("fresh");
    let rebuilt = meaning::keep_current(
        &db,
        &mut Fake,
        &mut fresh,
        published,
        &Sources::default(),
        100,
    )
    .await
    .expect("rebuilt");
    assert_eq!(rebuilt.embedded, 5);
    assert_eq!(rebuilt.asked, 0, "no source, nobody asked");
    assert_eq!(fresh.len(), 5);
}

#[tokio::test]
async fn a_launch_takes_the_most_voted_first_and_leaves_the_rest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let mut ids = Vec::new();
    for k in 0..5 {
        let id = film(&db, &format!("Film {k}"), 100 * k, None, None).await;
        sqlx::query("UPDATE media_items SET synopsis = ? WHERE id = ?")
            .bind(long(&format!("Film {k}.")))
            .bind(id)
            .execute(db.pool())
            .await
            .expect("synopsis");
        ids.push(id);
    }
    let path = dir.path().join("vector-index-fake.usearch");
    let mut companion = Companion::open(&path, 8).expect("companion");
    let first = meaning::keep_current(&db, &mut Fake, &mut companion, 0, &Sources::default(), 3)
        .await
        .expect("first");
    assert_eq!(first.embedded, 3);
    assert!(companion.contains(ids[4]) && companion.contains(ids[3]) && companion.contains(ids[2]));
    assert!(!companion.contains(ids[0]));
    let second = meaning::keep_current(&db, &mut Fake, &mut companion, 0, &Sources::default(), 3)
        .await
        .expect("second");
    assert_eq!(second.embedded, 2, "the rest, the next launch");
    assert_eq!(companion.len(), 5);
}
