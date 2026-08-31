//! Portable profile export/import (`SPEC.md` Phase 3).
//!
//! The test that matters is `history_survives_a_different_installation`. Internal
//! row ids are assigned in ingestion order, so the same film is a different number
//! on two machines. An archive keyed on them would restore a viewing history
//! pointing at the wrong films — and it would look like it worked.

use sinephile_persistence::archive::Archiver;
use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, IdSource, NewMediaItem};

/// Insert films in the given order, giving each a stable IMDb id.
///
/// The order is the point: it decides the internal ids.
async fn catalogue(db: &Db, films: &[(&str, i64, &str)]) -> Vec<i64> {
    let media = MediaRepository::new(db);
    let mut ids = Vec::new();
    for (title, year, imdb) in films {
        let id = media
            .insert(&NewMediaItem::film(*title, *year))
            .await
            .expect("insert");
        media
            .add_external_id(id, IdSource::Imdb, imdb, 1.0)
            .await
            .expect("imdb id");
        ids.push(id);
    }
    ids
}

const SEVEN_SAMURAI: (&str, i64, &str) = ("Seven Samurai", 1954, "tt0047478");
const RASHOMON: (&str, i64, &str) = ("Rashomon", 1950, "tt0042876");
const IKIRU: (&str, i64, &str) = ("Ikiru", 1952, "tt0044741");

#[tokio::test]
async fn export_captures_history_watchlist_and_settings() {
    let db = Db::in_memory().await.expect("open");
    let ids = catalogue(&db, &[SEVEN_SAMURAI, RASHOMON, IKIRU]).await;
    let profiles = ProfileRepository::new(&db);
    let profile = profiles.create("Author", true).await.expect("profile");

    profiles
        .record_watch(profile, ids[0], 3_000, true)
        .await
        .expect("watch");
    profiles
        .add_to_watchlist(profile, ids[1], Some("after Seven Samurai"))
        .await
        .expect("watchlist");
    profiles
        .save_position(
            profile,
            &PlaybackPosition {
                media_item_id: ids[2],
                position_seconds: 1_234,
                duration_seconds: Some(8_460),
                audio_language: Some("ja".into()),
                subtitle_language: Some("en".into()),
            },
        )
        .await
        .expect("position");
    profiles
        .set_setting("theme", "dark")
        .await
        .expect("setting");

    let archive = Archiver::new(&db).export("Author").await.expect("export");

    assert_eq!(archive.watch_events.len(), 1);
    assert_eq!(archive.watch_events[0].item.external_id, SEVEN_SAMURAI.2);
    assert_eq!(archive.watchlist.len(), 1);
    assert_eq!(
        archive.watchlist[0].reason.as_deref(),
        Some("after Seven Samurai")
    );
    assert_eq!(archive.positions.len(), 1);
    assert_eq!(
        archive.positions[0].subtitle_language.as_deref(),
        Some("en")
    );
    assert!(archive
        .settings
        .iter()
        .any(|(k, v)| k == "theme" && v == "dark"));
}

#[tokio::test]
async fn history_survives_a_different_installation() {
    // THE test. Two installations, the same films, ingested in a different order,
    // so every internal id differs.
    let source = Db::in_memory().await.expect("source db");
    let source_ids = catalogue(&source, &[SEVEN_SAMURAI, RASHOMON, IKIRU]).await;

    let profiles = ProfileRepository::new(&source);
    let profile = profiles.create("Author", true).await.expect("profile");
    profiles
        .record_watch(profile, source_ids[2], 5_000, true) // Ikiru, internal id 3
        .await
        .expect("watch");
    profiles
        .add_to_watchlist(profile, source_ids[0], None) // Seven Samurai, id 1
        .await
        .expect("watchlist");

    let archive = Archiver::new(&source)
        .export("Author")
        .await
        .expect("export");

    // Reverse order, plus an extra film, so nothing lines up numerically.
    let target = Db::in_memory().await.expect("target db");
    let target_ids = catalogue(
        &target,
        &[
            ("Stray Dog", 1949, "tt0041699"),
            IKIRU,
            RASHOMON,
            SEVEN_SAMURAI,
        ],
    )
    .await;

    assert_ne!(
        source_ids[2], target_ids[1],
        "test premise: Ikiru must have a different internal id in each installation"
    );

    let target_profiles = ProfileRepository::new(&target);
    let new_profile = target_profiles
        .create("Author", true)
        .await
        .expect("profile");
    let summary = Archiver::new(&target)
        .import(new_profile, &archive)
        .await
        .expect("import");

    assert_eq!(summary.watch_events, 1);
    assert_eq!(summary.watchlist, 1);
    assert!(summary.unmatched.is_empty(), "everything resolved");

    // The history must point at Ikiru, not at whatever holds Ikiru's old id.
    let watched: Vec<i64> =
        sqlx::query_scalar("SELECT media_item_id FROM watch_events WHERE profile_id = ?")
            .bind(new_profile)
            .fetch_all(target.pool())
            .await
            .expect("query");

    assert_eq!(
        watched,
        vec![target_ids[1]],
        "restored history points at Ikiru"
    );

    let watchlisted = target_profiles
        .watchlist(new_profile)
        .await
        .expect("watchlist");
    assert_eq!(
        watchlisted,
        vec![target_ids[3]],
        "watchlist points at Seven Samurai"
    );
}

