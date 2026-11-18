//! Phase 17: Home's taste rail through the hybrid ranker, end to end over a real database.
//! The meaning index is a few hand-made vectors (the real one needs the model's files).

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::{MediaRepository, NeighbourRepository};
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_recommend::rank::Reason;
use sinephile_taste::axes::Axes;
use sinephile_taste::hybrid::{note, rail, Meaning};
use sinephile_taste::modes::Mode;

/// Vectors by title; nearest by cosine.
struct Hand(HashMap<i64, Vec<f32>>);

impl Meaning for Hand {
    fn nearest(&self, direction: &[f32], k: usize, exclude: &HashSet<i64>) -> Vec<i64> {
        let mut by: Vec<(f32, i64)> = self
            .0
            .iter()
            .filter(|(id, _)| !exclude.contains(id))
            .map(|(id, v)| (v.iter().zip(direction).map(|(a, b)| a * b).sum(), *id))
            .collect();
        by.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        by.into_iter().take(k).map(|(_, id)| id).collect()
    }

    fn vector(&self, id: i64) -> Option<Vec<f32>> {
        self.0.get(&id).cloned()
    }
}

async fn film(db: &Db, title: &str, year: i64, votes: i64) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem::film(title, year))
        .await
        .unwrap();
    sqlx::query("UPDATE media_items SET rating_votes = ? WHERE id = ?")
        .bind(votes)
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn the_rail_mixes_meaning_and_people_who_loved_the_same_and_says_why() {
    let db = Db::in_memory().await.expect("open");
    let stalker = film(&db, "Stalker", 1979, 150_000).await;
    let solaris = film(&db, "Solaris", 1972, 100_000).await;
    let seen_it = film(&db, "Mirror", 1975, 60_000).await;
    let loved_together = film(&db, "Come and See", 1985, 90_000).await;
    let near_in_meaning = film(&db, "Ivan's Childhood", 1962, 40_000).await;
    let both = film(&db, "Andrei Rublev", 1966, 50_000).await;
    let blockbuster = film(&db, "A Very Popular Film", 2010, 2_000_000).await;

    let mut hand = Hand(HashMap::from([
        (stalker, vec![1.0, 0.0]),
        (solaris, vec![0.98, 0.2]),
        (seen_it, vec![0.99, 0.1]),
        (near_in_meaning, vec![0.95, 0.31]),
        // Only modestly close in meaning (cosine 0.4): the matrix is what sets it apart, so its
        // reason names the loved films, whatever the tuned weights.
        (both, vec![0.4, 0.92]),
        (blockbuster, vec![0.0, 1.0]),
        // Come and See has no vector: only the matrix can bring it.
    ]));
    NeighbourRepository::new(&db)
        .replace_all(&[
            (stalker, loved_together, 0.9),
            (stalker, both, 0.5),
            (stalker, seen_it, 0.8),
            (solaris, both, 0.4),
        ])
        .await
        .unwrap();
    let modes = vec![Mode {
        direction: vec![1.0, 0.0],
        weight: 1.0,
        members: vec![stalker, solaris],
    }];
    let met: HashSet<i64> = [stalker, solaris, seen_it].into();
    let liked = [(stalker, 1.0), (solaris, 0.8)];

    let picks = rail(
        &db,
        &mut hand,
        &modes,
        &met,
        &liked,
        &Axes::default(),
        &HashSet::new(),
        10,
    )
    .await
    .unwrap();
    let ids: Vec<i64> = picks.iter().map(|p| p.id).collect();
    for m in &met {
        assert!(!ids.contains(m), "already met: {ids:?}");
    }
    let reason = |id: i64| picks.iter().find(|p| p.id == id).map(|p| p.reason.clone());
    assert_eq!(
        reason(loved_together),
        Some(Reason::LovedWith(vec![stalker]))
    );
    assert_eq!(
        reason(both),
        Some(Reason::LovedWith(vec![stalker, solaris]))
    );
    assert_eq!(reason(near_in_meaning), Some(Reason::Mode(0)));
    assert!(
        ids.contains(&blockbuster),
        "the widely seen pool is there too: {ids:?}"
    );
    // Loved alongside what was loved, and near it in meaning, ranks first.
    assert_eq!(ids[0], both, "{picks:?}");

    let titles: HashMap<i64, String> = [(stalker, "Stalker"), (solaris, "Solaris")]
        .map(|(id, t)| (id, t.to_string()))
        .into();
    let said = picks.iter().find(|p| p.id == both).unwrap();
    assert_eq!(
        note(said, &modes, &titles),
        "Loved by people who loved Stalker and Solaris"
    );
    let said = picks.iter().find(|p| p.id == near_in_meaning).unwrap();
    assert_eq!(
        note(said, &modes, &titles),
        "Close to Stalker and Solaris, which you loved"
    );
}
