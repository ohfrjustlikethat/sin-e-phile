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

#[tokio::test]
async fn with_a_tmdb_key_what_is_showing_now_leads_new_releases() {
    // Phase 16: the catalogue knows years; TMDB knows what is in cinemas this week.
    use sinephile_persistence::repositories::CredentialRepository;
    let db = Db::in_memory().await.unwrap();
    let year = this_year();
    let big = film(&db, "Most Watched This Year", year, 90_000, &["Drama"]).await;
    let showing = film(&db, "Just Opened", year, 2_000, &["Drama"]).await;
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();

    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
    let new = r.iter().find(|x| x.key == "new").unwrap();
    assert_eq!(new.items, vec![big, showing], "without a list: by votes");

    CredentialRepository::new(&db)
        .set(
            me,
            "tmdb.now",
            &format!(r#"{{"day":"2026-11-08","ids":[{showing}]}}"#),
        )
        .await
        .unwrap();
    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
    let new = r.iter().find(|x| x.key == "new").unwrap();
    assert_eq!(new.items, vec![showing, big], "showing now leads");
}

#[tokio::test]
async fn rising_now_follows_new_releases_with_what_each_gained() {
    // ADR-0054 step 6: what gained most at the catalogue's last refresh, kept by the
    // refresh; a title the profile has already loved is left out.
    let db = Db::in_memory().await.unwrap();
    let media = MediaRepository::new(&db);
    let climbing = film(&db, "Climbing", 2026, 50_000, &["Drama"]).await;
    let steady = film(&db, "Steady", 2019, 900_000, &["Crime"]).await;
    let loved = film(&db, "Already Loved", 2025, 30_000, &["Drama"]).await;
    for (id, previous) in [(climbing, 37_600), (steady, 899_000), (loved, 10_000)] {
        sqlx::query(
            "UPDATE media_items SET previous_votes = ?, votes_changed_at = datetime('now')
             WHERE id = ?",
        )
        .bind(previous)
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    }
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    ProfileRepository::new(&db)
        .rate(me, loved, Some(10))
        .await
        .unwrap();
    assert_eq!(media.keep_rising(2, 1_000, 200).await.unwrap(), 3);

    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
    let rising = r.iter().find(|x| x.key == "rising").expect("a rising rail");
    assert_eq!(rising.title, "Rising now");
    assert_eq!(
        rising.items,
        vec![climbing, steady],
        "best gain first, loved left out"
    );
    assert_eq!(
        rising.notes.get(&climbing).map(String::as_str),
        Some("+12,400 ratings")
    );
    assert_eq!(
        rising.notes.get(&steady).map(String::as_str),
        Some("+1,000 ratings")
    );
}

#[tokio::test]
async fn the_taste_preview_is_homes_taste_rail() {
    // 17.6: the profile's "Where your taste points so far" is Home's rail of that name, not a
    // second answer to the same question. Without the meaning index, both are empty.
    let db = Db::in_memory().await.unwrap();
    film(&db, "Heat", 1995, 900_000, &["Crime"]).await;
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let cache = Default::default();
    let home = rails(&db, None, me, false, None, &cache).await.unwrap();
    let preview = sinephile_taste::home::taste_rail(&db, None, me, false, None, &cache)
        .await
        .unwrap();
    let taste: Vec<i64> = home
        .iter()
        .find(|r| r.key == "taste")
        .map(|r| r.items.clone())
        .unwrap_or_default();
    assert_eq!(preview, taste);
}

#[tokio::test]
async fn a_dismissed_title_leaves_every_rail_and_not_right_now_comes_back() {
    // Phase 18: "not for me" and "already seen" for good; "not right now" for a fortnight.
    use sinephile_persistence::repositories::DiscoveryRepository;
    let db = Db::in_memory().await.unwrap();
    let heat = film(&db, "Heat", 1995, 900_000, &["Crime"]).await;
    let thief = film(&db, "Thief", 1981, 800_000, &["Crime"]).await;
    let ronin = film(&db, "Ronin", 1998, 700_000, &["Crime"]).await;
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let loved = |r: Vec<sinephile_taste::home::Rail>| {
        r.into_iter()
            .find(|r| r.key == "loved")
            .map(|r| r.items)
            .unwrap_or_default()
    };
    let models = sinephile_taste::model::ModelCache::default();
    let home = || rails(&db, None, me, false, None, &models);
    assert_eq!(loved(home().await.unwrap()), vec![heat, thief, ronin]);

    let discovery = DiscoveryRepository::new(&db);
    discovery
        .dismiss(me, heat, "not_for_me", "evening")
        .await
        .unwrap();
    discovery
        .dismiss(me, thief, "not_right_now", "evening")
        .await
        .unwrap();
    assert_eq!(loved(home().await.unwrap()), vec![ronin]);

    // A fortnight on, "not right now" has passed; "not for me" has not.
    sqlx::query("UPDATE dismissals SET at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-15 days')")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(loved(home().await.unwrap()), vec![thief, ronin]);
    // And taken back, it is as if it was never said.
    discovery.undismiss(me, heat).await.unwrap();
    assert_eq!(loved(home().await.unwrap()), vec![heat, thief, ronin]);
}

#[tokio::test]
async fn the_bandit_learns_once_per_title_from_its_strongest_outcome() {
    use sinephile_persistence::repositories::DiscoveryRepository;
    use sinephile_recommend::bandit::Context;
    use sinephile_recommend::strategy::Arm;
    let db = Db::in_memory().await.unwrap();
    let a = film(&db, "A", 1990, 1_000, &["Drama"]).await;
    let b = film(&db, "B", 1991, 1_000, &["Drama"]).await;
    let profiles = ProfileRepository::new(&db);
    let me = profiles.ensure_default().await.unwrap();
    let discovery = DiscoveryRepository::new(&db);
    discovery
        .record_shown(me, &[(a, "deep_cut"), (b, "canon")], "evening")
        .await
        .unwrap();
    // A: played, then finished, then rated: one success, not three.
    profiles
        .record_viewing(me, a, 600, false, 600)
        .await
        .unwrap();
    profiles
        .record_viewing(me, a, 6_000, true, 6_000)
        .await
        .unwrap();
    profiles.rate(me, a, Some(10)).await.unwrap();
    // B: "not for me".
    discovery
        .dismiss(me, b, "not_for_me", "evening")
        .await
        .unwrap();
    let today = sinephile_taste::model::day_of(&sinephile_taste::home::now_iso()).unwrap();
    let (beliefs, hidden) = sinephile_taste::discover::memory(&db, me, today)
        .await
        .unwrap();
    assert_eq!(beliefs.belief(Arm::DeepCut, Context::Evening), (2.0, 1.0));
    assert_eq!(beliefs.belief(Arm::Canon, Context::Evening), (1.0, 2.0));
    assert_eq!(hidden, [b].into_iter().collect());
}

#[tokio::test]
async fn surprise_me_picks_a_film_within_the_length_asked() {
    // Phase 18 (ADR-0043) and I42: with no taste yet, from what is widely loved; films only;
    // and "I have 90 minutes" leaves out what runs longer, or whose length is not known.
    use sinephile_persistence::MediaKind;
    use sinephile_taste::home::Ask;
    use sinephile_taste::surprise::surprise;
    let db = Db::in_memory().await.unwrap();
    let media = MediaRepository::new(&db);
    let mut long = NewMediaItem::film("Long", 1990);
    long.rating_votes = Some(900_000);
    long.runtime_minutes = Some(170);
    let long = media.insert(&long).await.unwrap();
    let mut short = NewMediaItem::film("Short", 1991);
    short.rating_votes = Some(800_000);
    short.runtime_minutes = Some(85);
    let short = media.insert(&short).await.unwrap();
    let mut unknown = NewMediaItem::film("Unknown length", 1992);
    unknown.rating_votes = Some(700_000);
    media.insert(&unknown).await.unwrap();
    let mut show = NewMediaItem::film("A Show", 1993);
    show.kind = Some(MediaKind::Series);
    show.rating_votes = Some(950_000);
    show.runtime_minutes = Some(30);
    media.insert(&show).await.unwrap();
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let models = sinephile_taste::model::ModelCache::default();

    let mut seen = std::collections::HashSet::new();
    for seed in 0..40 {
        let s = surprise(&db, None, me, false, None, &models, Ask::default(), seed)
            .await
            .unwrap()
            .expect("a film");
        seen.insert(s.id);
        assert!(!s.on_this_pc);
    }
    assert!(
        seen.contains(&long) && seen.contains(&short),
        "asking again gives another: {seen:?}"
    );
    assert_eq!(seen.len(), 3, "films only, never the series: {seen:?}");
}