#[tokio::test]
async fn importing_twice_does_not_double_the_history() {
    // Re-importing after a failed import is a normal thing to do.
    let db = Db::in_memory().await.expect("open");
    let ids = catalogue(&db, &[SEVEN_SAMURAI]).await;
    let profiles = ProfileRepository::new(&db);
    let profile = profiles.create("Author", true).await.expect("profile");
    profiles
        .record_watch(profile, ids[0], 3_000, true)
        .await
        .expect("watch");

    let archive = Archiver::new(&db).export("Author").await.expect("export");

    let target = Db::in_memory().await.expect("target");
    catalogue(&target, &[SEVEN_SAMURAI]).await;
    let target_profiles = ProfileRepository::new(&target);
    let p = target_profiles
        .create("Author", true)
        .await
        .expect("profile");

    let archiver = Archiver::new(&target);
    let first = archiver.import(p, &archive).await.expect("first import");
    let second = archiver.import(p, &archive).await.expect("second import");

    assert_eq!(first.watch_events, 1);
    assert_eq!(second.watch_events, 0, "the second import added nothing");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_events")
        .fetch_one(target.pool())
        .await
        .expect("count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn an_item_the_target_catalogue_lacks_is_reported_not_dropped() {
    let source = Db::in_memory().await.expect("source");
    let ids = catalogue(&source, &[SEVEN_SAMURAI, RASHOMON]).await;
    let profiles = ProfileRepository::new(&source);
    let profile = profiles.create("Author", true).await.expect("profile");
    profiles
        .record_watch(profile, ids[0], 100, true)
        .await
        .expect("w1");
    profiles
        .record_watch(profile, ids[1], 200, true)
        .await
        .expect("w2");

    let archive = Archiver::new(&source)
        .export("Author")
        .await
        .expect("export");

    // Target has only one of the two films.
    let target = Db::in_memory().await.expect("target");
    catalogue(&target, &[RASHOMON]).await;
    let p = ProfileRepository::new(&target)
        .create("Author", true)
        .await
        .expect("profile");

    let summary = Archiver::new(&target)
        .import(p, &archive)
        .await
        .expect("import");

    assert_eq!(summary.watch_events, 1, "the matchable one was imported");
    assert_eq!(summary.unmatched.len(), 1, "the other was reported");
    assert_eq!(summary.unmatched[0].external_id, SEVEN_SAMURAI.2);
    assert_eq!(
        summary.unmatched[0].title, "Seven Samurai",
        "the report names the film, so the user can act on it"
    );
}

#[tokio::test]
async fn an_item_with_no_external_id_is_reported_rather_than_silently_missing() {
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    // A local file identified by hand, with no catalogue entry behind it.
    let orphan = media
        .insert(&NewMediaItem::film("Home Movie 1997", 1997))
        .await
        .expect("insert");

    let profiles = ProfileRepository::new(&db);
    let profile = profiles.create("Author", true).await.expect("profile");
    profiles
        .record_watch(profile, orphan, 600, true)
        .await
        .expect("watch");

    let archive = Archiver::new(&db).export("Author").await.expect("export");

    assert!(
        archive.watch_events.is_empty(),
        "it cannot be referenced portably"
    );
    assert_eq!(
        archive.unreferenceable_items,
        vec!["Home Movie 1997".to_string()],
        "and the archive says so instead of quietly being short"
    );
}

#[tokio::test]
async fn a_newer_archive_is_refused_rather_than_partially_read() {
    let db = Db::in_memory().await.expect("open");
    let profile = ProfileRepository::new(&db)
        .create("Author", true)
        .await
        .expect("profile");

    let mut archive = Archiver::new(&db).export("Author").await.expect("export");
    archive.version = 999;

    let result = Archiver::new(&db).import(profile, &archive).await;
    assert!(
        result.is_err(),
        "an archive from a future build must be refused"
    );
}

#[tokio::test]
async fn the_archive_round_trips_through_a_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profile.json");

    let db = Db::in_memory().await.expect("open");
    let ids = catalogue(&db, &[SEVEN_SAMURAI]).await;
    let profiles = ProfileRepository::new(&db);
    let profile = profiles.create("Author", true).await.expect("profile");
    profiles
        .record_watch(profile, ids[0], 3_000, true)
        .await
        .expect("watch");

    Archiver::new(&db)
        .export_to_file("Author", &path)
        .await
        .expect("export to file");

    // Readable as plain JSON by anything, which is the point of the format choice.
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(
        text.contains("tt0047478"),
        "external ids are visible in the file"
    );

    let target = Db::in_memory().await.expect("target");
    catalogue(&target, &[SEVEN_SAMURAI]).await;
    let p = ProfileRepository::new(&target)
        .create("Author", true)
        .await
        .expect("profile");

    let summary = Archiver::new(&target)
        .import_from_file(p, &path)
        .await
        .expect("import from file");
    assert_eq!(summary.watch_events, 1);
}
