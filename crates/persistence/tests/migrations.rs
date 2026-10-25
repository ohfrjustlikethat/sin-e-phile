//! Migration integration tests — `SPEC.md` Phase 3, exit criterion E1:
//! "All migrations run forward and backward cleanly against a populated database."
//!
//! The words that matter are **populated** and **backward**. Rolling back an empty
//! database proves almost nothing: the failures worth catching are a `DROP TABLE`
//! blocked by a foreign key that still has rows pointing at it, and a `down` script
//! that forgets an index and so cannot be re-applied. Both need data present.
//!
//! These tests can exist at all only because this crate does not link Tauri
//! (ADR-0022) — a test binary that does cannot launch on Windows.

use sinephile_persistence::model::EpisodeNumbering;
use sinephile_persistence::repositories::{EpisodeRepository, MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, IdSource, MediaKind, NewMediaItem, TitleVariant};

/// Fill every table a migration will have to drop.
///
/// Deliberately touches all four migrations' tables, including the join tables,
/// because those hold the foreign keys that make a careless rollback fail.
async fn populate(db: &Db) -> i64 {
    let media = MediaRepository::new(db);
    let episodes = EpisodeRepository::new(db);
    let profiles = ProfileRepository::new(db);

    // A film with external ids and several title variants.
    let film = media
        .insert(&NewMediaItem::film("Seven Samurai", 1954))
        .await
        .expect("insert film");
    media
        .add_external_id(film, IdSource::Imdb, "tt0047478", 1.0)
        .await
        .expect("imdb id");
    media
        .add_title(film, "七人の侍", TitleVariant::Native, Some("ja"))
        .await
        .expect("native title");
    media
        .add_title(film, "Shichinin no samurai", TitleVariant::Romaji, None)
        .await
        .expect("romaji title");

    // A series with a season and an episode carrying two numbering schemes.
    let series = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::AnimeSeries),
            primary_title: "A Long Running Series".into(),
            ..Default::default()
        })
        .await
        .expect("insert series");
    episodes
        .create_series(series, true)
        .await
        .expect("series row");
    let season = episodes
        .create_season(series, 3, Some("Third Cour"))
        .await
        .expect("season");

    let episode = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Episode),
            primary_title: "An Episode".into(),
            ..Default::default()
        })
        .await
        .expect("insert episode");
    episodes
        .create_episode(episode, series, Some(season), Some(3), Some(7), Some(59))
        .await
        .expect("episode row");
    episodes
        .set_numbering(&EpisodeNumbering {
            episode_id: episode,
            source: IdSource::Tvdb,
            season_number: Some(3),
            episode_number: Some(7),
            absolute_number: None,
        })
        .await
        .expect("tvdb numbering");
    episodes
        .set_numbering(&EpisodeNumbering {
            episode_id: episode,
            source: IdSource::Anilist,
            season_number: None,
            episode_number: None,
            absolute_number: Some(59),
        })
        .await
        .expect("anilist numbering");

    // User data, which is what the foreign keys out of profiles constrain.
    let profile = profiles.create("Default", true).await.expect("profile");
    profiles
        .record_watch(profile, film, 3_000, true)
        .await
        .expect("watch event");
    profiles
        .add_to_watchlist(profile, series, Some("recommended after Seven Samurai"))
        .await
        .expect("watchlist");
    profiles
        .set_setting("theme", "dark")
        .await
        .expect("setting");

    profile
}

#[tokio::test]
async fn migrations_run_forward_on_a_fresh_database() {
    let db = Db::in_memory().await.expect("open");
    let version = db.schema_version().await.expect("version");
    assert_eq!(
        version,
        Some(Db::latest_schema_version()),
        "every migration this binary carries was applied"
    );
}

#[tokio::test]
async fn the_embedded_migrations_match_the_files_on_disk() {
    // `sqlx::migrate!` embeds the directory at compile time, and adding a file does
    // not reliably bust cargo's cache. A stale embed means the binary quietly
    // carries an older schema than the source says it does — which cost an hour
    // when migration 0006 was added, failing as "no column named in_core" against a
    // tree where the column was plainly there. build.rs now forces the rebuild;
    // this makes the failure loud if that ever stops working.
    let on_disk = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
        .expect("migrations directory")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".up.sql"))
        .count() as i64;

    assert_eq!(
        Db::latest_schema_version(),
        on_disk,
        "the binary carries {} migrations but {on_disk} are on disk — the embedded          set is stale. Touch a file in crates/persistence/src/ and rebuild.",
        Db::latest_schema_version()
    );
}

#[tokio::test]
async fn every_table_the_spec_names_exists() {
    let db = Db::in_memory().await.expect("open");
    // SPEC.md Phase 3 lists these by name. If one is missing the schema is not
    // what was specified, whatever else works.
    let expected = [
        "media_items",
        "external_ids",
        "titles",
        "people",
        "credits",
        "genres",
        "keywords",
        "series",
        "seasons",
        "episodes",
        "collections",
        "profiles",
        "watch_events",
        "playback_positions",
        "watchlist_items",
        "local_files",
        "local_file_matches",
        "sources_config",
        "settings",
    ];
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
            .fetch_all(db.pool())
            .await
            .expect("list tables");

    for table in expected {
        assert!(names.iter().any(|n| n == table), "missing table: {table}");
    }
    // Plus the reconciliation table §6.2 requires.
    assert!(names.iter().any(|n| n == "episode_numbering"));
}

