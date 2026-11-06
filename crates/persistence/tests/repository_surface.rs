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
    BackfillRepository, CatalogueRepository, CredentialRepository, DirectLinkRepository,
    EpisodeRepository, MatchReason, MediaRepository, ProfileRepository, Readiness, Sample,
    SearchRepository, SourceRepository, TmdbAccess, WikipediaRepository,
};
use sinephile_persistence::repositories::{CachedSubtitle, QuotaDay, SubtitleRepository};
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

    // seasons / episodes_of / next_after / previous_before (Phase 11)
    let mk = |title: &'static str| NewMediaItem {
        kind: Some(MediaKind::Episode),
        primary_title: title.into(),
        ..Default::default()
    };
    let s1e4 = media.insert(&mk("S1E4")).await.unwrap();
    let s2e1 = media.insert(&mk("S2E1")).await.unwrap();
    let season2 = repo.create_season(series, 2, None).await.unwrap();
    repo.create_episode(s1e4, series, Some(season), Some(1), Some(4), None)
        .await
        .unwrap();
    repo.create_episode(s2e1, series, Some(season2), Some(2), Some(1), None)
        .await
        .unwrap();
    assert_eq!(
        repo.seasons(series).await.expect("seasons"),
        [(Some(1), 2), (Some(2), 1)]
    );
    let first = repo
        .episodes_of(series, Some(1))
        .await
        .expect("episodes_of");
    assert_eq!(
        first.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(),
        ["An Episode", "S1E4"]
    );
    assert!(repo.episodes_of(series, None).await.unwrap().is_empty());
    // Across the season boundary, both ways; nothing past the ends.
    assert_eq!(
        repo.next_after(s1e4)
            .await
            .expect("next_after")
            .map(|e| e.media_item_id),
        Some(s2e1)
    );
    assert_eq!(repo.next_after(s2e1).await.unwrap(), None);
    assert_eq!(
        repo.previous_before(s2e1)
            .await
            .expect("previous_before")
            .map(|e| e.media_item_id),
        Some(s1e4)
    );
    assert_eq!(repo.previous_before(episode).await.unwrap(), None);
    // placement
    assert_eq!(
        repo.placement(s2e1).await.expect("placement"),
        Some((series, Some(2), Some(1)))
    );
    assert_eq!(
        repo.placement(series).await.unwrap(),
        None,
        "not an episode"
    );

    // set_next_airing / next_airing — no episode row needed; the newest replaces
    assert_eq!(repo.next_airing(series).await.expect("next_airing"), None);
    repo.set_next_airing(series, 7, 1_794_200_000)
        .await
        .expect("set");
    repo.set_next_airing(series, 8, 1_794_800_000)
        .await
        .expect("replace");
    assert_eq!(
        repo.next_airing(series).await.unwrap(),
        Some((8, 1_794_800_000))
    );

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

    // rate / rating (Phase 11) — replace, clear, refuse out of range
    assert_eq!(repo.rating(first, film).await.expect("rating"), None);
    repo.rate(first, film, Some(4)).await.expect("rate");
    repo.rate(first, film, Some(5)).await.expect("rate again");
    assert_eq!(repo.rating(first, film).await.unwrap(), Some(5));
    assert!(repo.rate(first, film, Some(6)).await.is_err());
    repo.rate(first, film, None).await.expect("clear");
    assert_eq!(repo.rating(first, film).await.unwrap(), None);
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

    // opensubtitles_key / set_opensubtitles_key / clear_opensubtitles_key (Phase 10)
    assert_eq!(
        creds.opensubtitles_key(profile).await.unwrap(),
        None,
        "none ships"
    );
    assert!(creds
        .set_opensubtitles_key(profile, "too-short")
        .await
        .is_err());
    creds
        .set_opensubtitles_key(profile, " Ab0123456789cdEF0123456789abcd01 ")
        .await
        .expect("set_opensubtitles_key");
    assert_eq!(
        creds.opensubtitles_key(profile).await.unwrap().as_deref(),
        Some("Ab0123456789cdEF0123456789abcd01")
    );
    assert!(
        creds.get(profile, "opensubtitles.api_key").await.is_err(),
        "a secret"
    );
    assert!(creds.clear_opensubtitles_key(profile).await.unwrap());
    assert!(!creds.clear_opensubtitles_key(profile).await.unwrap());
    assert_eq!(creds.opensubtitles_key(profile).await.unwrap(), None);

    // ── SearchRepository ──────────────────────────────────────────────────────
    let search = SearchRepository::new(&db);

    // indexed_count — empty is zero, not an error
    assert_eq!(search.indexed_count().await.expect("indexed_count"), 0);

    // `search_indexed` has a foreign key onto media_items, so the row has to exist.
    let probe = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Probe Title", 1970))
        .await
        .expect("probe item");

    // index_item, then again, to exercise the contentless delete-and-reinsert
    search
        .index_item(probe, "Probe Title", "Alt Probe", "A Person", "drama")
        .await
        .expect("index_item");
    search
        .index_item(probe, "Probe Title", "Alt Probe", "A Person", "drama")
        .await
        .expect("re-index");
    assert_eq!(search.indexed_count().await.expect("indexed_count"), 1);

    // index_trigram
    search
        .index_trigram(probe, "Probe Title")
        .await
        .expect("index_trigram");

    // keyword / exact_title / search
    assert_eq!(search.keyword("probe", 5).await.expect("keyword").len(), 1);
    assert!(search
        .exact_title("no such title", 5)
        .await
        .expect("exact")
        .is_empty());
    assert_eq!(search.search("probe", 5).await.expect("search").len(), 1);

    // describe — the batch lookup the vector half needs, since it returns bare ids.
    // An id that is not in the catalogue is absent rather than an error: an index built
    // against an older snapshot is a real situation, not a corruption.
    let described = search
        .describe(&[probe, probe, 999_999], MatchReason::Semantic)
        .await
        .expect("describe");
    assert_eq!(described.len(), 1, "duplicates collapse, strangers vanish");
    assert_eq!(described[0].media_item_id, probe);
    assert_eq!(described[0].why, MatchReason::Semantic);
    assert!(
        search
            .describe(&[], MatchReason::Semantic)
            .await
            .expect("describe of nothing")
            .is_empty(),
        "an empty id list must not build a query with no placeholders"
    );

    // details — the byline under a result. Nothing credited yet: every field empty.
    let bare = search.details(&[probe, 999_999]).await.expect("details");
    assert_eq!(bare.len(), 1, "strangers vanish here too");
    assert_eq!(
        (bare[0].made_by.as_deref(), bare[0].runtime_minutes),
        (None, None)
    );
    assert!(search
        .details(&[])
        .await
        .expect("details of nothing")
        .is_empty());

    // Billing order decides, unbilled sorts last, a creator ranks after any director,
    // and only two names are kept. Inserted directly: credits have no repository yet.
    for (name, role, billing) in [
        ("Third", "director", None),
        ("Creator", "creator", Some(0)),
        ("Second", "director", Some(2)),
        ("First", "director", Some(1)),
    ] {
        let person: i64 = sqlx::query_scalar("INSERT INTO people (name) VALUES (?) RETURNING id")
            .bind(name)
            .fetch_one(db.pool())
            .await
            .expect("person");
        sqlx::query(
            "INSERT INTO credits (media_item_id, person_id, role, billing) VALUES (?, ?, ?, ?)",
        )
        .bind(probe)
        .bind(person)
        .bind(role)
        .bind(billing)
        .execute(db.pool())
        .await
        .expect("credit");
    }
    let credited = search.details(&[probe]).await.expect("details");
    assert_eq!(credited[0].made_by.as_deref(), Some("First, Second"));
    // The credits must not leak into the filter checks below, which expect none.
    sqlx::query("DELETE FROM credits WHERE media_item_id = ?")
        .bind(probe)
        .execute(db.pool())
        .await
        .expect("clear credits");

    // admitted — structured filters over a candidate set (subtask 5.5).
    // The probe is "Probe Title" (1970), no runtime, no director.
    assert_eq!(
        search
            .admitted(&[probe], Some((1960, 1979)), None, None, None)
            .await
            .expect("admitted: year in range"),
        vec![probe]
    );
    assert!(search
        .admitted(&[probe], Some((1980, 1989)), None, None, None)
        .await
        .expect("admitted: year out of range")
        .is_empty());
    // A NULL runtime cannot satisfy a runtime bound. It must be excluded deliberately
    // rather than by SQL's three-valued logic doing it for the wrong reason.
    assert!(
        search
            .admitted(&[probe], None, Some(200), None, None)
            .await
            .expect("admitted: null runtime")
            .is_empty(),
        "an item with no runtime cannot satisfy a runtime filter"
    );
    // No director credited, so a director filter admits nothing.
    assert!(search
        .admitted(&[probe], None, None, None, Some("Kurosawa"))
        .await
        .expect("admitted: director")
        .is_empty());
    // No filters at all is a pass-through, not an empty set.
    assert_eq!(
        search
            .admitted(&[probe], None, None, None, None)
            .await
            .expect("admitted: no filters"),
        vec![probe]
    );
    assert!(
        search
            .admitted(&[], Some((1900, 2000)), None, None, None)
            .await
            .expect("admitted: no candidates")
            .is_empty(),
        "an empty candidate list must not build a query with no placeholders"
    );

    // by_filters — retrieval BY a filter, for a query that is nothing but filters
    let found = search
        .by_filters(Some((1960, 1979)), None, None, None, 5)
        .await
        .expect("by_filters: year");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].media_item_id, probe);
    assert!(search
        .by_filters(Some((1980, 1989)), None, None, None, 5)
        .await
        .expect("by_filters: year miss")
        .is_empty());
    // No constraints at all would be "every title, most popular first" — a browse
    // surface, not a search result.
    assert!(
        search
            .by_filters(None, None, None, None, 5)
            .await
            .expect("by_filters: unconstrained")
            .is_empty(),
        "an unconstrained filter query must return nothing, not everything"
    );

    // ── CatalogueRepository ───────────────────────────────────────────────────
    let catalogue = CatalogueRepository::new(&db);

    // searchable_titles — the probe item above is the only one here
    assert_eq!(
        catalogue
            .searchable_titles()
            .await
            .expect("searchable_titles"),
        1
    );

    // core_ids — the probe item is not in the core tier, so the list is empty rather
    // than "every title". The artefact is positional, so an over-broad core query would
    // shift every vector onto the wrong film; this asserts the predicate, not the count.
    assert!(
        catalogue.core_ids().await.expect("core_ids").is_empty(),
        "a title with in_core = 0 must not appear in artefact order"
    );
    sqlx::query("UPDATE media_items SET in_core = 1 WHERE id = ?")
        .bind(probe)
        .execute(db.pool())
        .await
        .expect("promote to core");
    assert_eq!(
        catalogue.core_ids().await.expect("core_ids"),
        vec![probe],
        "and one that is in_core must appear exactly once"
    );

    // readiness — titles present and no ingest job ever ran, which is what a prebuilt
    // index looks like
    assert_eq!(
        catalogue.readiness().await.expect("readiness"),
        Readiness::Ready { titles: 1 }
    );

    // steps — no job, so nothing to report rather than an error
    assert!(catalogue.steps().await.expect("steps").is_empty());

    // ── WikipediaRepository (ADR-0033, migration 0014) ────────────────────────
    let wiki = WikipediaRepository::new(&db);

    // map — a mapping for an IMDb id we do not hold is nothing to do, not an error.
    // Wikidata knows 509,464 of them and this catalogue holds a subset.
    assert!(
        !wiki
            .map("tt9999999", "Some Article")
            .await
            .expect("map miss"),
        "an unknown imdb id stores nothing and says so"
    );
    MediaRepository::new(&db)
        .add_external_id(probe, IdSource::Imdb, "tt0000001", 1.0)
        .await
        .expect("probe imdb id");
    assert!(wiki.map("tt0000001", "Probe Article").await.expect("map"));
    // Re-mapping after an article was moved updates the title and must NOT re-queue.
    assert!(wiki
        .map("tt0000001", "Probe Article (film)")
        .await
        .expect("re-map"));

    // pending — mapped and not yet fetched
    let pending = wiki.pending(10).await.expect("pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].media_item_id, probe);
    assert_eq!(pending[0].article, "Probe Article (film)");

    // counts — (mapped, fetched, with a synopsis)
    assert_eq!(wiki.counts().await.expect("counts"), (1, 0, 0));

    // store — the synopsis and the fetch marker commit together
    wiki.store(probe, "A film about a probe.")
        .await
        .expect("store");
    assert!(
        wiki.pending(10)
            .await
            .expect("pending after store")
            .is_empty(),
        "a fetched article leaves the queue"
    );
    assert_eq!(wiki.counts().await.expect("counts"), (1, 1, 1));

    // mark_empty — an article Wikipedia has no lead for must not be re-asked forever
    let second = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Second Probe", 1971))
        .await
        .expect("second item");
    MediaRepository::new(&db)
        .add_external_id(second, IdSource::Imdb, "tt0000002", 1.0)
        .await
        .expect("second imdb id");
    assert!(wiki.map("tt0000002", "Blank Article").await.expect("map"));
    wiki.mark_empty(second).await.expect("mark_empty");
    assert!(wiki
        .pending(10)
        .await
        .expect("pending after mark_empty")
        .is_empty());
    assert_eq!(
        wiki.counts().await.expect("counts"),
        (2, 2, 1),
        "both fetched, only one carries text"
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

    // compact — on a file database, so both sizes are real and VACUUM is meaningful
    let compacted = db.compact().await.expect("compact");
    assert!(compacted.before > 0, "measured a real file");
    assert!(
        compacted.after <= compacted.before,
        "VACUUM never grows the file"
    );
    assert!(db.schema_version().await.expect("still open").is_some());

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

/// `SourceRepository` (Phase 6, migration 0015).
#[tokio::test]
async fn source_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let repo = SourceRepository::new(&db);

    // ensure — new rows at the end, existing ones untouched
    repo.ensure(&["local", "archive"]).await.expect("ensure");
    repo.set_enabled("archive", false)
        .await
        .expect("set_enabled");
    repo.ensure(&["archive", "direct"])
        .await
        .expect("ensure again");

    // all — in order, and the user's switch survived the second ensure
    let all = repo.all().await.expect("all");
    let ids: Vec<&str> = all.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["local", "archive", "direct"]);
    assert!(
        !all[1].enabled,
        "ensure must not re-enable what the user turned off"
    );
    assert!(repo.set_enabled("nope", true).await.is_err());

    // reorder — must name every backend exactly once
    let order = ["direct", "local", "archive"].map(String::from);
    repo.reorder(&order).await.expect("reorder");
    let ids: Vec<String> = repo
        .all()
        .await
        .unwrap()
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(ids, order);
    assert!(
        repo.reorder(&["direct".to_string()]).await.is_err(),
        "a partial order"
    );

    // record — counts, the weighted latency, and the last error
    repo.record("local", &Sample::Answered { latency_ms: 100.0 })
        .await
        .expect("record");
    repo.record("local", &Sample::Answered { latency_ms: 200.0 })
        .await
        .unwrap();
    repo.record(
        "local",
        &Sample::Failed {
            error: "x".repeat(5_000),
        },
    )
    .await
    .unwrap();
    repo.record("local", &Sample::TimedOut).await.unwrap();
    repo.record("gone", &Sample::TimedOut)
        .await
        .expect("an unknown id is ignored");
    let local = repo
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|b| b.id == "local")
        .unwrap();
    assert_eq!((local.successes, local.failures, local.timeouts), (2, 1, 1));
    assert!(
        (local.latency_ms.unwrap() - 120.0).abs() < 1e-9,
        "100 + 0.2 × (200 − 100)"
    );
    assert_eq!(local.last_error.as_deref(), Some("timed out"));
    assert!(local.last_outcome_at.is_some());

    // install_addon, addons — and reinstalling replaces the manifest in place
    repo.install_addon("addon:com.example.a", "https://a.example/m.json", "{}")
        .await
        .expect("install_addon");
    repo.set_enabled("addon:com.example.a", false)
        .await
        .unwrap();
    repo.install_addon(
        "addon:com.example.a",
        "https://a.example/m2.json",
        r#"{"v":2}"#,
    )
    .await
    .expect("reinstall");
    let addons = repo.addons().await.expect("addons");
    assert_eq!(addons.len(), 1);
    assert_eq!(addons[0].manifest_url, "https://a.example/m2.json");
    assert!(!addons[0].installed_at.is_empty());
    let row = repo
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|b| b.id == "addon:com.example.a")
        .unwrap();
    assert!(!row.enabled, "an update keeps the user's switch");
    assert_eq!(row.position, 3);

    // remove_addon — the registry row and the manifest go together; built-ins cannot
    repo.remove_addon("addon:com.example.a")
        .await
        .expect("remove_addon");
    assert!(repo.addons().await.unwrap().is_empty());
    assert_eq!(repo.all().await.unwrap().len(), 3);
    assert!(
        repo.remove_addon("local").await.is_err(),
        "a built-in is switched off, not deleted"
    );
}

