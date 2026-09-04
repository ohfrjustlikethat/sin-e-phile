//! Every repository method, against a freshly migrated database.
//!
//! **This test is the compensating control for not using sqlx's compile-time
//! `query!` macros** (ADR-0026). It is not a nice-to-have and it is not a summary of
//! the methods that happened to be convenient to call.
//!
//! What it replaces: a typo'd column name, a renamed table, or a query the schema no
//! longer supports would be a compile error with the macros. Here it is a test
//! failure — slightly later, but automatic, and with no `cargo sqlx prepare` ritual
//! after every schema change.
//!
//! It is strictly better than the macros in one respect: every query runs against the
//! **actual migrated schema**, so a migration that forgot a column the code expects
//! fails here. `query!` checks against whatever database `prepare` was last pointed
//! at, which may be neither the schema in the migrations nor the one in production.
//!
//! **STANDING REQUIREMENT (ADR-0026): a new repository method without a line in this
//! file does not pass review.** A method missing from here has no protection at all,
//! and its absence is invisible.

use sinephile_persistence::archive::Archiver;
use sinephile_persistence::model::EpisodeNumbering;
use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{
    CredentialRepository, EpisodeRepository, MediaRepository, ProfileRepository, TmdbAccess,
};
use sinephile_persistence::{Db, IdSource, MediaKind, NewMediaItem, TitleVariant};

/// Exercise every public method on `MediaRepository`.
#[tokio::test]
async fn media_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let repo = MediaRepository::new(&db);

    // insert
    let film = repo
        .insert(&NewMediaItem::film("Seven Samurai", 1954))
        .await
        .expect("insert");

    // insert_many
    let many = repo
        .insert_many(&[
            NewMediaItem::film("Rashomon", 1950),
            NewMediaItem::film("Ikiru", 1952),
        ])
        .await
        .expect("insert_many");
    assert_eq!(many.len(), 2);

    // add_external_id — both the insert and the ON CONFLICT update path
    repo.add_external_id(film, IdSource::Imdb, "tt0047478", 1.0)
        .await
        .expect("add_external_id");
    repo.add_external_id(film, IdSource::Imdb, "tt0047478", 0.9)
        .await
        .expect("add_external_id (conflict path)");

    // add_title — insert and ON CONFLICT DO NOTHING
    repo.add_title(film, "七人の侍", TitleVariant::Native, Some("ja"))
        .await
        .expect("add_title");
    repo.add_title(film, "七人の侍", TitleVariant::Native, Some("ja"))
        .await
        .expect("add_title (conflict path)");

    // by_id
    let found = repo.by_id(film).await.expect("by_id").expect("exists");
    assert_eq!(found.primary_title, "Seven Samurai");
    assert!(repo.by_id(999_999).await.expect("by_id miss").is_none());

    // by_external_id
    assert!(repo
        .by_external_id(IdSource::Imdb, "tt0047478")
        .await
        .expect("by_external_id")
        .is_some());
    assert!(repo
        .by_external_id(IdSource::Tmdb, "nope")
        .await
        .expect("by_external_id miss")
        .is_none());

    // by_exact_title
    assert_eq!(
        repo.by_exact_title("seven samurai")
            .await
            .expect("by_exact_title")
            .len(),
        1,
        "the lookup is case-insensitive"
    );

    // titles_for
    let titles = repo.titles_for(film).await.expect("titles_for");
    assert!(titles.len() >= 2, "primary plus the native variant");

    // count
    assert_eq!(repo.count().await.expect("count"), 3);
}

