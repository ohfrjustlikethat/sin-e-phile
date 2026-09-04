//! Episode loading.
//!
//! Every SQL statement in `episodes_load.rs` runs against a migrated database here
//! (ADR-0026's standing requirement).

use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;

use sinephile_ingest::episodes_load as ep;
use sinephile_ingest::Job;
use sinephile_persistence::Db;

/// `(source, season, episode, absolute)` as `episode_numbering` stores it.
type Numbering = (String, Option<i64>, Option<i64>, Option<i64>);

const NULL: char = '\\';

fn null() -> String {
    format!("{NULL}N")
}

fn write_gz(dir: &std::path::Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut encoder = flate2::write::GzEncoder::new(
        std::fs::File::create(&path).expect("create"),
        flate2::Compression::fast(),
    );
    encoder.write_all(contents.as_bytes()).expect("write");
    encoder.finish().expect("finish");
    path
}

fn tsv(rows: &[&[&str]]) -> String {
    rows.iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// tt10 is a popular series, tt20 an anime one, tt30 an obscure series.
async fn catalogue(db: &Db) -> (i64, i64, i64) {
    let mut ids = Vec::new();
    for (tconst, kind, title, votes) in [
        ("tt10", "series", "Popular Show", 90_000i64),
        ("tt20", "anime_series", "Anime Show", 12i64),
        ("tt30", "series", "Obscure Show", 12i64),
    ] {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO media_items (kind, primary_title, rating_votes, in_core)
             VALUES (?, ?, ?, 1) RETURNING id",
        )
        .bind(kind)
        .bind(title)
        .bind(votes)
        .fetch_one(db.pool())
        .await
        .expect("insert series");
        sqlx::query(
            "INSERT INTO external_ids (media_item_id, source, external_id) VALUES (?, 'imdb', ?)",
        )
        .bind(id)
        .bind(tconst)
        .execute(db.pool())
        .await
        .expect("insert id");
        ids.push(id);
    }
    (ids[0], ids[1], ids[2])
}

fn episode_tsv() -> String {
    let n = null();
    tsv(&[
        &["tconst", "parentTconst", "seasonNumber", "episodeNumber"],
        &["tt101", "tt10", "1", "1"],
        &["tt102", "tt10", "1", "2"],
        &["tt201", "tt20", "1", "1"],
        // An OVA: IMDb genuinely has no season or episode for it.
        &["tt202", "tt20", &n, &n],
        &["tt301", "tt30", "1", "1"],
    ])
}

