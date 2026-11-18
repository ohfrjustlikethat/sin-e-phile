//! ADR-0054 step 7: Settings says, per source, when it last updated and what it did.

use sinephile_catalogue::current::{self, Source};
use sinephile_persistence::repositories::{
    MeaningRepository, MediaRepository, NeighbourRepository, NeighbourSource, ProfileRepository,
};
use sinephile_persistence::{Db, NewMediaItem};

fn named<'a>(all: &'a [Source], name: &str) -> &'a Source {
    all.iter().find(|s| s.name == name).expect(name)
}

#[tokio::test]
async fn each_source_says_when_it_last_updated_and_what_it_did() {
    let db = Db::in_memory().await.expect("open");

    // Before anything has run: every source says what it is waiting for.
    let before = current::sources(&db).await.expect("sources");
    let names: Vec<&str> = before.iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        [
            "IMDb's catalogue",
            "Meaning for new titles",
            "MovieLens",
            "AniList's recommendations",
            "TMDB's recommendations",
            "Rising now"
        ]
    );
    assert!(before.iter().all(|s| s.updated_at.is_none()), "{before:?}");
    assert_eq!(
        named(&before, "TMDB's recommendations").detail,
        "Used with a profile's own TMDB key; none is set"
    );
    assert!(named(&before, "MovieLens")
        .detail
        .starts_with("Not built yet"));

    // Each job's record, as the jobs write them.
    let profiles = ProfileRepository::new(&db);
    profiles
        .set_setting(
            sinephile_catalogue::freshness::LAST_REFRESH,
            r#"{"at":"2026-11-18T06:00:00Z","titles_added":1204,"episodes_added":87}"#,
        )
        .await
        .unwrap();
    profiles
        .set_setting(
            sinephile_catalogue::meaning::LAST_KEY,
            r#"{"waiting":2000,"asked":1900,"tmdb":0,"anilist":69,"wikipedia":119,"embedded":188,"total":374}"#,
        )
        .await
        .unwrap();
    profiles
        .set_setting(
            sinephile_catalogue::live::LAST_KEY,
            r#"{"anilist_asked":2,"anilist_rows":16,"tmdb_asked":0,"tmdb_rows":0,"tmdb":true}"#,
        )
        .await
        .unwrap();
    let media = MediaRepository::new(&db);
    let mut ids = Vec::new();
    for k in 0..3 {
        ids.push(
            media
                .insert(&NewMediaItem::film(format!("Film {k}"), 2026))
                .await
                .unwrap(),
        );
    }
    let meaning = MeaningRepository::new(&db);
    meaning.embedded(&ids).await.unwrap();
    let neighbours = NeighbourRepository::new(&db);
    neighbours
        .replace_for(ids[0], NeighbourSource::AniList, &[(ids[1], 0.3)])
        .await
        .unwrap();
    neighbours
        .replace_all(&[(ids[1], ids[2], 0.3), (ids[2], ids[1], 0.2)])
        .await
        .unwrap();
    profiles
        .set_setting(current::MOVIELENS_KEY, r#"{"titles":2,"rows":2}"#)
        .await
        .unwrap();

    let after = current::sources(&db).await.expect("sources");
    let imdb = named(&after, "IMDb's catalogue");
    assert_eq!(imdb.updated_at.as_deref(), Some("2026-11-18T06:00:00Z"));
    assert_eq!(imdb.detail, "1,204 new titles and 87 new episodes");
    let meaning_line = named(&after, "Meaning for new titles");
    assert_eq!(
        meaning_line.detail,
        "188 titles given meaning at the last update, 3 in all"
    );
    // A setting's write time, as ISO 8601 UTC.
    let at = meaning_line.updated_at.as_deref().expect("written");
    assert!(
        at.len() == 20 && at.contains('T') && at.ends_with('Z'),
        "{at}"
    );
    assert_eq!(
        named(&after, "MovieLens").detail,
        "2 titles with neighbours; rebuilt when GroupLens publishes a new release"
    );
    assert!(named(&after, "MovieLens").updated_at.is_some());
    let anilist = named(&after, "AniList's recommendations");
    assert!(anilist.updated_at.is_some());
    assert_eq!(
        anilist.detail,
        "1 anime with recommendations; each asked again after a week"
    );
    assert_eq!(
        named(&after, "TMDB's recommendations").detail,
        "0 titles with recommendations; each asked again after a week",
        "a key is set, so it says what it found rather than asking for one"
    );
    assert_eq!(
        named(&after, "Rising now").detail,
        "Starts at the next catalogue update"
    );
    // A refresh that moved nothing has still updated it.
    media.keep_rising(2, 1_000, 200).await.unwrap();
    let kept = current::sources(&db).await.expect("sources");
    let rising = named(&kept, "Rising now");
    assert!(rising.updated_at.is_some());
    assert_eq!(rising.detail, "No counts moved at the last update");
}
