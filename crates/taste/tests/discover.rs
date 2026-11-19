//! Phase 18: the discovery rails gathered from a real (migrated) database and a small meaning
//! index: every label holds of its title (E3), an episode counts as its series, and nothing is
//! said of a country until Wikidata was asked about everything the profile met (ADR-0055).

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::{
    EpisodeRepository, MediaRepository, RegionRepository, Regions,
};
use sinephile_persistence::{Db, MediaKind, NewMediaItem};
use sinephile_recommend::strategy::{self, Arm, Axis, Label, Region};
use sinephile_taste::discover;
use sinephile_taste::hybrid::Meaning;
use sinephile_taste::modes::Mode;

/// Brute force over a handful of vectors: the real index needs the model's files.
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

struct World {
    db: Db,
    space: Space,
    modes: Vec<Mode>,
    met: HashSet<i64>,
    /// Acclaimed films from the 1950s, which the profile never met.
    fifties: Vec<i64>,
    iranian: i64,
    series: i64,
}

async fn genre(db: &Db, id: i64, name: &str) {
    sqlx::query("INSERT OR IGNORE INTO genres (name) VALUES (?)")
        .bind(name)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO media_genres (media_item_id, genre_id) SELECT ?, id FROM genres WHERE name = ?",
    )
    .bind(id)
    .bind(name)
    .execute(db.pool())
    .await
    .unwrap();
}

async fn film(db: &Db, title: &str, year: i64, genres: &[&str], rating: i64, votes: i64) -> i64 {
    let mut item = NewMediaItem::film(title, year);
    item.rating = Some(rating);
    item.rating_votes = Some(votes);
    let id = MediaRepository::new(db).insert(&item).await.unwrap();
    for g in genres {
        genre(db, id, g).await;
    }
    id
}

async fn world() -> World {
    let db = Db::in_memory().await.expect("open");
    let mut space = HashMap::new();
    let mut met = HashSet::new();
    // Twelve dramas from the 1970s and 1980s the profile loved: two tastes, one near the
    // first axis and one near the second.
    let mut members = (Vec::new(), Vec::new());
    for k in 0..12 {
        let id = film(
            &db,
            &format!("Loved {k}"),
            1970 + k * 2,
            &["Drama"],
            80,
            50_000,
        )
        .await;
        met.insert(id);
        let (v, list) = if k % 2 == 0 {
            (vec![1.0, 0.05 * k as f32, 0.0], &mut members.0)
        } else {
            (vec![0.05 * k as f32, 1.0, 0.0], &mut members.1)
        };
        space.insert(id, unit(v));
        list.push(id);
    }
    // A series it watched an episode of: the series is a western, the episode has no genre.
    let mut s = NewMediaItem::film("A Western Series", 1975);
    s.kind = Some(MediaKind::Series);
    let series = MediaRepository::new(&db).insert(&s).await.unwrap();
    genre(&db, series, "Western").await;
    let episodes = EpisodeRepository::new(&db);
    episodes.create_series(series, false).await.unwrap();
    let mut e = NewMediaItem::film("Pilot", 1975);
    e.kind = Some(MediaKind::Episode);
    let episode = MediaRepository::new(&db).insert(&e).await.unwrap();
    episodes
        .create_episode(episode, series, None, Some(1), Some(1), None)
        .await
        .unwrap();
    met.insert(episode);

    // Acclaimed films the profile never met: dramas from the 1950s near its first taste, a
    // western from the 1970s near it, and an Iranian drama from the 1970s near the second.
    let mut fifties = Vec::new();
    for k in 0..6 {
        let id = film(
            &db,
            &format!("Fifties {k}"),
            1950 + k,
            &["Drama"],
            81,
            9_000,
        )
        .await;
        space.insert(id, unit(vec![1.0, 0.1, 0.3]));
        fifties.push(id);
    }
    let western = film(&db, "A Western", 1972, &["Western"], 82, 20_000).await;
    space.insert(western, unit(vec![1.0, 0.0, 0.2]));
    let iranian = film(&db, "An Iranian Drama", 1978, &["Drama"], 80, 8_000).await;
    space.insert(iranian, unit(vec![0.1, 1.0, 0.25]));
    // Filler, far from both tastes, so "close" means something.
    for k in 0..30 {
        let id = film(&db, &format!("Far {k}"), 2010, &["Comedy"], 50, 900).await;
        space.insert(id, unit(vec![0.0, 0.0, 1.0 + k as f32]));
    }
    let modes = vec![
        Mode {
            direction: unit(vec![1.0, 0.2, 0.0]),
            weight: 0.5,
            members: members.0,
        },
        Mode {
            direction: unit(vec![0.2, 1.0, 0.0]),
            weight: 0.5,
            members: members.1,
        },
    ];
    World {
        db,
        space: Space(space),
        modes,
        met,
        fifties,
        iranian,
        series,
    }
}

