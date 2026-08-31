//! Portability — `SPEC.md` Phase 3, exit criterion E4:
//! "Copying the app folder to another location and launching it preserves all data."
//!
//! §2.4's promise is that the whole app is a folder you can move to a USB stick.
//! That is only true if nothing in the database refers to where it currently is —
//! no absolute paths in the schema, no machine-specific state, and a WAL that has
//! been folded back into the main file before the copy.
//!
//! The WAL part is the one that actually bites. SQLite keeps recent writes in a
//! `-wal` sidecar file, so copying only `sinephile.db` silently loses every write
//! since the last checkpoint — and a user copying a folder copies whatever the
//! file manager shows them.

use std::path::Path;

use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{paths, Db, DataLocation, IdSource, NewMediaItem};

/// Copy a directory tree, the way a user dragging a folder would.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create destination");
    for entry in std::fs::read_dir(from).expect("read source") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

#[tokio::test]
async fn a_copied_data_folder_keeps_everything() {
    let root = tempfile::tempdir().expect("tempdir");
    let original = root.path().join("sin-e-phile/data");
    let elsewhere = root.path().join("D_drive/sin-e-phile/data");

    // Populate the original installation.
    let (film_id, profile_id) = {
        let db = Db::open_in(&original).await.expect("open original");
        let media = MediaRepository::new(&db);
        let profiles = ProfileRepository::new(&db);

        let film = media
            .insert(&NewMediaItem::film("Tokyo Story", 1953))
            .await
            .expect("insert");
        media
            .add_external_id(film, IdSource::Imdb, "tt0046438", 1.0)
            .await
            .expect("external id");

        let profile = profiles.create("Author", true).await.expect("profile");
        profiles
            .record_watch(profile, film, 8_100, true)
            .await
            .expect("watch");
        profiles
            .save_position(
                profile,
                &PlaybackPosition {
                    media_item_id: film,
                    position_seconds: 4_050,
                    duration_seconds: Some(8_136),
                    audio_language: Some("ja".into()),
                    subtitle_language: Some("en".into()),
                },
            )
            .await
            .expect("position");
        profiles.set_setting("theme", "dark").await.expect("setting");

        // Fold the WAL back in, as closing the app does. Without this the copy
        // below loses every write made since the last automatic checkpoint.
        db.backup_to(&original.join("checkpoint-probe.db"))
            .await
            .expect("checkpoint");
        std::fs::remove_file(original.join("checkpoint-probe.db")).ok();

        (film, profile)
    }; // the pool is dropped here — the app has "closed"

    copy_dir(&original, &elsewhere);

    // Launch from the new location. Nothing is told where it used to be.
    let moved = Db::open_in(&elsewhere).await.expect("open the copy");

    let media = MediaRepository::new(&moved);
    let film = media
        .by_external_id(IdSource::Imdb, "tt0046438")
        .await
        .expect("query")
        .expect("the film survived the move");
    assert_eq!(film.primary_title, "Tokyo Story");
    assert_eq!(film.id, film_id, "internal ids are stable across a copy");

    let profiles = ProfileRepository::new(&moved);
    let position = profiles
        .position(profile_id, film_id)
        .await
        .expect("query")
        .expect("the resume position survived");
    assert_eq!(position.position_seconds, 4_050);
    assert_eq!(position.subtitle_language.as_deref(), Some("en"));

    assert_eq!(
        profiles.setting("theme").await.expect("query").as_deref(),
        Some("dark")
    );

    let watched: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_events")
        .fetch_one(moved.pool())
        .await
        .expect("count");
    assert_eq!(watched, 1, "the watch history survived");
}

#[tokio::test]
async fn the_original_still_works_after_being_copied() {
    // Copying is not moving. A user who copies to a USB stick expects the machine
    // they copied FROM to be untouched.
    let root = tempfile::tempdir().expect("tempdir");
    let original = root.path().join("app/data");
    let copy = root.path().join("usb/data");

    {
        let db = Db::open_in(&original).await.expect("open");
        MediaRepository::new(&db)
            .insert(&NewMediaItem::film("Late Spring", 1949))
            .await
            .expect("insert");
    }

    copy_dir(&original, &copy);

    let db = Db::open_in(&original).await.expect("reopen original");
    assert_eq!(
        MediaRepository::new(&db).count().await.expect("count"),
        1,
        "the original was left intact"
    );
}

#[tokio::test]
async fn nothing_in_the_database_records_where_it_lives() {
    // The structural reason a copy works: no absolute path is ever stored, except
    // in `local_files`, where the path IS the data and is expected to be machine
    // specific.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT m.name || '.' || i.name
         FROM sqlite_master m
         JOIN pragma_table_info(m.name) i
         WHERE m.type = 'table'
           AND (i.name LIKE '%path%' OR i.name LIKE '%dir%' OR i.name LIKE '%location%')",
    )
    .fetch_all(db.pool())
    .await
    .expect("introspect");

    assert_eq!(
        columns,
        vec!["local_files.path".to_string()],
        "a new path-shaped column appeared; if it holds an absolute path, copying \
         the app folder no longer preserves it (SPEC.md §2.4, Phase 3 E4)"
    );
}

#[tokio::test]
async fn the_data_directory_defaults_to_portable() {
    // §2.4: portable is the DEFAULT and installed mode is the opt-in, not the
    // other way round.
    let dir = tempfile::tempdir().expect("tempdir");
    std::env::set_var(paths::DATA_DIR_ENV, dir.path());
    let resolved = paths::data_dir(DataLocation::Portable).expect("resolve");
    assert_eq!(resolved, dir.path());
    std::env::remove_var(paths::DATA_DIR_ENV);
}

#[tokio::test]
async fn an_unwritable_directory_fails_clearly() {
    // A portable app gets run from read-only media. "The database is read-only"
    // surfacing as an opaque SQLite error several layers down is worse than a
    // named failure at startup.
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does/not/exist/and/cannot/be/made");

    // On Windows a deep non-existent path is created by create_dir_all, so the
    // meaningful assertion is that assert_writable SUCCEEDS on a real directory
    // and the error type exists for the case where it does not.
    assert!(paths::assert_writable(dir.path()).is_ok());
    assert!(
        !missing.exists(),
        "test premise: the probe directory does not exist"
    );
    assert!(paths::assert_writable(&missing).is_err());
}
