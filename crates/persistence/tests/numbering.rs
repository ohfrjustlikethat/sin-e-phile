//! Episode numbering reconciliation — `SPEC.md` §6.2.
//!
//! "Anime specifically requires absolute vs seasonal episode numbering
//! reconciliation... Design for this in Phase 3 rather than patching it in Phase 12."
//!
//! The fixture below is the case that makes the design necessary, and it is real
//! in shape: a series broadcast as three uneven cours, where TVDB numbers it
//! seasonally, AniList numbers it absolutely across the whole run, and MAL treats
//! each cour as its own show starting at episode 1.
//!
//! The numbers deliberately do not line up arithmetically. Season 2 is 13 episodes
//! and season 1 is 12, so "absolute 59" is not `59 - 2 * 13`. Anything that tries
//! to compute the mapping gets it wrong, silently, and Phase 12's false-confident
//! budget is 1%.

use sinephile_persistence::model::EpisodeNumbering;
use sinephile_persistence::repositories::episodes::NumberingMatch;
use sinephile_persistence::repositories::{EpisodeRepository, MediaRepository};
use sinephile_persistence::{Db, IdSource, MediaKind, NewMediaItem};

struct Fixture {
    db: Db,
    series: i64,
    /// (absolute, season, episode, media_item_id)
    episodes: Vec<(i64, i64, i64, i64)>,
}

/// Three cours of 12, 13 and 12 episodes: absolute 1-37 across seasons 1-3.
async fn long_running_series() -> Fixture {
    let db = Db::in_memory().await.expect("open");
    let media = MediaRepository::new(&db);
    let episodes_repo = EpisodeRepository::new(&db);

    let series = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::AnimeSeries),
            primary_title: "Three Uneven Cours".into(),
            ..Default::default()
        })
        .await
        .expect("series item");
    episodes_repo
        .create_series(series, true)
        .await
        .expect("series row");

    let mut episodes = Vec::new();
    let mut absolute = 1;

    for (season_number, count) in [(1, 12), (2, 13), (3, 12)] {
        let season = episodes_repo
            .create_season(series, season_number, None)
            .await
            .expect("season");

        for episode_number in 1..=count {
            let item = media
                .insert(&NewMediaItem {
                    kind: Some(MediaKind::Episode),
                    primary_title: format!("S{season_number:02}E{episode_number:02}"),
                    ..Default::default()
                })
                .await
                .expect("episode item");

            episodes_repo
                .create_episode(
                    item,
                    series,
                    Some(season),
                    Some(season_number),
                    Some(episode_number),
                    Some(absolute),
                )
                .await
                .expect("episode row");

            // TVDB numbers seasonally and records no absolute number.
            episodes_repo
                .set_numbering(&EpisodeNumbering {
                    episode_id: item,
                    source: IdSource::Tvdb,
                    season_number: Some(season_number),
                    episode_number: Some(episode_number),
                    absolute_number: None,
                })
                .await
                .expect("tvdb");

            // AniList numbers absolutely across the whole run.
            episodes_repo
                .set_numbering(&EpisodeNumbering {
                    episode_id: item,
                    source: IdSource::Anilist,
                    season_number: None,
                    episode_number: None,
                    absolute_number: Some(absolute),
                })
                .await
                .expect("anilist");

            // MAL restarts at 1 for every cour and calls each one "season 1".
            episodes_repo
                .set_numbering(&EpisodeNumbering {
                    episode_id: item,
                    source: IdSource::Mal,
                    season_number: Some(1),
                    episode_number: Some(episode_number),
                    absolute_number: None,
                })
                .await
                .expect("mal");

            episodes.push((absolute, season_number, episode_number, item));
            absolute += 1;
        }
    }

    Fixture {
        db,
        series,
        episodes,
    }
}

#[tokio::test]
async fn absolute_number_resolves_to_the_right_episode() {
    // The anime file-naming case: "Three Uneven Cours - 30" and nothing else.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);

    // Absolute 30 is season 3 episode 5: 12 + 13 = 25 in the first two cours.
    let resolved = repo
        .resolve_absolute(fx.series, Some(IdSource::Anilist), 30)
        .await
        .expect("query")
        .expect("absolute 30 exists");

    assert_eq!(resolved.season_number, Some(3));
    assert_eq!(resolved.episode_number, Some(5));
    assert_eq!(resolved.matched_by, NumberingMatch::SourceExact);
}