#[tokio::test]
async fn every_label_holds_and_an_episode_counts_as_its_series() {
    let mut w = world().await;
    let gathered = discover::gather(&w.db, &mut w.space, &w.modes, &w.met, &HashMap::new())
        .await
        .expect("gather");
    // An episode counts as its series: the profile met a western.
    assert!(gathered
        .profile
        .met
        .contains(&Region::Genre("Western".into())));
    assert_eq!(
        gathered.profile.met_titles, 13,
        "twelve films and one series"
    );
    let rails = discover::rails(&gathered, 20, &HashSet::new());
    let by_id: HashMap<i64, &strategy::Title> = gathered.titles.iter().map(|t| (t.id, t)).collect();
    assert!(!rails.is_empty());
    for rail in &rails {
        for d in &rail.picks {
            assert!(!w.met.contains(&d.id), "{} was met", d.id);
            assert!(
                strategy::holds(&d.label, by_id[&d.id], &gathered.profile),
                "{d:?} in {:?}",
                rail.arm
            );
        }
    }
    // The 1950s are the blind spot, and the rail says so. (In `rails` the stretch, which runs
    // first, has taken them: "like what you loved, but from the 1950s" is as true. Which arm
    // a title goes to is the bandit's to choose, 18.2.)
    let blind = strategy::blind_spot(&gathered.titles, &gathered.profile, 20);
    assert_eq!(
        discover::rail_title(Arm::BlindSpot, &blind),
        "You've never watched anything from the 1950s"
    );
    let picked: HashSet<i64> = blind.iter().map(|d| d.id).collect();
    assert_eq!(picked, w.fifties.iter().copied().collect());
    let stretched: HashSet<i64> = rails
        .iter()
        .filter(|r| r.arm == Arm::Stretch)
        .flat_map(|r| r.picks.iter().map(|d| d.id))
        .collect();
    assert!(stretched.is_superset(&picked), "{rails:?}");
    // Not the series' genre: it was met through its episode.
    assert!(rails.iter().all(|r| r
        .picks
        .iter()
        .all(|d| d.label != Label::BlindSpot(Region::Genre("Western".into())))));
}

#[tokio::test]
async fn a_country_is_claimed_only_once_everything_met_was_asked() {
    let mut w = world().await;
    let gathered = discover::gather(&w.db, &mut w.space, &w.modes, &w.met, &HashMap::new())
        .await
        .unwrap();
    assert!(gathered.profile.unknown.contains(&Axis::Country));

    // Wikidata answers for the Iranian film and for everything the profile met, the series
    // standing for its episode.
    let regions = RegionRepository::new(&w.db);
    let mut answers = vec![Regions {
        media_item_id: w.iranian,
        countries: vec!["Iran".into()],
        movements: vec!["Iranian New Wave".into()],
    }];
    for id in w.met.iter().copied().filter(|id| *id != w.series) {
        answers.push(Regions {
            media_item_id: id,
            countries: vec!["France".into()],
            ..Default::default()
        });
    }
    regions.record(&answers).await.unwrap();
    let gathered = discover::gather(&w.db, &mut w.space, &w.modes, &w.met, &HashMap::new())
        .await
        .unwrap();
    assert!(
        gathered.profile.unknown.contains(&Axis::Country),
        "the episode was asked, its series was not: still unknown"
    );
    regions
        .record(&[Regions {
            media_item_id: w.series,
            countries: vec!["United States".into()],
            ..Default::default()
        }])
        .await
        .unwrap();
    let gathered = discover::gather(&w.db, &mut w.space, &w.modes, &w.met, &HashMap::new())
        .await
        .unwrap();
    assert!(gathered.profile.unknown.is_empty());
    // Now "from Iran" may be said of the Iranian film: a stretch from the second taste.
    let rails = discover::rails(&gathered, 20, &HashSet::new());
    let iranian = rails
        .iter()
        .flat_map(|r| &r.picks)
        .find(|d| d.id == w.iranian)
        .expect("the Iranian film is chosen");
    assert!(
        matches!(
            &iranian.label,
            Label::Stretch { region: Region::Country(c), .. } if c == "Iran"
        ) || matches!(
            &iranian.label,
            Label::Stretch { region: Region::Movement(m), .. } if m == "Iranian New Wave"
        ),
        "{iranian:?}"
    );
}
