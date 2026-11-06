//! Identification against a small catalogue built here: each test is one of the reasons a
//! single signal is not enough. The full measurement is `eval identify` (Phase 12 E1, E2).

use sinephile_library::{identify, Evidence, ACCEPT_AT};
use sinephile_persistence::repositories::{
    EpisodeRepository, LibraryRepository, MediaRepository, SearchRepository,
};
use sinephile_persistence::{Db, MediaKind, NewMediaItem};

/// The catalogue's normalised form, as the ingest writes it.
fn normalised(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn item(db: &Db, kind: MediaKind, title: &str, year: i64, runtime: i64, votes: i64) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem {
            kind: Some(kind),
            primary_title: title.into(),
            release_year: Some(year),
            runtime_minutes: Some(runtime),
            rating_votes: Some(votes),
            ..Default::default()
        })
        .await
        .expect("insert");
    sqlx::query("UPDATE titles SET normalised = ? WHERE media_item_id = ?")
        .bind(normalised(title))
        .bind(id)
        .execute(db.pool())
        .await
        .expect("normalise");
    SearchRepository::new(db)
        .index_item(id, title, "", "", "")
        .await
        .expect("index");
    id
}

fn evidence(name: &str) -> Evidence {
    Evidence {
        name: name.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn the_year_tells_a_remake_from_the_original() {
    let db = Db::in_memory().await.unwrap();
    let old = item(&db, MediaKind::Film, "Halloween", 1978, 91, 300_000).await;
    let new = item(&db, MediaKind::Film, "Halloween", 2018, 106, 200_000).await;

    let r = identify(&db, &evidence("Halloween.2018.1080p.BluRay.x264-GROUP.mkv"))
        .await
        .unwrap();
    assert_eq!(r.best().unwrap().media_item_id, new);
    assert!(
        r.confidence >= ACCEPT_AT,
        "a year settles it: {}",
        r.confidence
    );

    let r = identify(&db, &evidence("Halloween.1978.mkv"))
        .await
        .unwrap();
    assert_eq!(r.best().unwrap().media_item_id, old);
}

#[tokio::test]
async fn with_no_year_and_no_duration_it_asks() {
    let db = Db::in_memory().await.unwrap();
    item(&db, MediaKind::Film, "Halloween", 1978, 91, 300_000).await;
    item(&db, MediaKind::Film, "Halloween", 2018, 106, 200_000).await;
    let r = identify(&db, &evidence("Halloween.mkv")).await.unwrap();
    assert!(
        r.confidence < ACCEPT_AT,
        "two good readings: ask ({})",
        r.confidence
    );
    assert_eq!(r.candidates.len(), 2, "both are offered");
}

#[tokio::test]
async fn the_duration_tells_them_apart_when_the_name_cannot() {
    let db = Db::in_memory().await.unwrap();
    let old = item(&db, MediaKind::Film, "Halloween", 1978, 91, 200_000).await;
    item(&db, MediaKind::Film, "Halloween", 2018, 106, 200_000).await;
    let r = identify(
        &db,
        &Evidence {
            name: "Halloween.mkv".into(),
            duration_minutes: Some(91.4),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(r.best().unwrap().media_item_id, old);
}

#[tokio::test]
async fn an_episode_name_means_the_series_and_finds_the_episode() {
    let db = Db::in_memory().await.unwrap();
    item(&db, MediaKind::Film, "Westworld", 1973, 88, 70_000).await;
    let series = item(&db, MediaKind::Series, "Westworld", 2016, 62, 600_000).await;
    let episodes = EpisodeRepository::new(&db);
    episodes.create_series(series, false).await.unwrap();
    let season = episodes.create_season(series, 1, None).await.unwrap();
    let finale = MediaRepository::new(&db)
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Episode),
            primary_title: "The Bicameral Mind".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    episodes
        .create_episode(finale, series, Some(season), Some(1), Some(10), None)
        .await
        .unwrap();

    // A 90-minute finale fits the film's runtime better; the name and the episode win.
    let r = identify(
        &db,
        &Evidence {
            name: "Westworld.S01E10.The.Bicameral.Mind.1080p.mkv".into(),
            duration_minutes: Some(90.0),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(r.best().unwrap().media_item_id, series);
    assert_eq!(r.episode.map(|e| e.episode_id), Some(finale));
    assert_eq!(r.best().unwrap().signals.episode_found, Some(true));
}

#[tokio::test]
async fn the_folders_name_what_the_file_does_not() {
    let db = Db::in_memory().await.unwrap();
    let series = item(&db, MediaKind::Series, "Breaking Bad", 2008, 49, 2_000_000).await;
    let r = identify(
        &db,
        &Evidence {
            name: "S01E02.mkv".into(),
            folders: vec!["Breaking Bad (2008)".into(), "Season 1".into()],
            duration_minutes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(r.title, "Breaking Bad");
    assert_eq!((r.season, r.episodes.clone()), (Some(1), vec![2]));
    assert_eq!(r.best().unwrap().media_item_id, series);
}

#[tokio::test]
async fn the_users_decision_outranks_every_signal() {
    let db = Db::in_memory().await.unwrap();
    item(&db, MediaKind::Film, "Halloween", 1978, 91, 300_000).await;
    let new = item(&db, MediaKind::Film, "Halloween", 2018, 106, 200_000).await;
    let first = identify(&db, &evidence("Halloween.mkv")).await.unwrap();
    LibraryRepository::new(&db)
        .remember(&first.pattern, new)
        .await
        .unwrap();

    let r = identify(&db, &evidence("halloween.mkv")).await.unwrap();
    assert!(r.remembered);
    assert_eq!((r.best().unwrap().media_item_id, r.confidence), (new, 1.0));
}

#[tokio::test]
async fn a_name_the_catalogue_does_not_know_is_not_guessed() {
    let db = Db::in_memory().await.unwrap();
    item(&db, MediaKind::Film, "Halloween", 1978, 91, 300_000).await;
    let r = identify(&db, &evidence("Zxqv.Unknown.Thing.2011.mkv"))
        .await
        .unwrap();
    assert!(r.candidates.is_empty());
    assert_eq!(r.confidence, 0.0);
}