/// `DirectLinkRepository` (Phase 6, migration 0016).
#[tokio::test]
async fn direct_link_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let repo = DirectLinkRepository::new(&db);

    // add — and the same URL again updates rather than duplicating
    let link = |t: &str, y, u: &str| (t.to_string(), y, u.to_string());
    let n = repo
        .add(&[
            link("Nosferatu", Some(1922), "https://a.example/nosferatu.mp4"),
            link("Metropolis", None, "https://a.example/metropolis.mp4"),
        ])
        .await
        .expect("add");
    assert_eq!(n, 2);
    repo.add(&[link(
        "Metropolis",
        Some(1927),
        "https://a.example/metropolis.mp4",
    )])
    .await
    .expect("add again");

    // all — ordered by title, one row per URL
    let all = repo.all().await.expect("all");
    assert_eq!(all.len(), 2);
    assert_eq!(
        (all[0].title.as_str(), all[0].year),
        ("Metropolis", Some(1927))
    );

    // remove
    assert!(repo.remove(all[0].id).await.expect("remove"));
    assert!(
        !repo.remove(all[0].id).await.expect("remove again"),
        "already gone"
    );
    assert_eq!(repo.all().await.unwrap().len(), 1);
}

/// `SubtitleRepository` (Phase 10, migration 0018).
#[tokio::test]
async fn subtitle_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let repo = SubtitleRepository::new(&db);

    // nudge / set_nudge — per file AND subtitle; zero removes it
    assert_eq!(repo.nudge("hash:1", "embedded:3").await.unwrap(), None);
    repo.set_nudge("hash:1", "embedded:3", 300)
        .await
        .expect("set");
    repo.set_nudge("hash:1", "embedded:3", -200)
        .await
        .expect("update");
    repo.set_nudge("hash:1", "file:x.srt", 900)
        .await
        .expect("another subtitle");
    assert_eq!(
        repo.nudge("hash:1", "embedded:3").await.unwrap(),
        Some(-200)
    );
    assert_eq!(repo.nudge("hash:1", "file:x.srt").await.unwrap(), Some(900));
    repo.set_nudge("hash:1", "embedded:3", 0)
        .await
        .expect("clear");
    assert_eq!(repo.nudge("hash:1", "embedded:3").await.unwrap(), None);

    // quota / reserve / release / set_server_remaining — never past the budget
    let day = "2026-11-06";
    assert_eq!(
        repo.quota("opensubtitles", day).await.unwrap(),
        QuotaDay::default()
    );
    assert!(repo.reserve("opensubtitles", day, 2).await.unwrap());
    assert!(repo.reserve("opensubtitles", day, 2).await.unwrap());
    assert!(
        !repo.reserve("opensubtitles", day, 2).await.unwrap(),
        "budget spent"
    );
    assert_eq!(repo.quota("opensubtitles", day).await.unwrap().used, 2);
    repo.release("opensubtitles", day).await.expect("release");
    assert_eq!(repo.quota("opensubtitles", day).await.unwrap().used, 1);
    // The provider says none is left: that wins over our own budget.
    repo.set_server_remaining("opensubtitles", day, 0)
        .await
        .expect("server");
    assert!(!repo.reserve("opensubtitles", day, 20).await.unwrap());
    assert_eq!(
        repo.quota("opensubtitles", day).await.unwrap(),
        QuotaDay {
            used: 1,
            server_remaining: Some(0)
        }
    );
    // A new day starts fresh.
    assert!(repo
        .reserve("opensubtitles", "2026-11-07", 20)
        .await
        .unwrap());

    // cached / remember
    assert_eq!(repo.cached("8e245d9679d31e12", "en").await.unwrap(), None);
    let entry = CachedSubtitle {
        provider: "opensubtitles".into(),
        file_id: "1234".into(),
        relative_path: "cache/subtitles/8e245d9679d31e12.en.srt".into(),
    };
    // remember — refuses a path that would not travel with the data folder
    let absolute = CachedSubtitle {
        relative_path: "C:/Users/x/sub.srt".into(),
        ..entry.clone()
    };
    assert!(repo
        .remember("8e245d9679d31e12", "en", &absolute)
        .await
        .is_err());
    repo.remember("8e245d9679d31e12", "en", &entry)
        .await
        .expect("remember");
    assert_eq!(
        repo.cached("8e245d9679d31e12", "en").await.unwrap(),
        Some(entry)
    );
    assert_eq!(repo.cached("8e245d9679d31e12", "fr").await.unwrap(), None);
}

