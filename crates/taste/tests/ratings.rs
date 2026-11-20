//! Ratings for any title (ADR-0056): half stars feed the taste model, a show by its seasons'
//! average, and the rate-a-batch screen asks about well-known films not yet rated.

use sinephile_persistence::model::{MediaKind, NewMediaItem};
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::Db;
use sinephile_taste::model::signals;
use sinephile_taste::modes::{weigh, Signal};
use sinephile_taste::rate::{batch, MIN_VOTES};

async fn film(db: &Db, title: &str, votes: i64) -> i64 {
    let mut f = NewMediaItem::film(title, 1990);
    f.rating_votes = Some(votes);
    MediaRepository::new(db).insert(&f).await.unwrap()
}

#[tokio::test]
async fn half_stars_and_a_shows_seasons_reach_the_taste_model() {
    let db = Db::in_memory().await.unwrap();
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.unwrap();
    let loved = film(&db, "Loved", 1_000).await;
    let liked = film(&db, "Liked", 1_000).await;
    let mut show = NewMediaItem::film("A Show", 2015);
    show.kind = Some(MediaKind::Series);
    let show = MediaRepository::new(&db).insert(&show).await.unwrap();
    profiles.rate(me, loved, Some(9)).await.unwrap();
    profiles.rate(me, liked, Some(7)).await.unwrap();
    profiles.rate_season(me, show, 1, Some(10)).await.unwrap();
    profiles.rate_season(me, show, 2, Some(6)).await.unwrap();

    let s = signals(&db, me, 0.0).await.unwrap();
    let rated = |id: i64| {
        s.iter()
            .find_map(|(i, sig, _)| match sig {
                Signal::Rated(stars) if *i == id => Some(*stars),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(rated(loved), 4.5);
    assert_eq!(rated(liked), 3.5);
    assert_eq!(
        rated(show),
        4.0,
        "the seasons' average, one signal for the show"
    );
    let w = weigh(&s);
    let of = |id: i64| w.iter().find(|(i, _)| *i == id).unwrap().1;
    assert!(of(loved) > of(show) && of(show) > of(liked), "{w:?}");
}

#[tokio::test]
async fn the_batch_asks_about_well_known_films_not_yet_rated_or_skipped() {
    let db = Db::in_memory().await.unwrap();
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.unwrap();
    let biggest = film(&db, "Biggest", 2_000_000).await;
    let big = film(&db, "Big", 900_000).await;
    let known = film(&db, "Known", MIN_VOTES).await;
    let _obscure = film(&db, "Obscure", MIN_VOTES - 1).await;
    assert_eq!(
        batch(&db, me, false, 10).await.unwrap(),
        vec![biggest, big, known]
    );
    assert_eq!(batch(&db, me, false, 2).await.unwrap(), vec![biggest, big]);
    profiles.rate(me, biggest, Some(8)).await.unwrap();
    profiles.skip_rating(me, big).await.unwrap();
    assert_eq!(batch(&db, me, false, 10).await.unwrap(), vec![known]);
}