fn basics_tsv() -> String {
    let n = null();
    let row = |t: &str, title: &str| {
        vec![
            t.to_string(),
            "tvEpisode".into(),
            title.into(),
            title.into(),
            "0".into(),
            "2013".into(),
            n.clone(),
            "24".into(),
            "Animation".into(),
        ]
    };
    let mut rows = vec![vec![
        "tconst".to_string(),
        "titleType".into(),
        "primaryTitle".into(),
        "originalTitle".into(),
        "isAdult".into(),
        "startYear".into(),
        "endYear".into(),
        "runtimeMinutes".into(),
        "genres".into(),
    ]];
    rows.push(row("tt101", "Pilot"));
    rows.push(row("tt102", "Second"));
    rows.push(row("tt201", "Anime One"));
    rows.push(row("tt202", "The OVA"));
    rows.push(row("tt301", "Obscure One"));
    rows.iter()
        .map(|r| r.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    episodes: std::path::PathBuf,
    basics: std::path::PathBuf,
    popular: i64,
    anime: i64,
    obscure: i64,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let (popular, anime, obscure) = catalogue(&db).await;
    let episodes = write_gz(dir.path(), "title.episode.tsv.gz", &episode_tsv());
    let basics = write_gz(dir.path(), "title.basics.tsv.gz", &basics_tsv());
    Fixture {
        _dir: dir,
        db,
        episodes,
        basics,
        popular,
        anime,
        obscure,
    }
}

async fn run(f: &Fixture, scope: ep::EpisodeScope) -> usize {
    let series = ep::series(&f.db).await.expect("series");
    let skip = ep::already_loaded(&f.db).await.expect("skip");
    let wanted = ep::collect(&f.episodes, &series, scope, &skip).expect("collect");
    if wanted.is_empty() {
        return 0;
    }
    let count = wanted.len();
    let by_id: std::collections::HashMap<u32, ep::Wanted> =
        wanted.iter().map(|w| (w.tconst, *w)).collect();

    let mut job = Job::begin(&f.db, "episodes").await.expect("begin");
    ep::load_series_rows(&mut job, Arc::new(wanted))
        .await
        .expect("series rows");
    ep::load_episodes(&mut job, f.basics.clone(), Arc::new(by_id))
        .await
        .expect("episodes");
    ep::load_seasons(&mut job).await.expect("seasons");
    job.finish().await.expect("finish");
    count
}

async fn episodes_of(db: &Db, series_id: i64) -> Vec<(String, Option<i64>, Option<i64>)> {
    sqlx::query_as(
        "SELECT m.primary_title, e.season_number, e.episode_number
           FROM episodes e JOIN media_items m ON m.id = e.media_item_id
          WHERE e.series_id = ? ORDER BY m.primary_title",
    )
    .bind(series_id)
    .fetch_all(db.pool())
    .await
    .expect("episodes")
}

#[tokio::test]
async fn anime_is_loaded_in_full_regardless_of_votes() {
    // The anime series has 12 votes — far under any threshold. SPEC.md 6.2 requires
    // its numbering anyway, and the whole anime catalogue is 132,880 episodes.
    let f = fixture().await;
    run(&f, ep::EpisodeScope::ANIME_ONLY).await;

    let anime = episodes_of(&f.db, f.anime).await;
    assert_eq!(anime.len(), 2, "both the episode and the OVA");
    assert!(episodes_of(&f.db, f.popular).await.is_empty());
    assert!(episodes_of(&f.db, f.obscure).await.is_empty());
}

#[tokio::test]
async fn a_vote_threshold_admits_popular_series_and_not_obscure_ones() {
    let f = fixture().await;
    run(
        &f,
        ep::EpisodeScope {
            all_anime: true,
            min_votes: 10_000,
        },
    )
    .await;

    assert_eq!(episodes_of(&f.db, f.popular).await.len(), 2);
    assert_eq!(episodes_of(&f.db, f.anime).await.len(), 2);
    assert!(
        episodes_of(&f.db, f.obscure).await.is_empty(),
        "12 votes is under the threshold"
    );
}

#[tokio::test]
async fn an_ova_keeps_its_missing_numbers_rather_than_being_given_a_zero() {
    // IMDb genuinely has no season or episode for some specials. Inventing a 0 would
    // be a fact we made up, and migration 0003 says both columns may be NULL.
    let f = fixture().await;
    run(&f, ep::EpisodeScope::ANIME_ONLY).await;

    let ova: (Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT e.season_number, e.episode_number, e.season_id
           FROM episodes e JOIN media_items m ON m.id = e.media_item_id
          WHERE m.primary_title = 'The OVA'",
    )
    .fetch_one(f.db.pool())
    .await
    .expect("the ova");
    assert_eq!(
        ova,
        (None, None, None),
        "no season, no episode, no season row"
    );
}

#[tokio::test]
async fn widening_the_scope_adds_episodes_without_duplicating_the_first_pass() {
    // The scope is expected to widen as R4 headroom allows. `episodes` has no natural
    // key, so nothing in the schema would stop a second pass inserting every episode
    // again — the skip set is the only thing that does.
    let f = fixture().await;
    run(&f, ep::EpisodeScope::ANIME_ONLY).await;
    assert_eq!(episodes_of(&f.db, f.anime).await.len(), 2);

    sinephile_ingest::Job::reset(&f.db, "episodes")
        .await
        .expect("reset");
    let added = run(
        &f,
        ep::EpisodeScope {
            all_anime: true,
            min_votes: 10_000,
        },
    )
    .await;

    assert_eq!(added, 2, "only the popular series was new");
    assert_eq!(
        episodes_of(&f.db, f.anime).await.len(),
        2,
        "the anime episodes were not loaded a second time"
    );
    assert_eq!(episodes_of(&f.db, f.popular).await.len(), 2);
}

#[tokio::test]
async fn seasons_are_derived_and_episodes_point_at_them() {
    // IMDb publishes no season list, so this is the only way to have one.
    let f = fixture().await;
    run(
        &f,
        ep::EpisodeScope {
            all_anime: true,
            min_votes: 10_000,
        },
    )
    .await;

    let seasons: Vec<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT series_id, season_number, episode_count FROM seasons ORDER BY series_id",
    )
    .fetch_all(f.db.pool())
    .await
    .expect("seasons");
    assert_eq!(
        seasons,
        vec![(f.popular, 1, Some(2)), (f.anime, 1, Some(1))],
        "one season each, counted from the episodes actually loaded"
    );

    let linked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM episodes WHERE season_number IS NOT NULL AND season_id IS NULL",
    )
    .fetch_one(f.db.pool())
    .await
    .expect("count");
    assert_eq!(linked, 0, "every numbered episode points at its season");
}

