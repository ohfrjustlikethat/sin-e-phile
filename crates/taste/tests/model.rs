//! Phase 15: abandoning early and late mean different things (E2), and a taste that changes
//! over time is followed (E4), from the database rows through the model.

use sinephile_persistence::model::NewMediaItem;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::Db;
use sinephile_taste::axes::axes;
use sinephile_taste::model::signals;
use sinephile_taste::modes::{modes, weigh, Signal};
use sinephile_taste::Statements;

async fn film(db: &Db, title: &str, genre: &str) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem {
            runtime_minutes: Some(100),
            ..NewMediaItem::film(title, 1980)
        })
        .await
        .unwrap();
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
    id
}

#[tokio::test]
async fn abandoning_at_five_minutes_and_at_eighty_mean_different_things() {
    let db = Db::in_memory().await.unwrap();
    let early = film(&db, "Walked Out Of", "Horror").await;
    let late = film(&db, "Nearly Finished", "Drama").await;
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let profiles = ProfileRepository::new(&db);
    profiles
        .record_viewing(me, early, 300, false, 300)
        .await
        .unwrap();
    profiles
        .record_viewing(me, late, 4_800, false, 4_800)
        .await
        .unwrap();

    let w = weigh(&signals(&db, me, 20_764.0).await.unwrap());
    let of = |id| w.iter().find(|(i, _)| *i == id).unwrap().1;
    assert!(
        of(early) < -0.5,
        "five minutes in: not for me ({})",
        of(early)
    );
    assert!(
        of(late) > -0.25 && of(late) < 0.0,
        "eighty minutes in: barely a negative ({})",
        of(late)
    );

    // And it shows in the axes: what was walked out of early is what it turns away from.
    let a = axes(&db, &w, &Statements::default()).await.unwrap();
    assert_eq!(a.avoids[0].0, "Horror", "{:?}", a.avoids);
}

#[test]
fn a_taste_that_moves_is_followed_and_the_old_one_fades() {
    // Twelve horror films loved two years ago; twelve documentaries loved last month.
    let horror = |i: usize| (i as i64, vec![1.0, 0.01 * i as f32, 0.0]);
    let docs = |i: usize| (100 + i as i64, vec![0.0, 0.01 * i as f32, 1.0]);
    let run = |horror_age: f64, docs_age: f64| {
        let mut signals = Vec::new();
        for i in 0..12 {
            signals.push((horror(i).0, Signal::Completed, horror_age));
            signals.push((docs(i).0, Signal::Completed, docs_age));
        }
        let w = weigh(&signals);
        let liked: Vec<(i64, Vec<f32>, f64)> = w
            .iter()
            .map(|(id, weight)| {
                let v = if *id < 100 {
                    horror(*id as usize).1
                } else {
                    docs((*id - 100) as usize).1
                };
                (*id, v, *weight)
            })
            .collect();
        modes(&liked)
    };
    let now = run(730.0, 30.0);
    assert!(
        now[0].members.iter().all(|id| *id >= 100),
        "the strongest mode is the recent taste"
    );
    assert!(
        now[0].weight > 0.7,
        "and the old one has faded to a small share: {}",
        now[0].weight
    );
    // The other way round: the model follows time, not the genre.
    let then = run(30.0, 730.0);
    assert!(then[0].members.iter().all(|id| *id < 100));
}
