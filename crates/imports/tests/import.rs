//! A Letterboxd export end to end: read from its `.zip`, matched to a catalogue, recorded
//! for one profile, and imported a second time without adding anything (Phase 14).

use std::io::Write;

use sinephile_imports::{import, read, Format};
use sinephile_persistence::model::{IdSource, NewMediaItem};
use sinephile_persistence::repositories::{MediaRepository, NewProfile, ProfileRepository};
use sinephile_persistence::Db;

fn zip(dir: &std::path::Path, files: &[(&str, &str)]) -> std::path::PathBuf {
    let path = dir.join("letterboxd-someone-2026-11-07.zip");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, text) in files {
        z.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(text.as_bytes()).unwrap();
    }
    z.finish().unwrap();
    path
}

const WATCHED: &str = "Date,Name,Year,Letterboxd URI\n\
2024-01-02,Heat,1995,\n\
2024-01-03,\"Crouching Tiger, Hidden Dragon\",2000,\n\
2024-01-04,Thief,1980,\n\
2024-01-05,Solaris,1972,\n\
2024-01-06,A Film Nobody Has Heard Of,2011,\n";

const DIARY: &str = "Date,Name,Year,Letterboxd URI,Rating,Rewatch,Tags,Watched Date\n\
2024-01-02,Heat,1995,,4.5,,,2019-06-01\n\
2024-01-10,Heat,1995,,5,Yes,,2024-01-09\n";

const RATINGS: &str = "Date,Name,Year,Letterboxd URI,Rating\n\
2024-01-02,Heat,1995,,5\n\
2024-01-03,\"Crouching Tiger, Hidden Dragon\",2000,,3.5\n";

const WATCHLIST: &str = "Date,Name,Year,Letterboxd URI\n2024-02-01,Manhunter,1986,\n";

/// Something deleted from the account: not the person's history.
const DELETED: &str = "Date,Name,Year,Letterboxd URI\n2020-01-01,Collateral,2004,\n";

async fn catalogue(db: &Db) -> Vec<i64> {
    let media = MediaRepository::new(db);
    let mut ids = Vec::new();
    for (title, year, tt) in [
        ("Heat", 1995, "tt0113277"),
        ("Crouching Tiger, Hidden Dragon", 2000, "tt0190332"),
        ("Thief", 1981, "tt0083190"),
        ("Manhunter", 1986, "tt0091474"),
        ("Collateral", 2004, "tt0369339"),
        ("Solaris", 1972, "tt0069293"),
        ("Solaris", 1972, "tt0000001"),
    ] {
        let id = media
            .insert(&NewMediaItem {
                runtime_minutes: Some(120),
                ..NewMediaItem::film(title, year)
            })
            .await
            .unwrap();
        media
            .add_external_id(id, IdSource::Imdb, tt, 1.0)
            .await
            .unwrap();
        ids.push(id);
    }
    ids
}

