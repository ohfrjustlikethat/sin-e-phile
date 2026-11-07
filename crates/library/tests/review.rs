//! The review queue against a small catalogue: one answer settles every file named the
//! same way, is remembered for files added later, and "not a film" stays answered.

use std::path::Path;

use sinephile_library::review::{not_a_title, queue, resolve};
use sinephile_library::scan::{scan, Progress};
use sinephile_persistence::repositories::{
    EpisodeRepository, LibraryRepository, MediaRepository, Probe, SearchRepository,
};
use sinephile_persistence::{Db, MediaKind, NewMediaItem};

fn normalised(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn item(db: &Db, kind: MediaKind, title: &str, year: i64, runtime: i64, votes: i64) -> i64 {
    let id = MediaRepository::new(db)
        .insert(&NewMediaItem {
            kind: Some(kind),
            primary_title: title.into(),
            release_year: Some(year),
            runtime_minutes: Some(runtime),
            rating_votes: Some(votes),
            ..Default::default()
        })
        .await
        .unwrap();
    sqlx::query("UPDATE titles SET normalised = ? WHERE media_item_id = ?")
        .bind(normalised(title))
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    SearchRepository::new(db)
        .index_item(id, title, "", "", "")
        .await
        .unwrap();
    id
}

fn file(root: &Path, rel: &str, bytes: usize) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, vec![1u8; bytes]).unwrap();
}

fn no_probe(_: &Path) -> Option<Probe> {
    None
}

fn quiet(_: Progress) {}

/// Two series with the same name and no year in the files: a question, then one answer.
#[tokio::test(flavor = "multi_thread")]
async fn one_answer_settles_the_whole_series_and_is_remembered() {
    let db = Db::in_memory().await.unwrap();
    let old = item(&db, MediaKind::Series, "Monster", 2004, 24, 50_000).await;
    item(&db, MediaKind::Series, "Monster", 2004, 45, 40_000).await;
    let episodes = EpisodeRepository::new(&db);
    episodes.create_series(old, false).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    for n in 1..=5 {
        file(
            dir.path(),
            &format!("Monster/Monster - {n:02} [DVD].mkv"),
            1000 + n,
        );
    }
    let repo = LibraryRepository::new(&db);
    let folder = repo
        .add_folder(&dir.path().to_string_lossy())
        .await
        .unwrap();
    let r = scan(&db, folder, dir.path(), no_probe, &quiet)
        .await
        .unwrap();
    assert_eq!(
        r.asked, 5,
        "two series called Monster from 2004: ask ({r:?})"
    );

    let waiting = queue(&db, 20).await.unwrap();
    assert_eq!(waiting.len(), 5);
    assert_eq!(
        waiting[0].same_name, 4,
        "the other four are named the same way"
    );
    assert!(waiting[0].options.len() >= 2, "both are offered");

    let done = resolve(&db, waiting[0].file.id, old).await.unwrap();
    assert_eq!(done.settled, 4, "one answer, five files");
    assert!(queue(&db, 20).await.unwrap().is_empty());
    let m = repo.match_of(waiting[0].file.id).await.unwrap().unwrap();
    assert!(m.confirmed, "the answered file is confirmed");

    // Remembered: an episode added later is not asked about.
    file(dir.path(), "Monster/Monster - 06 [DVD].mkv", 2000);
    let r = scan(&db, folder, dir.path(), no_probe, &quiet)
        .await
        .unwrap();
    assert_eq!((r.new, r.accepted, r.asked), (1, 1, 0), "{r:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn not_a_film_stays_answered_across_rescans() {
    let db = Db::in_memory().await.unwrap();
    item(&db, MediaKind::Film, "Halloween", 1978, 91, 300_000).await;
    item(&db, MediaKind::Film, "Halloween", 2018, 106, 300_000).await;
    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), "Halloween.mkv", 3000);
    let repo = LibraryRepository::new(&db);
    let folder = repo
        .add_folder(&dir.path().to_string_lossy())
        .await
        .unwrap();
    scan(&db, folder, dir.path(), no_probe, &quiet)
        .await
        .unwrap();
    let waiting = queue(&db, 20).await.unwrap();
    assert_eq!(waiting.len(), 1);

    not_a_title(&db, waiting[0].file.id).await.unwrap();
    assert!(queue(&db, 20).await.unwrap().is_empty());
    scan(&db, folder, dir.path(), no_probe, &quiet)
        .await
        .unwrap();
    assert!(
        queue(&db, 20).await.unwrap().is_empty(),
        "a rescan does not bring it back"
    );
}
