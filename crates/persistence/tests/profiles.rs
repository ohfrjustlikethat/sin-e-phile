//! Phase 14 E5: profiles leak no state. Everything personal one profile does is invisible
//! to another, and deleting a profile takes all of it and nothing else.

use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{
    CredentialRepository, MediaRepository, NewProfile, ProfileRepository,
};
use sinephile_persistence::{Db, NewMediaItem};

/// What one profile can see of its own.
#[derive(Debug, PartialEq)]
struct Seen {
    watchlist: Vec<i64>,
    rating: Option<i64>,
    position: Option<i64>,
    continue_watching: usize,
    finished: bool,
    setting: Option<String>,
    key: Option<String>,
    pin_opens_with_wrong_code: bool,
}

async fn seen(db: &Db, profile: i64, film: i64) -> Seen {
    let p = ProfileRepository::new(db);
    let c = CredentialRepository::new(db);
    Seen {
        watchlist: p.watchlist(profile).await.unwrap(),
        rating: p.rating(profile, film).await.unwrap(),
        position: p
            .position(profile, film)
            .await
            .unwrap()
            .map(|x| x.position_seconds),
        continue_watching: p.continue_watching(profile, 10).await.unwrap().len(),
        finished: p.finished(profile, film).await.unwrap(),
        setting: c.get(profile, "playback.audio").await.unwrap(),
        key: c.secret(profile, "qbittorrent.password").await.unwrap(),
        pin_opens_with_wrong_code: c.verify_pin(profile, "0000").await.unwrap(),
    }
}

#[tokio::test]
async fn nothing_one_profile_does_is_visible_to_another() {
    let db = Db::in_memory().await.unwrap();
    let film = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Tokyo Story", 1953))
        .await
        .unwrap();
    let other = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Late Spring", 1949))
        .await
        .unwrap();
    let profiles = ProfileRepository::new(&db);
    let creds = CredentialRepository::new(&db);
    let a = profiles.ensure_default().await.unwrap();
    let b = profiles
        .create_with(&NewProfile {
            name: "Mina",
            ..Default::default()
        })
        .await
        .unwrap();
    let before_b = seen(&db, b, film).await;

    // Everything personal, done as A.
    profiles.add_to_watchlist(a, film, None).await.unwrap();
    profiles.rate(a, film, Some(5)).await.unwrap();
    profiles
        .save_position(
            a,
            &PlaybackPosition {
                media_item_id: film,
                position_seconds: 1_200,
                duration_seconds: Some(8_100),
                audio_language: Some("ja".into()),
                subtitle_language: Some("en".into()),
            },
        )
        .await
        .unwrap();
    profiles.record_watch(a, other, 6_000, true).await.unwrap();
    profiles
        .save_position(
            a,
            &PlaybackPosition {
                media_item_id: film,
                position_seconds: 8_000,
                duration_seconds: Some(8_100),
                audio_language: None,
                subtitle_language: None,
            },
        )
        .await
        .unwrap();
    creds.set(a, "playback.audio", "ja").await.unwrap();
    creds
        .set_secret(a, "qbittorrent.password", "a's own")
        .await
        .unwrap();
    creds.set_pin(a, "1357").await.unwrap();

    let after_a = seen(&db, a, film).await;
    assert_eq!(after_a.watchlist, vec![film]);
    assert_eq!(after_a.rating, Some(5));
    assert!(after_a.finished && after_a.setting.as_deref() == Some("ja"));
    assert!(!after_a.pin_opens_with_wrong_code);

    // B saw none of it.
    let after_b = seen(&db, b, film).await;
    assert_eq!(after_b, before_b, "B is exactly as it was");
    assert_eq!(
        after_b,
        Seen {
            watchlist: vec![],
            rating: None,
            position: None,
            continue_watching: 0,
            finished: false,
            setting: None,
            key: None,
            pin_opens_with_wrong_code: true,
        }
    );
    assert!(
        !profiles.finished(b, other).await.unwrap(),
        "A's watch is A's"
    );
}

#[tokio::test]
async fn deleting_a_profile_takes_all_of_its_own_and_nothing_else() {
    let db = Db::in_memory().await.unwrap();
    let film = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Floating Weeds", 1959))
        .await
        .unwrap();
    let profiles = ProfileRepository::new(&db);
    let creds = CredentialRepository::new(&db);
    let a = profiles.ensure_default().await.unwrap();
    let b = profiles
        .create_with(&NewProfile {
            name: "Guest",
            ..Default::default()
        })
        .await
        .unwrap();
    for p in [a, b] {
        profiles.add_to_watchlist(p, film, None).await.unwrap();
        profiles.rate(p, film, Some(4)).await.unwrap();
        profiles.record_watch(p, film, 100, false).await.unwrap();
        creds.set(p, "playback.audio", "en").await.unwrap();
        creds.set_pin(p, "2468").await.unwrap();
    }
    let a_before = seen(&db, a, film).await;
    assert!(profiles.delete(b).await.unwrap());

    let count = |sql: &'static str| {
        let db = db.clone();
        async move {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(b)
                .fetch_one(db.pool())
                .await
                .unwrap()
        }
    };
    for sql in [
        "SELECT COUNT(*) FROM watchlist_items WHERE profile_id = ?",
        "SELECT COUNT(*) FROM ratings WHERE profile_id = ?",
        "SELECT COUNT(*) FROM watch_events WHERE profile_id = ?",
        "SELECT COUNT(*) FROM profile_settings WHERE profile_id = ?",
        "SELECT COUNT(*) FROM playback_positions WHERE profile_id = ?",
    ] {
        assert_eq!(count(sql).await, 0, "{sql}");
    }
    assert_eq!(seen(&db, a, film).await, a_before, "A untouched");
}