/// Exercise every public method on `EpisodeRepository`.
#[tokio::test]
async fn episode_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    let repo = EpisodeRepository::new(&db);

    let series = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::AnimeSeries),
            primary_title: "A Series".into(),
            ..Default::default()
        })
        .await
        .expect("series item");

    // create_series — insert and ON CONFLICT update
    repo.create_series(series, true)
        .await
        .expect("create_series");
    repo.create_series(series, false)
        .await
        .expect("create_series (conflict path)");

    // create_season — insert and ON CONFLICT update
    let season = repo
        .create_season(series, 1, Some("First"))
        .await
        .expect("create_season");
    let same = repo
        .create_season(series, 1, Some("Renamed"))
        .await
        .expect("create_season (conflict path)");
    assert_eq!(season, same, "the same season, not a second one");

    let episode = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Episode),
            primary_title: "An Episode".into(),
            ..Default::default()
        })
        .await
        .expect("episode item");

    // create_episode
    repo.create_episode(episode, series, Some(season), Some(1), Some(3), Some(3))
        .await
        .expect("create_episode");

    // set_numbering — insert and ON CONFLICT update
    let numbering = EpisodeNumbering {
        episode_id: episode,
        source: IdSource::Tvdb,
        season_number: Some(1),
        episode_number: Some(3),
        absolute_number: None,
    };
    repo.set_numbering(&numbering).await.expect("set_numbering");
    repo.set_numbering(&numbering)
        .await
        .expect("set_numbering (conflict path)");

    // resolve_seasonal — source-exact path, own-numbering fallback, and a miss
    assert!(repo
        .resolve_seasonal(series, IdSource::Tvdb, 1, 3)
        .await
        .expect("resolve_seasonal")
        .is_some());
    assert!(repo
        .resolve_seasonal(series, IdSource::Tmdb, 1, 3)
        .await
        .expect("resolve_seasonal fallback")
        .is_some());
    assert!(repo
        .resolve_seasonal(series, IdSource::Tvdb, 9, 9)
        .await
        .expect("resolve_seasonal miss")
        .is_none());

    // resolve_absolute — with a source, without one, and a miss
    assert!(repo
        .resolve_absolute(series, Some(IdSource::Tvdb), 3)
        .await
        .expect("resolve_absolute with source")
        .is_some());
    assert!(repo
        .resolve_absolute(series, None, 3)
        .await
        .expect("resolve_absolute without source")
        .is_some());
    assert!(repo
        .resolve_absolute(series, None, 999)
        .await
        .expect("resolve_absolute miss")
        .is_none());

    // numberings_for
    assert_eq!(
        repo.numberings_for(episode)
            .await
            .expect("numberings_for")
            .len(),
        1
    );
}

/// Exercise every public method on `ProfileRepository`.
#[tokio::test]
async fn profile_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    let repo = ProfileRepository::new(&db);

    let film = media
        .insert(&NewMediaItem::film("Tokyo Story", 1953))
        .await
        .expect("film");

    // create — including the demote-the-incumbent path
    let first = repo.create("First", true).await.expect("create");
    let second = repo
        .create("Second", true)
        .await
        .expect("create (demotes first)");

    // default_profile
    let default = repo
        .default_profile()
        .await
        .expect("default_profile")
        .expect("there is one");
    assert_eq!(default.id, second, "the newer default won");
    assert_ne!(default.id, first);

    // all
    assert_eq!(repo.all().await.expect("all").len(), 2);

    // record_watch
    repo.record_watch(second, film, 3_000, true)
        .await
        .expect("record_watch");

    // save_position — insert and ON CONFLICT update
    let position = PlaybackPosition {
        media_item_id: film,
        position_seconds: 100,
        duration_seconds: Some(8_136),
        audio_language: Some("ja".into()),
        subtitle_language: Some("en".into()),
    };
    repo.save_position(second, &position)
        .await
        .expect("save_position");
    repo.save_position(second, &position)
        .await
        .expect("save_position (conflict path)");

    // position — hit and miss
    assert!(repo
        .position(second, film)
        .await
        .expect("position")
        .is_some());
    assert!(repo
        .position(second, 999_999)
        .await
        .expect("position miss")
        .is_none());

    // continue_watching — exercises the completion-threshold arithmetic
    assert_eq!(
        repo.continue_watching(second, 10)
            .await
            .expect("continue_watching")
            .len(),
        1
    );

    // add_to_watchlist — insert and ON CONFLICT DO NOTHING
    repo.add_to_watchlist(second, film, Some("because"))
        .await
        .expect("add_to_watchlist");
    repo.add_to_watchlist(second, film, None)
        .await
        .expect("add_to_watchlist (conflict path)");

    // watchlist
    assert_eq!(repo.watchlist(second).await.expect("watchlist"), vec![film]);

    // set_setting — insert and ON CONFLICT update
    repo.set_setting("theme", "dark")
        .await
        .expect("set_setting");
    repo.set_setting("theme", "light")
        .await
        .expect("set_setting (conflict path)");

    // setting — hit and miss
    assert_eq!(
        repo.setting("theme").await.expect("setting").as_deref(),
        Some("light")
    );
    assert!(repo
        .setting("absent")
        .await
        .expect("setting miss")
        .is_none());
}

