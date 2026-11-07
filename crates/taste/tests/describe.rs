//! Phase 15: a taste described from what its modes hold, and corrections kept per profile.

use sinephile_persistence::model::NewMediaItem;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::Db;
use sinephile_taste::axes::Axes;
use sinephile_taste::describe::{correct, corrections, describe};
use sinephile_taste::modes::Mode;

async fn film(db: &Db, title: &str, year: i64, votes: i64, genre: &str, director: &str) -> i64 {
    films(db, title, year, votes, &[genre], director).await
}

async fn films(
    db: &Db,
    title: &str,
    year: i64,
    votes: i64,
    genres: &[&str],
    director: &str,
) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem {
            rating_votes: Some(votes),
            ..NewMediaItem::film(title, year)
        })
        .await
        .unwrap();
    for genre in genres {
        sqlx::query("INSERT INTO genres (name) VALUES (?) ON CONFLICT DO NOTHING")
            .bind(genre)
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_genres (media_item_id, genre_id) SELECT ?, id FROM genres WHERE name = ?")
            .bind(id)
            .bind(genre)
            .execute(db.pool())
            .await
            .unwrap();
    }
    let person: i64 = sqlx::query_scalar("INSERT INTO people (name) VALUES (?) RETURNING id")
        .bind(director)
        .fetch_one(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO credits (media_item_id, person_id, role) VALUES (?, ?, 'director')")
        .bind(id)
        .bind(person)
        .execute(db.pool())
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn each_mode_is_named_by_what_it_holds() {
    let db = Db::in_memory().await.unwrap();
    let mut horror = Vec::new();
    for (i, (t, y, v)) in [
        ("Night Fog", 1978, 300_000),
        ("The Pale House", 1982, 900_000),
        ("Cold Hands", 1979, 500_000),
        ("Below Stairs", 1981, 100_000),
    ]
    .into_iter()
    .enumerate()
    {
        let who = if i < 3 { "Ann Director" } else { "Bo Other" };
        horror.push(film(&db, t, y, v, "Horror", who).await);
    }
    let mut docs = Vec::new();
    // Three genres a title: a genre's share is of the titles (History is on half of them),
    // not of all six labels (where it would be a sixth, too little to say).
    for (t, v, g) in [
        ("Rivers", 20_000, ["Documentary", "History", "Music"]),
        ("Salt", 40_000, ["Documentary", "News", "Sport"]),
    ] {
        docs.push(films(&db, t, 2015, v, &g, "Cy Maker").await);
    }
    let modes = vec![
        Mode {
            direction: vec![1.0],
            weight: 0.7,
            members: horror.clone(),
        },
        Mode {
            direction: vec![0.0],
            weight: 0.3,
            members: docs.clone(),
        },
    ];
    let axes = Axes {
        avoids: vec![("War".into(), 0.6)],
        ..Default::default()
    };
    let d = describe(&db, &modes, &axes, &Default::default())
        .await
        .unwrap();
    assert_eq!(
        d.modes[0].sentence,
        "You gravitate toward horror, mostly from the 1970s and 80s, Ann Director above all (The Pale House, Cold Hands, Night Fog)."
    );
    assert_eq!(d.modes[0].anchor, horror[0]);
    assert_eq!(d.modes[0].examples, vec![horror[1], horror[2], horror[0]]);
    assert_eq!(
        d.modes[1].sentence,
        "You also love documentary and history, mostly from the 2010s, Cy Maker above all (Salt, Rivers)."
    );
    assert_eq!(d.axes, vec!["You tend to walk out of war."]);
}

#[tokio::test]
async fn corrections_are_kept_per_profile() {
    let db = Db::in_memory().await.unwrap();
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.unwrap();
    let other = profiles.create("Other", false).await.unwrap();
    correct(&db, me, 7, Some("not_me")).await.unwrap();
    correct(&db, me, 9, Some("more")).await.unwrap();
    correct(&db, me, 9, Some("nonsense")).await.unwrap();
    let mine = corrections(&db, me).await.unwrap();
    assert_eq!(mine.get(&7).map(String::as_str), Some("not_me"));
    assert_eq!(mine.get(&9), None, "an unknown correction clears it");
    assert!(corrections(&db, other).await.unwrap().is_empty());
    correct(&db, me, 7, None).await.unwrap();
    assert!(corrections(&db, me).await.unwrap().is_empty());
}
