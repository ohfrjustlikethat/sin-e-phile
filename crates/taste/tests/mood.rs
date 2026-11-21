//! Phase 18: a mood shelf from a real (migrated) database and a small meaning index: nearest
//! the mood first, a profile's taste breaking near-ties, its length kept, and nothing another
//! rail shows.

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_taste::hybrid::Meaning;
use sinephile_taste::modes::Mode;
use sinephile_taste::mood::{shelf, MOODS};

struct Space(HashMap<i64, Vec<f32>>);

fn unit(v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.into_iter().map(|x| x / n).collect()
}

impl Meaning for Space {
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

async fn film(db: &Db, title: &str, minutes: i64, votes: i64) -> i64 {
    let mut item = NewMediaItem::film(title, 2000);
    item.runtime_minutes = Some(minutes);
    item.rating_votes = Some(votes);
    MediaRepository::new(db).insert(&item).await.unwrap()
}

#[tokio::test]
async fn a_shelf_is_the_mood_first_its_length_kept_and_nothing_shown_twice() {
    let db = Db::in_memory().await.unwrap();
    let mut space = HashMap::new();
    // The mood points along the first axis. A taste points along the second.
    let closest = film(&db, "Closest to the mood", 95, 5_000).await;
    space.insert(closest, unit(vec![1.0, 0.0, 0.05]));
    let near_taste = film(&db, "Near the mood, near the taste", 95, 5_000).await;
    space.insert(near_taste, unit(vec![1.0, 0.25, 0.0]));
    let near_plain = film(&db, "Near the mood, far from the taste", 95, 5_000).await;
    space.insert(near_plain, unit(vec![1.0, -0.25, 0.0]));
    let too_long = film(&db, "Close but long", 150, 5_000).await;
    space.insert(too_long, unit(vec![1.0, 0.0, 0.0]));
    let shown = film(&db, "Already on another rail", 90, 5_000).await;
    space.insert(shown, unit(vec![1.0, 0.0, 0.01]));
    let taste_only = film(&db, "The taste's, far from the mood", 90, 5_000).await;
    space.insert(taste_only, unit(vec![0.5, 0.866, 0.0]));
    let far = film(&db, "Far from the mood", 90, 900_000).await;
    space.insert(far, unit(vec![0.0, 0.0, 1.0]));
    let mut space = Space(space);

    let tense = MOODS.iter().find(|m| m.key == "mood_tense").unwrap();
    assert_eq!(tense.max_minutes, Some(100));
    let taste = vec![Mode {
        direction: unit(vec![0.0, 1.0, 0.0]),
        weight: 1.0,
        members: vec![],
    }];
    let taken: HashSet<i64> = [shown].into_iter().collect();
    let ids = shelf(&db, &mut space, tense, &[1.0, 0.0, 0.0], &taste, &taken, 5)
        .await
        .unwrap();
    assert!(!ids.contains(&too_long), "over 100 minutes: {ids:?}");
    assert!(!ids.contains(&shown), "on another rail: {ids:?}");
    let at = |id: i64| ids.iter().position(|x| *x == id).unwrap();
    assert!(
        at(closest) < at(taste_only),
        "the mood leads the taste: {ids:?}"
    );
    assert!(
        at(near_taste) < at(near_plain),
        "the taste breaks a near-tie: {ids:?}"
    );
    assert_eq!(
        *ids.last().unwrap(),
        far,
        "the most seen, far from the mood, last: {ids:?}"
    );
    assert_eq!(ids.len(), 5);
}