/// `BackfillRepository` (Phase 6, ADR-0030 layer 3, migration 0017).
#[tokio::test]
async fn backfill_repository_surface() {
    let db = Db::in_memory().await.expect("open");
    let repo = BackfillRepository::new(&db);
    let media = MediaRepository::new(&db);
    let search = SearchRepository::new(&db);

    // existing — sees an ordinary catalogue row by its normalised title
    let real = media
        .insert(&NewMediaItem::film("Metropolis", 1927))
        .await
        .expect("film");
    sqlx::query("UPDATE titles SET normalised = 'metropolis' WHERE media_item_id = ?")
        .bind(real)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        repo.existing("METROPOLIS!", Some(1927))
            .await
            .expect("existing"),
        Some(real)
    );
    assert_eq!(
        repo.existing("Metropolis", Some(2001)).await.unwrap(),
        None,
        "another year"
    );

    // create_film — findable by all three text tiers at once
    let id = repo
        .create_film("Häxan", Some(1922), "internet-archive")
        .await
        .expect("create_film");
    assert_eq!(repo.existing("haxan", Some(1922)).await.unwrap(), Some(id));
    let exact = search.exact_title("Haxan", 5).await.expect("exact");
    assert_eq!(exact.first().map(|h| h.media_item_id), Some(id));
    assert!(search
        .keyword("häxan", 5)
        .await
        .unwrap()
        .iter()
        .any(|h| h.media_item_id == id));
    assert!(search
        .fuzzy("haxxan", 5)
        .await
        .unwrap()
        .iter()
        .any(|h| h.media_item_id == id));
    assert!(
        repo.create_film("  ", None, "x").await.is_err(),
        "an empty title"
    );

    // all — only the marked rows
    let rows = repo.all().await.expect("all");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].id, rows[0].from.as_str()),
        (id, "internet-archive")
    );
}

