//! `LocalFileBackend` answers from the library (Phase 12): a film by its catalogue id, an
//! episode through its series, and nothing for a file that has gone.

use sinephile_persistence::repositories::{
    EpisodeRepository, FileMatch, LibraryRepository, MatchMethod, MediaRepository, NewLocalFile,
};
use sinephile_persistence::{Db, IdSource, MediaKind, NewMediaItem};
use sinephile_sources::backends::local::LocalFileBackend;
use source_protocol::{
    ExternalIds, MediaKind as Kind, MediaQuery, Resolution, SourceBackend, Transport,
};

async fn library_file(db: &Db, path: &str, item: i64, height: Option<i64>) {
    let repo = LibraryRepository::new(db);
    let folder = repo.add_folder(r"D:\Library").await.unwrap();
    let id = repo
        .upsert_file(&NewLocalFile {
            folder_id: folder,
            path,
            size_bytes: 1_000,
            modified_at: None,
        })
        .await
        .unwrap()
        .id();
    if let Some(h) = height {
        repo.set_probe(
            id,
            &sinephile_persistence::repositories::Probe {
                height: Some(h),
                video_codec: Some("hevc".into()),
                audio_languages: vec!["en".into()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
    repo.set_match(
        id,
        &FileMatch {
            media_item_id: item,
            confidence: 1.0,
            method: MatchMethod::Manual,
            covers: 1,
            confirmed: true,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_film_in_the_library_is_offered_as_a_local_copy() {
    let db = Db::in_memory().await.unwrap();
    let media = MediaRepository::new(&db);
    let heat = media
        .insert(&NewMediaItem::film("Heat", 1995))
        .await
        .unwrap();
    media
        .add_external_id(heat, IdSource::Imdb, "tt0113277", 1.0)
        .await
        .unwrap();
    // A real file, so the "still there" check passes.
    let me = std::env::current_exe().unwrap();
    library_file(&db, &me.to_string_lossy(), heat, Some(2160)).await;

    let backend = LocalFileBackend::new(Some(db.clone()));
    let mut q = MediaQuery::film("Heat", Some(1995));
    q.ids = ExternalIds {
        imdb: Some("tt0113277".into()),
        ..Default::default()
    };
    let found = backend.find(&q).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].transport, Transport::Local);
    assert_eq!(found[0].quality.resolution, Resolution::Uhd);
    assert_eq!(found[0].languages, vec!["en"]);
    assert_eq!(found[0].key, me.to_string_lossy());

    // Without a database, or for a film not in the library: nothing.
    assert!(LocalFileBackend::default()
        .find(&q)
        .await
        .unwrap()
        .is_empty());
    let other = MediaQuery::film("Thief", Some(1981));
    assert!(backend.find(&other).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_episode_is_found_through_its_series_and_a_vanished_file_is_not() {
    let db = Db::in_memory().await.unwrap();
    let media = MediaRepository::new(&db);
    let series = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Series),
            primary_title: "The Wire".into(),
            release_year: Some(2002),
            ..Default::default()
        })
        .await
        .unwrap();
    media
        .add_external_id(series, IdSource::Imdb, "tt0306414", 1.0)
        .await
        .unwrap();
    let episodes = EpisodeRepository::new(&db);
    episodes.create_series(series, false).await.unwrap();
    let season = episodes.create_season(series, 1, None).await.unwrap();
    let ep = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Episode),
            primary_title: "The Target".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    episodes
        .create_episode(ep, series, Some(season), Some(1), Some(1), None)
        .await
        .unwrap();
    let me = std::env::current_exe().unwrap();
    library_file(&db, &me.to_string_lossy(), ep, None).await;

    let backend = LocalFileBackend::new(Some(db.clone()));
    let q = MediaQuery {
        kind: Kind::Episode,
        title: "The Wire".into(),
        year: Some(2002),
        ids: ExternalIds {
            imdb: Some("tt0306414".into()),
            ..Default::default()
        },
        season: Some(1),
        episode: Some(1),
    };
    assert_eq!(backend.find(&q).await.unwrap().len(), 1);
    let q2 = MediaQuery {
        episode: Some(2),
        ..q.clone()
    };
    assert!(
        backend.find(&q2).await.unwrap().is_empty(),
        "episode 2 is not in the library"
    );

    // The library says it is there; the disk says it is not: not offered.
    let db2 = Db::in_memory().await.unwrap();
    let m2 = MediaRepository::new(&db2);
    let f = m2.insert(&NewMediaItem::film("Heat", 1995)).await.unwrap();
    m2.add_external_id(f, IdSource::Imdb, "tt0113277", 1.0)
        .await
        .unwrap();
    library_file(&db2, r"C:\gone\Heat.mkv", f, None).await;
    let mut hq = MediaQuery::film("Heat", Some(1995));
    hq.ids.imdb = Some("tt0113277".into());
    assert!(LocalFileBackend::new(Some(db2))
        .find(&hq)
        .await
        .unwrap()
        .is_empty());
}