/// Exercise every public method on `Archiver`.
#[tokio::test]
async fn archiver_surface() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    let profiles = ProfileRepository::new(&db);

    let film = media
        .insert(&NewMediaItem::film("Late Spring", 1949))
        .await
        .expect("film");
    media
        .add_external_id(film, IdSource::Imdb, "tt0041154", 1.0)
        .await
        .expect("external id");
    let profile = profiles.create("Author", true).await.expect("profile");
    profiles
        .record_watch(profile, film, 500, true)
        .await
        .expect("watch");

    let archiver = Archiver::new(&db);

    // export, and the no-such-profile error path
    let archive = archiver.export("Author").await.expect("export");
    assert!(archiver.export("Nobody").await.is_err());

    // export_to_file
    let path = dir.path().join("p.json");
    archiver
        .export_to_file("Author", &path)
        .await
        .expect("export_to_file");

    // import
    let target = Db::in_memory().await.expect("target");
    let t_media = MediaRepository::new(&target);
    let t_film = t_media
        .insert(&NewMediaItem::film("Late Spring", 1949))
        .await
        .expect("film");
    t_media
        .add_external_id(t_film, IdSource::Imdb, "tt0041154", 1.0)
        .await
        .expect("external id");
    let t_profile = ProfileRepository::new(&target)
        .create("Author", true)
        .await
        .expect("profile");

    let t_archiver = Archiver::new(&target);
    t_archiver
        .import(t_profile, &archive)
        .await
        .expect("import");

    // import_from_file
    t_archiver
        .import_from_file(t_profile, &path)
        .await
        .expect("import_from_file");
}

/// Exercise every public method on `Db` itself.
#[tokio::test]
async fn db_surface() {
    let dir = tempfile::tempdir().expect("tempdir");

    // open, open_in, in_memory
    let db = Db::open(&dir.path().join("a.db")).await.expect("open");
    let _in_dir = Db::open_in(&dir.path().join("data"))
        .await
        .expect("open_in");
    let _mem = Db::in_memory().await.expect("in_memory");

    // pool, path
    assert!(db.path().ends_with("a.db"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(db.pool())
            .await
            .expect("pool works"),
        1
    );

    // ── CredentialRepository ──────────────────────────────────────────────────
    // ADR-0027: no key ever ships, so every one of these starts from Absent.
    let profile = ProfileRepository::new(&db)
        .create("Author", true)
        .await
        .expect("profile");
    let creds = CredentialRepository::new(&db);

    // tmdb_access — the default state of the application
    assert_eq!(
        creds.tmdb_access(profile).await.expect("tmdb_access"),
        TmdbAccess::Absent
    );

    // profiles_with_a_key
    assert_eq!(creds.profiles_with_a_key().await.expect("count"), 0);

    // set_tmdb_key
    creds
        .set_tmdb_key(profile, "abcdef0123456789abcdef0123456789")
        .await
        .expect("set_tmdb_key");
    assert!(creds
        .tmdb_access(profile)
        .await
        .expect("tmdb_access")
        .is_configured());

    // set_tmdb_key — refuses what is not a key
    assert!(creds.set_tmdb_key(profile, "nope").await.is_err());

    // set / get, the non-secret path
    creds.set(profile, "ui.theme", "dark").await.expect("set");
    assert_eq!(
        creds.get(profile, "ui.theme").await.expect("get"),
        Some("dark".to_string())
    );
    // get — refuses to hand back a secret
    assert!(creds.get(profile, "tmdb.api_key").await.is_err());

    // clear_tmdb_key
    let (removed, _discarded) = creds.clear_tmdb_key(profile).await.expect("clear_tmdb_key");
    assert!(removed);
    assert_eq!(
        creds.tmdb_access(profile).await.expect("tmdb_access"),
        TmdbAccess::Absent
    );

    // schema_version
    assert_eq!(
        db.schema_version().await.expect("schema_version"),
        Some(Db::latest_schema_version())
    );

    // backup_to
    let backup = dir.path().join("backups/b.db");
    db.backup_to(&backup).await.expect("backup_to");
    assert!(backup.is_file());

    // migrate_with_backup — the no-op path, already at the latest version
    assert!(db
        .migrate_with_backup(dir.path())
        .await
        .expect("migrate_with_backup")
        .is_none());

    // migrate_down_to
    let one_back = Db::latest_schema_version() - 1;
    db.migrate_down_to(one_back).await.expect("migrate_down_to");
    assert_eq!(db.schema_version().await.expect("version"), Some(one_back));
}