/// `MediaRepository::credits` and `ProfileRepository::ensure_default` (Phase 8).
#[tokio::test]
async fn detail_and_default_profile_surface() {
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    let film = media
        .insert(&NewMediaItem::film("Sunrise", 1927))
        .await
        .unwrap();
    for (name, role, character, billing) in [
        ("Janet Gaynor", "actor", "The Wife", Some(2)),
        ("F. W. Murnau", "director", "", None),
        ("George O'Brien", "actor", "The Man", Some(1)),
        ("Extra", "actor", "", None),
    ] {
        let person: i64 = sqlx::query_scalar("INSERT INTO people (name) VALUES (?) RETURNING id")
            .bind(name)
            .fetch_one(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO credits (media_item_id, person_id, role, character, billing) VALUES (?, ?, ?, ?, ?)")
            .bind(film)
            .bind(person)
            .bind(role)
            .bind(character)
            .bind(billing)
            .execute(db.pool())
            .await
            .unwrap();
    }
    // credits — the director first, then the cast by billing, the unbilled last
    let credits = media.credits(film, 10).await.expect("credits");
    let names: Vec<&str> = credits.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        ["F. W. Murnau", "George O'Brien", "Janet Gaynor", "Extra"]
    );
    assert_eq!(credits[1].character.as_deref(), Some("The Man"));
    assert_eq!(credits[0].character, None, "an empty character is None");
    assert_eq!(media.credits(film, 2).await.unwrap().len(), 2, "limit");

    // filmography (Phase 11) — from a credit's person id; films, not episodes; newest first
    let gaynor = credits[2].person_id;
    let later = media
        .insert(&NewMediaItem::film("Seventh Heaven", 1927))
        .await
        .unwrap();
    let newer = media
        .insert(&NewMediaItem::film("A Star Is Born", 1937))
        .await
        .unwrap();
    for (item, character) in [(later, "Diane"), (newer, "Esther")] {
        sqlx::query("INSERT INTO credits (media_item_id, person_id, role, character) VALUES (?, ?, 'actor', ?)")
            .bind(item)
            .bind(gaynor)
            .bind(character)
            .execute(db.pool())
            .await
            .unwrap();
    }
    let films = media.filmography(gaynor, 10).await.expect("filmography");
    assert_eq!(
        films
            .iter()
            .map(|a| (a.title.as_str(), a.year))
            .collect::<Vec<_>>(),
        [
            ("A Star Is Born", Some(1937)),
            ("Seventh Heaven", Some(1927)),
            ("Sunrise", Some(1927))
        ]
    );
    assert_eq!(films[0].character.as_deref(), Some("Esther"));
    assert_eq!(
        media.filmography(gaynor, 1).await.unwrap().len(),
        1,
        "limit"
    );

    // external_id
    media
        .add_external_id(film, IdSource::Imdb, "tt0018455", 1.0)
        .await
        .unwrap();
    assert_eq!(
        media
            .external_id(film, "imdb")
            .await
            .expect("external_id")
            .as_deref(),
        Some("tt0018455")
    );
    assert_eq!(media.external_id(film, "tmdb").await.unwrap(), None);

    // ensure_default — creates once, then returns the same profile
    let profiles = ProfileRepository::new(&db);
    let first = profiles.ensure_default().await.expect("ensure_default");
    assert_eq!(profiles.ensure_default().await.unwrap(), first);
    assert_eq!(profiles.all().await.unwrap().len(), 1);
}

