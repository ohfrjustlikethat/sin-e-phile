//! Home's rails (Phase 14): never empty, never showing what was seen as new, and a kids
//! profile's only what suits children.

use sinephile_persistence::model::NewMediaItem;
use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::Db;
use sinephile_taste::home::rails;

async fn film(db: &Db, title: &str, year: i64, votes: i64, genres: &[&str]) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem {
            rating_votes: Some(votes),
            ..NewMediaItem::film(title, year)
        })
        .await
        .unwrap();
    for g in genres {
        sqlx::query("INSERT INTO genres (name) VALUES (?) ON CONFLICT DO NOTHING")
            .bind(g)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_genres (media_item_id, genre_id) SELECT ?, id FROM genres WHERE name = ?")
            .bind(id)
            .bind(g)
            .execute(db.pool())
            .await
            .unwrap();
    }
    id
}

#[tokio::test]
async fn a_new_profile_with_nothing_still_has_a_home() {
    let db = Db::in_memory().await.unwrap();
    let heat = film(&db, "Heat", 1995, 900_000, &["Crime"]).await;
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let r = rails(&db, None, me, false, None).await.unwrap();
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!((r[0].key, r[0].items.clone()), ("loved", vec![heat]));
}

#[tokio::test]
async fn started_saved_and_loved_each_in_their_place_and_kids_see_only_what_suits_them() {
    let db = Db::in_memory().await.unwrap();
    let heat = film(&db, "Heat", 1995, 900_000, &["Crime"]).await;
    let toy = film(&db, "Toy Story", 1995, 800_000, &["Animation", "Children"]).await;
    let paddington = film(&db, "Paddington", 2014, 100_000, &["Family"]).await;
    let thief = film(&db, "Thief", 1981, 50_000, &["Crime"]).await;
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.unwrap();
    profiles
        .save_position(
            me,
            &PlaybackPosition {
                media_item_id: heat,
                position_seconds: 600,
                duration_seconds: Some(10_000),
                audio_language: None,
                subtitle_language: None,
            },
        )
        .await
        .unwrap();
    profiles.add_to_watchlist(me, thief, None).await.unwrap();
    profiles
        .add_to_watchlist(me, paddington, None)
        .await
        .unwrap();

    let r = rails(&db, None, me, false, None).await.unwrap();
    let keys: Vec<&str> = r.iter().map(|x| x.key).collect();
    assert_eq!(
        keys,
        ["continue", "watchlist", "loved"],
        "no vector half: no taste rail"
    );
    assert_eq!(r[0].items, vec![heat]);
    assert_eq!(
        r[2].items,
        vec![toy],
        "what is started or saved is not offered again as new"
    );

    let kids = rails(&db, None, me, true, None).await.unwrap();
    let all: Vec<i64> = kids.iter().flat_map(|x| x.items.clone()).collect();
    assert!(all.contains(&paddington) && all.contains(&toy), "{kids:?}");
    assert!(
        !all.contains(&heat) && !all.contains(&thief),
        "crime is not for a kids profile"
    );
    assert!(
        kids.iter().all(|x| !x.items.is_empty()),
        "empty rails are left out"
    );
}

#[tokio::test]
async fn a_round_spreads_across_decades_and_never_repeats() {
    use sinephile_taste::rounds::round;
    use std::collections::HashSet;
    let db = Db::in_memory().await.unwrap();
    let mut by_decade = Vec::new();
    for (i, year) in [1925, 1955, 1975, 1995, 2015].iter().enumerate() {
        let a = film(&db, &format!("A{i}"), *year, 1_000_000 - i as i64, &[]).await;
        let b = film(&db, &format!("B{i}"), *year, 900_000 - i as i64, &[]).await;
        by_decade.push((a, b));
    }
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let first = round(&db, None, me, false, &HashSet::new(), 5)
        .await
        .unwrap();
    // One from each decade, newest first, before any decade's second.
    let firsts: Vec<i64> = by_decade.iter().rev().map(|(a, _)| *a).collect();
    assert_eq!(first, firsts);
    let shown: HashSet<i64> = first.iter().copied().collect();
    let second = round(&db, None, me, false, &shown, 5).await.unwrap();
    assert!(
        second.iter().all(|id| !shown.contains(id)),
        "never shown twice"
    );
    assert_eq!(second.len(), 5);
}