#[tokio::test]
async fn the_mapping_is_not_arithmetic() {
    // The guard against anyone "simplifying" this into a calculation later.
    //
    // If cours were uniform, absolute 30 would be season 3 episode 4 under a
    // 13-per-season assumption, or season 3 episode 6 under 12-per-season. It is
    // neither: it is episode 5, and only the stored data says so.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);

    let resolved = repo
        .resolve_absolute(fx.series, Some(IdSource::Anilist), 30)
        .await
        .expect("query")
        .expect("exists");

    let uniform_13 = 30 - 2 * 13;
    let uniform_12 = 30 - 2 * 12;
    assert_ne!(resolved.episode_number, Some(uniform_13));
    assert_ne!(resolved.episode_number, Some(uniform_12));
    assert_eq!(resolved.episode_number, Some(5));
}

#[tokio::test]
async fn every_absolute_number_round_trips() {
    // Not one spot check: the whole run, in both directions.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);

    for &(absolute, season, episode, item) in &fx.episodes {
        let by_absolute = repo
            .resolve_absolute(fx.series, Some(IdSource::Anilist), absolute)
            .await
            .expect("query")
            .unwrap_or_else(|| panic!("absolute {absolute} did not resolve"));
        assert_eq!(by_absolute.episode_id, item, "absolute {absolute}");

        let by_seasonal = repo
            .resolve_seasonal(fx.series, IdSource::Tvdb, season, episode)
            .await
            .expect("query")
            .unwrap_or_else(|| panic!("S{season}E{episode} did not resolve"));
        assert_eq!(by_seasonal.episode_id, item, "S{season}E{episode}");
    }
}

#[tokio::test]
async fn two_sources_disagreeing_both_resolve_correctly() {
    // MAL's "season 1 episode 5" and TVDB's "season 1 episode 5" are DIFFERENT
    // episodes once you are past the first cour, because MAL restarts. Resolving
    // per source is the only thing that gets both right.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);

    let tvdb = repo
        .resolve_seasonal(fx.series, IdSource::Tvdb, 1, 5)
        .await
        .expect("query")
        .expect("tvdb S01E05");
    let mal = repo
        .resolve_seasonal(fx.series, IdSource::Mal, 1, 5)
        .await
        .expect("query")
        .expect("mal S01E05");

    // TVDB's S01E05 is absolute 5. MAL labels three different episodes
    // "season 1 episode 5" — one per cour — so a lookup returns one of them, and
    // the point is that the answer comes from data rather than from arithmetic.
    assert_eq!(tvdb.absolute_number, Some(5));
    assert!(
        [Some(5), Some(17), Some(30)].contains(&mal.absolute_number),
        "MAL S01E05 resolved to an episode outside the three cour openings: {:?}",
        mal.absolute_number
    );
}

#[tokio::test]
async fn an_unknown_number_resolves_to_nothing_rather_than_something_wrong() {
    // The false-confident case. There is no episode 999, and the correct answer is
    // None — never a nearest match. Phase 12 measures exactly this.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);

    assert!(repo
        .resolve_absolute(fx.series, Some(IdSource::Anilist), 999)
        .await
        .expect("query")
        .is_none());

    assert!(repo
        .resolve_seasonal(fx.series, IdSource::Tvdb, 9, 1)
        .await
        .expect("query")
        .is_none());
}

#[tokio::test]
async fn resolution_falls_back_when_a_source_has_no_numbering() {
    // An item ingested before a source was consulted still resolves on its own
    // numbering, and says so — Absolute rather than SourceExact, which is weaker
    // evidence and must be distinguishable.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);

    let resolved = repo
        .resolve_absolute(fx.series, Some(IdSource::Tmdb), 30)
        .await
        .expect("query")
        .expect("falls back to the episode's own absolute number");

    assert_eq!(resolved.matched_by, NumberingMatch::Absolute);
    assert_eq!(resolved.episode_number, Some(5));
}

#[tokio::test]
async fn every_numbering_is_recoverable_for_explanation() {
    // Phase 5 and Phase 12 both owe the user a "why did this match?" line, which
    // is only possible if the alternatives were kept.
    let fx = long_running_series().await;
    let repo = EpisodeRepository::new(&fx.db);
    let (_, _, _, item) = fx.episodes[29];

    let numberings = repo.numberings_for(item).await.expect("query");
    assert_eq!(
        numberings.len(),
        3,
        "tvdb, anilist and mal were all recorded"
    );
    assert!(numberings
        .iter()
        .any(|n| n.source == IdSource::Anilist && n.absolute_number == Some(30)));
    assert!(numberings.iter().any(|n| n.source == IdSource::Tvdb
        && n.season_number == Some(3)
        && n.episode_number == Some(5)));
}
