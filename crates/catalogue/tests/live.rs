//! ADR-0054 step 3: AniList's and TMDB's recommendations beside MovieLens, asked for what a
//! profile loved, once a week.

use std::sync::Arc;

use sinephile_catalogue::live::{self, Report};
use sinephile_metadata_api::anilist::AniList;
use sinephile_metadata_api::tmdb::Tmdb;
use sinephile_metadata_api::{FakeTransport, Response, Transport};
use sinephile_persistence::repositories::{
    MediaRepository, NeighbourRepository, ProfileRepository,
};
use sinephile_persistence::{Db, IdSource, NewMediaItem};

/// Obviously not a key: valid in shape only.
const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn title(db: &Db, name: &str, ids: &[(IdSource, &str)]) -> i64 {
    let media = MediaRepository::new(db);
    let id = media
        .insert(&NewMediaItem::film(name, 2026))
        .await
        .expect("insert");
    for (source, value) in ids {
        media
            .add_external_id(id, *source, value, 1.0)
            .await
            .expect("external id");
    }
    id
}

#[tokio::test]
async fn what_a_profile_loved_gets_live_neighbours_scored_beside_movielens() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let anime = title(&db, "Loved Anime", &[(IdSource::Anilist, "11")]).await;
    let near_anime = title(&db, "Its Sequel", &[(IdSource::Anilist, "22")]).await;
    let other_anime = title(&db, "Another", &[(IdSource::Anilist, "21")]).await;
    let film = title(&db, "Loved Film", &[(IdSource::Imdb, "tt0200001")]).await;
    let new_film = title(&db, "This Month's", &[(IdSource::Imdb, "tt0200002")]).await;
    let known_film = title(
        &db,
        "Known",
        &[(IdSource::Imdb, "tt0200003"), (IdSource::Tmdb, "movie/503")],
    )
    .await;
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.expect("profile");
    for id in [anime, film] {
        profiles.rate(me, id, Some(5)).await.expect("rate");
    }
    // A MovieLens matrix to be scored beside: its first neighbours average 0.35.
    let neighbours = NeighbourRepository::new(&db);
    neighbours
        .replace_all(&[(new_film, known_film, 0.4), (known_film, new_film, 0.3)])
        .await
        .expect("matrix");

    let anilist_fake = FakeTransport::new();
    anilist_fake.push(Response::new(
        200,
        r#"{"data":{"Page":{"media":[{"id":11,"recommendations":{"nodes":[
            {"rating":5,"mediaRecommendation":{"id":21}},
            {"rating":40,"mediaRecommendation":{"id":22}},
            {"rating":3,"mediaRecommendation":{"id":999}}
        ]}}]}}}"#,
    ));
    // TMDB, asked in this order (pushed in reverse): the loved film found, its
    // recommendations, then the IMDb ids of the two not met before.
    let tmdb_fake = Arc::new(FakeTransport::new());
    tmdb_fake.push(Response::new(200, r#"{"imdb_id":null}"#));
    tmdb_fake.push(Response::new(200, r#"{"imdb_id":"tt0200002"}"#));
    tmdb_fake.push(Response::new(
        200,
        r#"{"results":[{"id":502},{"id":503},{"id":504}]}"#,
    ));
    tmdb_fake.push(Response::new(
        200,
        r#"{"movie_results":[{"id":501,"poster_path":null}],"tv_results":[]}"#,
    ));
    let anilist = AniList::new(&anilist_fake).await;
    let tmdb = Tmdb::new(tmdb_fake.clone() as Arc<dyn Transport>, KEY);

    let report = live::keep_current(&db, Some(&anilist), Some(&tmdb))
        .await
        .expect("live");
    assert_eq!(
        report,
        Report {
            anilist_asked: 1,
            anilist_rows: 2,
            tmdb_asked: 1,
            tmdb_rows: 2,
        }
    );
    // AniList's best first; the one the catalogue lacks leaves its place empty. Scored as
    // MovieLens' neighbours at the same rank (0.35 at the first, and its last after).
    let of_anime = neighbours.of(anime, 10).await.unwrap();
    assert_eq!(of_anime.len(), 2);
    assert_eq!(of_anime[0].0, near_anime);
    assert!((of_anime[0].1 - 0.35).abs() < 1e-6, "{of_anime:?}");
    assert_eq!(of_anime[1].0, other_anime);
    // TMDB: one found through its IMDb id, one already known by its TMDB id.
    let of_film: Vec<i64> = neighbours
        .of(film, 10)
        .await
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(of_film.len(), 2);
    assert!(of_film.contains(&new_film) && of_film.contains(&known_film));
    let media = MediaRepository::new(&db);
    assert_eq!(
        media
            .external_id(new_film, "tmdb")
            .await
            .unwrap()
            .as_deref(),
        Some("movie/502"),
        "kept, never looked up again"
    );
    assert_eq!(
        media.external_id(film, "tmdb").await.unwrap().as_deref(),
        Some("movie/501")
    );
    // A MovieLens rebuild leaves them be.
    neighbours.replace_all(&[]).await.unwrap();
    assert_eq!(neighbours.of(anime, 10).await.unwrap().len(), 2);

    // A week has not passed: nothing is asked.
    let quiet = FakeTransport::new();
    let quiet_tmdb = Arc::new(FakeTransport::new());
    let anilist = AniList::new(&quiet).await;
    let tmdb = Tmdb::new(quiet_tmdb.clone() as Arc<dyn Transport>, KEY);
    let again = live::keep_current(&db, Some(&anilist), Some(&tmdb))
        .await
        .expect("again");
    assert_eq!(again, Report::default());
    assert!(quiet.requests.lock().unwrap().is_empty());
    assert!(quiet_tmdb.requests.lock().unwrap().is_empty());
}

#[test]
fn a_live_rank_scores_what_movielens_scores_at_that_rank() {
    assert_eq!(live::score_at(&[0.3, 0.2], 0), 0.3);
    assert_eq!(live::score_at(&[0.3, 0.2], 1), 0.2);
    assert_eq!(live::score_at(&[0.3, 0.2], 7), 0.2, "its last past the end");
    // No matrix yet: the curve fitted to ml-latest's.
    assert!((live::score_at(&[], 0) - 0.29).abs() < 1e-6);
    assert!((live::score_at(&[], 9) - 0.163).abs() < 0.01);
    assert!((live::score_at(&[], 49) - 0.109).abs() < 0.01);
}
