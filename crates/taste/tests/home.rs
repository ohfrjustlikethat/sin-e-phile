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
    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
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

    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
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

    let kids = rails(&db, None, me, true, None, &Default::default())
        .await
        .unwrap();
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

/// This year, as Home reckons it.
fn this_year() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as f64;
    (secs / 86_400.0 / 365.2425 + 1970.0).floor() as i64
}

#[tokio::test]
async fn home_is_in_the_authors_order_with_new_episodes_the_library_and_new_releases() {
    // Phase 16, the author's order: continue, new episodes of your shows, your library,
    // new releases, then taste, watchlist, widely loved.
    use sinephile_persistence::model::MediaKind;
    use sinephile_persistence::repositories::library::{FileMatch, MatchMethod, NewLocalFile};
    use sinephile_persistence::repositories::{EpisodeRepository, LibraryRepository};

    let db = Db::in_memory().await.unwrap();
    let year = this_year();
    let classic = film(&db, "Heat", 1995, 900_000, &["Crime"]).await;
    let premiere = film(&db, "A Film Just Out", year, 50_000, &["Drama"]).await;
    let quiet = film(&db, "Barely Seen", year, 40, &["Drama"]).await;
    let mine = film(&db, "On My Disk", 1981, 20_000, &["Crime"]).await;

    // A show in the library (one episode on disk), and a newer episode the catalogue
    // gained this month.
    let media = MediaRepository::new(&db);
    let show = media
        .insert(&NewMediaItem {
            kind: Some(MediaKind::Series),
            rating_votes: Some(30_000),
            ..NewMediaItem::film("A Followed Show", 2024)
        })
        .await
        .unwrap();
    let episodes = EpisodeRepository::new(&db);
    episodes.create_series(show, false).await.unwrap();
    let season = episodes.create_season(show, 2, None).await.unwrap();
    let mut eps = Vec::new();
    for n in 1..=2 {
        let id = media
            .insert(&NewMediaItem {
                kind: Some(MediaKind::Episode),
                ..NewMediaItem::film(format!("Part {n}"), year)
            })
            .await
            .unwrap();
        episodes
            .create_episode(id, show, Some(season), Some(2), Some(n), None)
            .await
            .unwrap();
        eps.push(id);
    }
    let library = LibraryRepository::new(&db);
    let folder = library.add_folder("C:/Films").await.unwrap();
    for (path, item) in [
        ("C:/Films/a.mkv", mine),
        ("C:/Films/show/s02e01.mkv", eps[0]),
    ] {
        let up = library
            .upsert_file(&NewLocalFile {
                folder_id: folder,
                path,
                size_bytes: 1,
                modified_at: None,
            })
            .await
            .unwrap();
        library
            .set_match(
                up.id(),
                &FileMatch {
                    media_item_id: item,
                    confidence: 1.0,
                    method: MatchMethod::Filename,
                    covers: 1,
                    confirmed: false,
                },
            )
            .await
            .unwrap();
    }
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    // The episode on disk was watched; the next one is new.
    ProfileRepository::new(&db)
        .record_viewing(me, eps[0], 2_400, true, 2_400)
        .await
        .unwrap();

    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
    let keys: Vec<&str> = r.iter().map(|x| x.key).collect();
    assert_eq!(keys, ["new_episodes", "library", "new", "loved"], "{r:?}");
    assert_eq!(r[0].items, vec![show], "one card, the series");
    assert_eq!(
        r[0].notes.get(&show).map(String::as_str),
        Some("New: S2 E2")
    );
    assert!(
        r[1].items.contains(&mine) && r[1].items.contains(&show),
        "{:?}",
        r[1]
    );
    assert_eq!(
        r[2].items,
        vec![premiere],
        "out and watched; not the 40-vote film"
    );
    assert!(!r[2].items.contains(&quiet));
    assert!(r[3].items.contains(&classic));
}
