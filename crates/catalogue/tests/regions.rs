//! Phase 18 (ADR-0055): countries and movements asked of Wikidata, what profiles have met
//! first, then the most voted; every title asked is recorded, answered or not.

use sinephile_catalogue::current;
use sinephile_catalogue::regions::{self, Report};
use sinephile_metadata_api::wikipedia::Wikipedia;
use sinephile_metadata_api::{FakeTransport, Response, TransportError};
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository, RegionRepository};
use sinephile_persistence::{Db, IdSource, NewMediaItem};

async fn film(db: &Db, title: &str, votes: i64, imdb: &str) -> i64 {
    let media = MediaRepository::new(db);
    let mut item = NewMediaItem::film(title, 1990);
    item.rating_votes = Some(votes);
    let id = media.insert(&item).await.expect("insert");
    media
        .add_external_id(id, IdSource::Imdb, imdb, 1.0)
        .await
        .expect("imdb id");
    id
}

#[tokio::test]
async fn what_profiles_met_is_asked_first_and_every_answer_is_kept() {
    let db = Db::in_memory().await.expect("open");
    let taste = film(&db, "Taste of Cherry", 60_000, "tt0118849").await;
    let stalker = film(&db, "Stalker", 150_000, "tt0079944").await;
    let obscure = film(&db, "Seen by few", 300, "tt0000300").await;
    let unknown = film(&db, "Unknown to Wikidata", 5_000, "tt0000500").await;
    let me = ProfileRepository::new(&db)
        .create("me", true)
        .await
        .unwrap();
    ProfileRepository::new(&db)
        .rate(me, obscure, Some(10))
        .await
        .unwrap();

    let fake = FakeTransport::new();
    fake.push(Response::new(
        200,
        r#"{"results":{"bindings":[
            {"imdb":{"value":"tt0118849"},"country":{"value":"Iran"},"movement":{"value":"Iranian New Wave"}},
            {"imdb":{"value":"tt0079944"},"country":{"value":"Soviet Union"}},
            {"imdb":{"value":"tt0000300"},"country":{"value":"Sweden"}}
        ]}}"#,
    ));
    let wiki = Wikipedia::new(&fake).await;
    let report = regions::ask(&db, &wiki, 10).await.expect("ask");
    assert_eq!(
        report,
        Report {
            asked: 4,
            countries: 3,
            movements: 1,
            finished: true
        }
    );
    // One query, the met title first though it has 300 votes, then by votes.
    let asked = fake.last_request().expect("asked").url;
    let at = |imdb: &str| {
        asked
            .find(imdb)
            .unwrap_or_else(|| panic!("{imdb} in {asked}"))
    };
    assert!(at("tt0000300") < at("tt0079944") && at("tt0079944") < at("tt0118849"));
    assert!(at("tt0118849") < at("tt0000500"));

    let facts = MediaRepository::new(&db)
        .facts(&[taste, stalker, unknown])
        .await
        .unwrap();
    let of = |id: i64| facts.iter().find(|f| f.id == id).unwrap();
    assert_eq!(of(taste).movements, vec!["Iranian New Wave".to_string()]);
    assert_eq!(of(stalker).countries, vec!["Soviet Union".to_string()]);
    assert!(of(unknown).countries.is_empty());
    // The one Wikidata did not know is asked, and not asked again.
    let repo = RegionRepository::new(&db);
    assert!(repo.unasked(&[unknown, obscure]).await.unwrap().is_empty());
    let again = regions::ask(&db, &wiki, 10).await.expect("again");
    assert_eq!(again.asked, 0);
    assert_eq!(fake.request_count(), 1, "nothing left to ask");

    let sources = current::sources(&db).await.unwrap();
    let line = sources
        .iter()
        .find(|s| s.name == "Countries and movements")
        .unwrap();
    assert_eq!(
        line.detail,
        "4 titles asked of Wikidata: 3 with a country, 1 with a movement"
    );
    assert!(line.updated_at.is_some());
}

#[tokio::test]
async fn wikidata_stopping_leaves_the_rest_for_the_next_run() {
    let db = Db::in_memory().await.expect("open");
    let a = film(&db, "A", 60_000, "tt0000001").await;
    let fake = FakeTransport::new();
    fake.push_error(TransportError::Timeout);
    let wiki = Wikipedia::new(&fake).await;
    let report = regions::ask(&db, &wiki, 10).await.expect("ask");
    assert!(!report.finished);
    assert_eq!(report.asked, 0);
    assert_eq!(
        RegionRepository::new(&db).unasked(&[a]).await.unwrap(),
        [a].into_iter().collect()
    );
}
