//! Continue watching, by title (Phase 16, the author's round 8): a show is one card titled
//! with the show, saying which episode, never its lone episode.

use sinephile_persistence::model::{MediaKind, NewMediaItem};
use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{EpisodeRepository, MediaRepository, ProfileRepository};
use sinephile_persistence::Db;
use sinephile_taste::continuing::{continuing, for_series};
use sinephile_taste::home::rails;

async fn item(db: &Db, title: &str, kind: MediaKind) -> i64 {
    MediaRepository::new(db)
        .insert(&NewMediaItem {
            kind: Some(kind),
            ..NewMediaItem::film(title, 1990)
        })
        .await
        .unwrap()
}

/// A show with one season of `titles`, returning (show, episodes).
async fn show(db: &Db, name: &str, titles: &[&str]) -> (i64, Vec<i64>) {
    let show = item(db, name, MediaKind::Series).await;
    let episodes = EpisodeRepository::new(db);
    episodes.create_series(show, false).await.unwrap();
    let season = episodes.create_season(show, 1, None).await.unwrap();
    let mut ids = Vec::new();
    for (n, t) in titles.iter().enumerate() {
        let id = item(db, t, MediaKind::Episode).await;
        episodes
            .create_episode(id, show, Some(season), Some(1), Some(n as i64 + 1), None)
            .await
            .unwrap();
        ids.push(id);
    }
    (show, ids)
}

/// A position saved `minutes_ago`, so the order is the test's, not the clock's.
async fn watched(db: &Db, profile: i64, id: i64, at: i64, of: i64, minutes_ago: i64) {
    ProfileRepository::new(db)
        .save_position(
            profile,
            &PlaybackPosition {
                media_item_id: id,
                position_seconds: at,
                duration_seconds: Some(of),
                audio_language: None,
                subtitle_language: None,
            },
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE playback_positions SET updated_at = datetime('now', ?)
         WHERE profile_id = ? AND media_item_id = ?",
    )
    .bind(format!("-{minutes_ago} minutes"))
    .bind(profile)
    .bind(id)
    .execute(db.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn a_show_is_one_card_saying_which_episode_and_a_finished_one_leads_to_the_next() {
    let db = Db::in_memory().await.unwrap();
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    // The author's own history, read from their install: the pilot finished, episode 2
    // started after it, and a film half-watched before both.
    let (peaks, eps) = show(
        &db,
        "Twin Peaks",
        &[
            "Episode #1.1",
            "Traces to Nowhere",
            "Zen, or the Skill to Catch a Killer",
        ],
    )
    .await;
    let film = item(&db, "Project Hail Mary", MediaKind::Film).await;
    // A show whose last episode was finished, with one after it; and one finished outright.
    let (monster, m) = show(
        &db,
        "Monster",
        &["Herr Doktor Tenma", "Night of the Execution"],
    )
    .await;
    let (done, d) = show(&db, "A Finished Show", &["The Only Episode"]).await;

    watched(&db, me, film, 896, 9_391, 300).await;
    watched(&db, me, d[0], 2_000, 2_000, 250).await;
    watched(&db, me, m[0], 1_430, 1_435, 200).await;
    watched(&db, me, eps[0], 5_625, 5_643, 120).await;
    watched(&db, me, eps[1], 76, 2_788, 60).await;

    let c = continuing(&db, me, 20).await.unwrap();
    let cards: Vec<i64> = c.iter().map(|c| c.card).collect();
    assert_eq!(
        cards,
        vec![peaks, monster, film],
        "newest first; the finished show is gone"
    );

    let tp = &c[0];
    assert!(tp.is_show);
    assert_eq!(tp.plays, eps[1], "episode 2, not the finished pilot");
    assert_eq!(tp.note.as_deref(), Some("S1 E2 · Traces to Nowhere"));
    assert!((tp.progress.unwrap() - 76.0 / 2_788.0).abs() < 1e-9);
    assert_eq!(tp.resume_at, Some(76));

    let mo = &c[1];
    assert_eq!(mo.plays, m[1], "finished episode 1: episode 2 next");
    assert_eq!(
        mo.note.as_deref(),
        Some("Next: S1 E2 · Night of the Execution")
    );
    assert_eq!((mo.progress, mo.resume_at), (None, None));

    assert!(!c[2].is_show);
    assert_eq!((c[2].plays, c[2].note.as_deref()), (film, None));

    // The show's page resumes the same episode.
    let up = for_series(&db, me, peaks).await.unwrap().unwrap();
    assert_eq!((up.plays, up.resume_at), (eps[1], Some(76)));
    assert!(for_series(&db, me, done).await.unwrap().is_none());

    // Home's rail is the same: the show, with its line and its bar.
    let r = rails(&db, None, me, false, None, &Default::default())
        .await
        .unwrap();
    let cont = r.iter().find(|r| r.key == "continue").unwrap();
    assert_eq!(cont.items, vec![peaks, monster, film]);
    assert_eq!(
        cont.notes.get(&peaks).map(String::as_str),
        Some("S1 E2 · Traces to Nowhere")
    );
    assert!(cont.progress.contains_key(&peaks) && !cont.progress.contains_key(&monster));
    assert!(!cont.items.contains(&eps[1]), "never the lone episode");
}

#[tokio::test]
async fn an_older_half_watched_episode_is_behind_the_latest() {
    let db = Db::in_memory().await.unwrap();
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let (s, e) = show(&db, "A Show", &["One", "Two", "Three"]).await;
    watched(&db, me, e[0], 600, 2_400, 90).await;
    watched(&db, me, e[2], 300, 2_400, 10).await;
    let c = continuing(&db, me, 20).await.unwrap();
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!((c[0].card, c[0].plays), (s, e[2]));
}

#[tokio::test]
async fn a_removed_title_leaves_continue_watching_until_it_is_played_again() {
    // ADR-0056, the author: remove anything from Continue watching; its place is kept, and
    // it comes back once played again (for a show, any of its episodes).
    let db = Db::in_memory().await.unwrap();
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let film = item(&db, "Heat", MediaKind::Film).await;
    let (series, eps) = show(&db, "Twin Peaks", &["Pilot", "Traces to Nowhere"]).await;
    watched(&db, me, film, 1_200, 10_200, 30).await;
    watched(&db, me, eps[0], 600, 2_800, 20).await;
    let cards = |db: Db| async move {
        continuing(&db, me, 20)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.card)
            .collect::<Vec<_>>()
    };
    assert_eq!(cards(db.clone()).await, vec![series, film]);

    let profiles = ProfileRepository::new(&db);
    profiles.hide_from_continue(me, film).await.unwrap();
    profiles.hide_from_continue(me, series).await.unwrap();
    assert!(cards(db.clone()).await.is_empty(), "both removed");
    let place = profiles
        .position(me, film)
        .await
        .unwrap()
        .expect("the place is kept");
    assert_eq!(place.position_seconds, 1_200);
    assert!(
        for_series(&db, me, series).await.unwrap().is_some(),
        "the show's page still resumes where it was"
    );

    // Played again: the show by its next episode, the film by itself.
    sqlx::query("UPDATE continue_hidden SET hidden_at = datetime('now', '-5 minutes')")
        .execute(db.pool())
        .await
        .unwrap();
    watched(&db, me, eps[1], 60, 2_800, 1).await;
    assert_eq!(cards(db.clone()).await, vec![series], "the show is back");
    watched(&db, me, film, 1_500, 10_200, 0).await;
    assert_eq!(cards(db.clone()).await, vec![film, series], "and the film");
}