#[tokio::test]
async fn imdb_numbering_is_recorded_and_the_absolute_number_is_left_null() {
    // IMDb does not publish an absolute number. Computing one by counting within a
    // season would be an invention: recaps and specials interleave, which migration
    // 0003 names as the largest cause of off-by-N drift between schemes.
    let f = fixture().await;
    run(&f, ep::EpisodeScope::ANIME_ONLY).await;

    let rows: Vec<Numbering> = sqlx::query_as(
        "SELECT n.source, n.season_number, n.episode_number, n.absolute_number
           FROM episode_numbering n
           JOIN media_items m ON m.id = n.episode_id
          WHERE m.primary_title = 'Anime One'",
    )
    .fetch_all(f.db.pool())
    .await
    .expect("numbering");
    assert_eq!(rows, vec![("imdb".to_string(), Some(1), Some(1), None)]);
}

#[tokio::test]
async fn every_episode_carries_its_own_imdb_id() {
    let f = fixture().await;
    run(&f, ep::EpisodeScope::ANIME_ONLY).await;

    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT e.external_id FROM external_ids e
           JOIN media_items m ON m.id = e.media_item_id
          WHERE m.kind = 'episode' ORDER BY e.external_id",
    )
    .fetch_all(f.db.pool())
    .await
    .expect("ids");
    assert_eq!(ids, vec!["tt0000201", "tt0000202"]);
}

#[tokio::test]
async fn a_series_row_exists_before_any_episode_references_it() {
    // episodes.series_id has a foreign key onto series.media_item_id, so a missing
    // series row is not a cosmetic gap — it is a failed insert.
    let f = fixture().await;
    run(&f, ep::EpisodeScope::ANIME_ONLY).await;

    let series: Vec<(i64, Option<i64>)> =
        sqlx::query_as("SELECT media_item_id, total_episodes FROM series")
            .fetch_all(f.db.pool())
            .await
            .expect("series");
    assert_eq!(series.len(), 1, "only the series that gained episodes");
    assert_eq!(series[0].0, f.anime);

    let (episodes, seasons) = ep::count_episodes(&f.db).await.expect("counts");
    assert_eq!((episodes, seasons), (2, 1));

    let total: Option<i64> = sqlx::query_scalar("SELECT total_episodes FROM series")
        .fetch_one(f.db.pool())
        .await
        .expect("total");
    assert_eq!(
        total,
        Some(2),
        "counted from what was loaded, not from IMDb"
    );
}

#[tokio::test]
async fn an_episode_whose_parent_is_not_in_the_catalogue_is_skipped() {
    // 281,850 rows of the real dataset point at series we never indexed. A foreign key
    // violation on series_id would stop the whole load.
    let f = fixture().await;
    let extra = format!("{}{}", episode_tsv(), tsv(&[&["tt401", "tt99", "1", "1"]]));
    let episodes = write_gz(f._dir.path(), "with-orphan.tsv.gz", &extra);

    let series = ep::series(&f.db).await.expect("series");
    let wanted = ep::collect(
        &episodes,
        &series,
        ep::EpisodeScope::ANIME_ONLY,
        &HashSet::new(),
    )
    .expect("collect");

    assert!(
        wanted.iter().all(|w| w.tconst != 401),
        "the orphan is dropped before it can violate a foreign key"
    );
}