/// Exercise every public method on `LibraryRepository` (Phase 12, migration 0021).
#[tokio::test]
async fn library_repository_surface() {
    use sinephile_persistence::repositories::{
        FileMatch, LibraryRepository, MatchMethod, NewLocalFile, Probe, Upserted,
    };
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    let heat = media
        .insert(&NewMediaItem::film("Heat", 1995))
        .await
        .expect("film");
    let other = media
        .insert(&NewMediaItem::film("Heat", 1986))
        .await
        .expect("film");
    let repo = LibraryRepository::new(&db);

    // add_folder (idempotent) / folders / mark_folder_scanned
    let folder = repo.add_folder(r"D:\Films").await.expect("add");
    assert_eq!(repo.add_folder(r"D:\Films").await.expect("again"), folder);
    repo.mark_folder_scanned(folder).await.expect("scanned");

    // upsert_file: new, unchanged, changed
    let f = NewLocalFile {
        folder_id: folder,
        path: r"D:\Films\Heat.1995.mkv",
        size_bytes: 1000,
        modified_at: Some("2024-01-01T00:00:00Z"),
    };
    let Upserted::New(id) = repo.upsert_file(&f).await.expect("insert") else {
        panic!("new")
    };
    assert_eq!(repo.upsert_file(&f).await.unwrap(), Upserted::Unchanged(id));

    // set_probe / set_content_hash / file / file_by_path / files_in_folder
    repo.set_probe(
        id,
        &Probe {
            duration_seconds: Some(10_200),
            video_codec: Some("h264".into()),
            audio_codec: Some("aac".into()),
            width: Some(1920),
            height: Some(1080),
            audio_languages: vec!["en".into(), "fr".into()],
            container_title: Some("Heat".into()),
        },
    )
    .await
    .expect("probe");
    repo.set_content_hash(id, "abc").await.expect("hash");
    let got = repo.file(id).await.unwrap().expect("file");
    assert!(got.probed);
    assert_eq!(got.audio_languages, vec!["en", "fr"]);
    assert_eq!(got.content_hash.as_deref(), Some("abc"));
    assert_eq!(repo.file_by_path(f.path).await.unwrap().unwrap().id, id);
    assert_eq!(repo.files_in_folder(folder).await.unwrap().len(), 1);
    assert_eq!(repo.folders().await.unwrap()[0].files, 1);

    // set_match / match_of: a confirmed match is not replaced by an unconfirmed one
    let confirmed = FileMatch {
        media_item_id: heat,
        confidence: 1.0,
        method: MatchMethod::Manual,
        covers: 1,
        confirmed: true,
    };
    repo.set_match(id, &confirmed).await.expect("match");
    repo.set_match(
        id,
        &FileMatch {
            media_item_id: other,
            confidence: 0.9,
            method: MatchMethod::Filename,
            covers: 1,
            confirmed: false,
        },
    )
    .await
    .expect("ignored");
    assert_eq!(repo.match_of(id).await.unwrap(), Some(confirmed.clone()));
    assert_eq!(repo.files_for_item(heat).await.unwrap().len(), 1);

    // a changed file keeps a confirmed match, and its probe is cleared
    let changed = NewLocalFile {
        size_bytes: 2000,
        ..f.clone()
    };
    assert_eq!(
        repo.upsert_file(&changed).await.unwrap(),
        Upserted::Changed(id)
    );
    assert!(!repo.file(id).await.unwrap().unwrap().probed);
    assert!(repo.match_of(id).await.unwrap().unwrap().confirmed);

    // clear_match / set_candidates / candidates / review_queue / review_count
    repo.clear_match(id).await.expect("clear");
    assert_eq!(repo.match_of(id).await.unwrap(), None);
    repo.set_candidates(id, &[(heat, 0.6), (other, 0.3)])
        .await
        .expect("candidates");
    let c = repo.candidates(id).await.unwrap();
    assert_eq!(
        (c[0].media_item_id, c[0].release_year, c.len()),
        (heat, Some(1995), 2)
    );
    assert_eq!(repo.review_queue(0.8, 10).await.unwrap().len(), 1);
    assert_eq!(repo.review_count(0.8).await.unwrap(), 1);
    repo.set_match(
        id,
        &FileMatch {
            media_item_id: heat,
            confidence: 0.85,
            method: MatchMethod::Filename,
            covers: 1,
            confirmed: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(repo.review_count(0.8).await.unwrap(), 0, "confident enough");
    assert_eq!(
        repo.review_count(0.9).await.unwrap(),
        1,
        "not for a higher bar"
    );

    // mark_missing / missing_with_size / move_file: a rename keeps the row and its match
    repo.mark_missing(id).await.expect("missing");
    assert_eq!(repo.missing_with_size(2000).await.unwrap()[0].id, id);
    assert_eq!(
        repo.review_count(0.9).await.unwrap(),
        0,
        "missing files wait"
    );
    repo.move_file(id, r"D:\Films\Heat (1995).mkv", folder)
        .await
        .expect("move");
    let moved = repo.file(id).await.unwrap().unwrap();
    assert_eq!(
        (moved.path.as_str(), moved.missing_since),
        (r"D:\Films\Heat (1995).mkv", None)
    );
    assert_eq!(
        repo.match_of(id).await.unwrap().unwrap().media_item_id,
        heat
    );

    // purge_missing / delete_file
    let other_file = repo
        .upsert_file(&NewLocalFile {
            path: r"D:\Films\gone.mkv",
            ..f.clone()
        })
        .await
        .unwrap()
        .id();
    repo.mark_missing(other_file).await.unwrap();
    assert_eq!(repo.purge_missing(folder).await.unwrap(), 1);
    repo.delete_file(id).await.expect("delete");
    assert_eq!(repo.file(id).await.unwrap(), None);

    // remember / recall
    assert_eq!(repo.recall("heat|1995|movie").await.unwrap(), None);
    repo.remember("heat|1995|movie", other).await.unwrap();
    repo.remember("heat|1995|movie", heat).await.unwrap();
    assert_eq!(repo.recall("heat|1995|movie").await.unwrap(), Some(heat));

    // remove_folder removes its files (0021's trigger)
    let again = repo.upsert_file(&f).await.unwrap().id();
    repo.remove_folder(folder).await.expect("remove");
    assert!(repo.folders().await.unwrap().is_empty());
    assert_eq!(repo.file(again).await.unwrap(), None);
}