#[tokio::test]
async fn all_eight_media_kinds_are_accepted() {
    // The generic-over-media-kind promise (ADR-0025): Phases 24-25 must need no
    // migration, which is only true if the CHECK constraint already allows them.
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);

    for kind in MediaKind::ALL {
        let id = media
            .insert(&NewMediaItem {
                kind: Some(kind),
                primary_title: format!("Item of kind {}", kind.as_str()),
                ..Default::default()
            })
            .await
            .unwrap_or_else(|e| panic!("schema rejected kind {}: {e}", kind.as_str()));

        let back = media.by_id(id).await.expect("read back").expect("exists");
        assert_eq!(back.kind, kind);
    }
}

#[tokio::test]
async fn migrations_roll_back_against_a_populated_database() {
    // The E1 test. A file database, not in-memory: rollback has to survive the
    // real journal, and `undo` on a single-connection in-memory pool would not
    // exercise the same locking.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open(&dir.path().join("rollback.db"))
        .await
        .expect("open");

    populate(&db).await;
    assert_eq!(
        db.schema_version().await.expect("version"),
        Some(Db::latest_schema_version())
    );

    // All the way down, one migration at a time, with data present at every step.
    for target in (0..Db::latest_schema_version()).rev() {
        db.migrate_down_to(target)
            .await
            .unwrap_or_else(|e| panic!("rollback to {target} failed: {e}"));
    }

    let remaining: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations'",
    )
    .fetch_all(db.pool())
    .await
    .expect("list tables");

    assert!(
        remaining.is_empty(),
        "rollback left tables behind: {remaining:?}"
    );
}

#[tokio::test]
async fn the_ladder_can_be_climbed_twice() {
    // Down then up again, which is what catches a `down` script that drops a table
    // but forgets its indexes: the re-apply then fails with "index already exists".
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("round-trip.db");

    let db = Db::open(&path).await.expect("open");
    populate(&db).await;
    db.migrate_down_to(0).await.expect("down");
    drop(db);

    // Re-opening runs the migrations forward again from nothing.
    let db = Db::open(&path).await.expect("reopen");
    assert_eq!(
        db.schema_version().await.expect("version"),
        Some(Db::latest_schema_version())
    );

    // And the schema is usable, not merely present.
    let media = MediaRepository::new(&db);
    let id = media
        .insert(&NewMediaItem::film("Rashomon", 1950))
        .await
        .expect("insert after round trip");
    assert!(media.by_id(id).await.expect("read").is_some());
}

#[tokio::test]
async fn foreign_keys_are_actually_enforced() {
    // SQLite ignores foreign keys unless the connection asks for them, so every
    // REFERENCES clause in the schema is decorative until this passes.
    let db = Db::in_memory().await.expect("open");
    let result = sqlx::query(
        "INSERT INTO external_ids (media_item_id, source, external_id)
         VALUES (999999, 'imdb', 'tt0000000')",
    )
    .execute(db.pool())
    .await;

    assert!(
        result.is_err(),
        "a reference to a non-existent media item was accepted"
    );
}

#[tokio::test]
async fn wal_mode_is_on_for_file_databases() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open(&dir.path().join("wal.db")).await.expect("open");
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(db.pool())
        .await
        .expect("pragma");
    assert_eq!(mode.to_lowercase(), "wal", "SPEC.md §2 requires WAL");
}

#[tokio::test]
async fn backup_on_migrate_writes_a_file_only_when_work_is_pending() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data_dir = dir.path();
    let db = Db::open(&data_dir.join("sinephile.db"))
        .await
        .expect("open");
    populate(&db).await;

    // Already at the latest version, so there is nothing to back up. Ordinary
    // startup must not duplicate the database on disk every launch.
    let backup = db
        .migrate_with_backup(data_dir)
        .await
        .expect("no-op migrate");
    assert!(backup.is_none(), "backed up with no migration pending");
}

#[tokio::test]
async fn a_backup_is_a_real_openable_database() {
    // The point of a backup is that it works when the app does not, so it must be
    // openable by plain SQLite rather than only by this code path.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open(&dir.path().join("live.db")).await.expect("open");
    let profile = populate(&db).await;

    let backup_path = dir.path().join("backups").join("copy.db");
    db.backup_to(&backup_path).await.expect("backup");
    assert!(backup_path.is_file(), "backup file was not written");

    let restored = Db::open(&backup_path).await.expect("open the backup");
    let profiles = ProfileRepository::new(&restored);
    let all = profiles.all().await.expect("read profiles from the backup");
    assert!(
        all.iter().any(|p| p.id == profile),
        "the backup does not contain the data that was live when it was taken"
    );
}