#[tokio::test]
async fn a_letterboxd_export_is_recorded_and_a_second_import_adds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = zip(
        dir.path(),
        &[
            ("watched.csv", WATCHED),
            ("diary.csv", DIARY),
            ("ratings.csv", RATINGS),
            ("watchlist.csv", WATCHLIST),
            ("deleted/watched.csv", DELETED),
        ],
    );
    let (format, entries) = read(&path).unwrap();
    assert_eq!(format, Format::Letterboxd);
    assert_eq!(
        entries.len(),
        6,
        "five watched (one twice), one on the watchlist, nothing deleted"
    );

    let db = Db::in_memory().await.unwrap();
    let ids = catalogue(&db).await;
    let (heat, tiger, thief, manhunter, collateral) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let profiles = ProfileRepository::new(&db);
    let me = profiles
        .create_with(&NewProfile {
            name: "Me",
            ..Default::default()
        })
        .await
        .unwrap();

    let s = import(&db, me, &entries, "2026-11-07 12:00:00")
        .await
        .unwrap();
    assert_eq!(s.entries, 6);
    assert_eq!(s.matched, 4, "{:?}", s.unmatched);
    // Heat's two diary watches (2019, and the rewatch) rather than watched.csv's one;
    // Crouching Tiger and Thief one each.
    assert_eq!(s.watches, 4);
    assert_eq!((s.ratings, s.watchlist), (2, 1));

    // Thief is 1980 on Letterboxd and 1981 here: one year either side, and only one.
    assert!(profiles.finished(me, thief).await.unwrap());
    // ratings.csv's rating beats the diary's; 3.5 rounds up to 4.
    assert_eq!(profiles.rating(me, heat).await.unwrap(), Some(5));
    assert_eq!(profiles.rating(me, tiger).await.unwrap(), Some(4));
    assert_eq!(profiles.watchlist(me).await.unwrap(), vec![manhunter]);
    assert!(
        !profiles.finished(me, collateral).await.unwrap(),
        "deleted/ is not history"
    );
    let dates: Vec<String> = sqlx::query_scalar(
        "SELECT started_at FROM watch_events WHERE profile_id = ? AND media_item_id = ? ORDER BY started_at",
    )
    .bind(me)
    .bind(heat)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(dates, ["2019-06-01 00:00:00", "2024-01-09 00:00:00"]);

    // Said in words, never guessed.
    let why: Vec<(&str, &str)> = s
        .unmatched
        .iter()
        .map(|u| (u.title.as_str(), u.why.as_str()))
        .collect();
    assert!(why.contains(&("Solaris", "more than one title by that name from 1972")));
    assert!(why.contains(&(
        "A Film Nobody Has Heard Of",
        "no title by that name in the catalogue"
    )));

    // Again: nothing new.
    let again = import(&db, me, &entries, "2026-11-08 12:00:00")
        .await
        .unwrap();
    assert_eq!(again.watches, 0, "the same watches are not recorded twice");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_events WHERE profile_id = ?")
        .bind(me)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 4);
}

#[tokio::test]
async fn one_of_two_same_named_titles_is_taken_only_when_it_is_plainly_the_one() {
    let db = Db::in_memory().await.unwrap();
    let media = MediaRepository::new(&db);
    let famous = media
        .insert(&NewMediaItem {
            rating_votes: Some(100_000),
            ..NewMediaItem::film("Solaris", 1972)
        })
        .await
        .unwrap();
    media
        .insert(&NewMediaItem {
            rating_votes: Some(40),
            ..NewMediaItem::film("Solaris", 1972)
        })
        .await
        .unwrap();
    let me = ProfileRepository::new(&db).ensure_default().await.unwrap();
    let entry = sinephile_imports::Entry {
        title: "Solaris".into(),
        year: Some(1972),
        watches: vec![None],
        ..Default::default()
    };
    let s = import(&db, me, std::slice::from_ref(&entry), "2026-11-07 12:00:00")
        .await
        .unwrap();
    assert_eq!(s.matched, 1);
    assert!(ProfileRepository::new(&db)
        .finished(me, famous)
        .await
        .unwrap());

    // Close in votes: asked about, not guessed.
    media
        .insert(&NewMediaItem {
            rating_votes: Some(30_000),
            ..NewMediaItem::film("Stalker", 1979)
        })
        .await
        .unwrap();
    media
        .insert(&NewMediaItem {
            rating_votes: Some(9_000),
            ..NewMediaItem::film("Stalker", 1979)
        })
        .await
        .unwrap();
    let s = import(
        &db,
        me,
        &[sinephile_imports::Entry {
            title: "Stalker".into(),
            year: Some(1979),
            ..Default::default()
        }],
        "2026-11-07 12:00:00",
    )
    .await
    .unwrap();
    assert_eq!(s.matched, 0);
    assert_eq!(
        s.unmatched[0].why,
        "more than one title by that name from 1979"
    );
}
