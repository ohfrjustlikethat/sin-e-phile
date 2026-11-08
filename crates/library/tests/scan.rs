//! Scanning and watching a real folder (a temporary one) against a small catalogue.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use sinephile_library::identify::Cache;
use sinephile_library::scan::{scan, Progress};
use sinephile_library::watch::{apply, watch, Applied, Change, Quieten};
use sinephile_persistence::repositories::{
    LibraryRepository, MediaRepository, Probe, SearchRepository,
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
    std::fs::write(p, vec![7u8; bytes]).unwrap();
}

static PROBES: AtomicUsize = AtomicUsize::new(0);

/// FFprobe's stand-in: counts, and says every file runs 91 minutes.
fn fake_probe(_: &Path) -> Option<Probe> {
    PROBES.fetch_add(1, Ordering::SeqCst);
    Some(Probe {
        duration_seconds: Some(91 * 60),
        ..Default::default()
    })
}

fn quiet(_: Progress) {}

#[tokio::test(flavor = "multi_thread")]
async fn a_scan_finds_identifies_and_keeps_a_renamed_file() {
    let db = Db::in_memory().await.unwrap();
    let heat = item(&db, MediaKind::Film, "Heat", 1995, 170, 700_000).await;
    item(&db, MediaKind::Film, "Heat", 1986, 101, 5_000).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    file(
        root,
        "Heat (1995)/Heat.1995.1080p.BluRay.x264-GROUP.mkv",
        4000,
    );
    file(root, "Heat (1995)/Heat.1995.sample.mkv", 100); // a sample: not the film
    file(root, "Heat (1995)/notes.txt", 10); // not a video
    let repo = LibraryRepository::new(&db);
    let folder = repo.add_folder(&root.to_string_lossy()).await.unwrap();

    let r = scan(&db, folder, root, fake_probe, &quiet).await.unwrap();
    assert_eq!((r.files, r.new, r.accepted, r.asked), (1, 1, 1, 0), "{r:?}");
    let files = repo.files_in_folder(folder).await.unwrap();
    let id = files[0].id;
    assert_eq!(
        repo.match_of(id).await.unwrap().unwrap().media_item_id,
        heat
    );

    // Renamed outside the app while it was closed: the same row, the same match.
    std::fs::rename(
        root.join("Heat (1995)/Heat.1995.1080p.BluRay.x264-GROUP.mkv"),
        root.join("Heat (1995)/Heat (1995).mkv"),
    )
    .unwrap();
    let r = scan(&db, folder, root, fake_probe, &quiet).await.unwrap();
    assert_eq!((r.files, r.moved, r.new, r.removed), (1, 1, 0, 0), "{r:?}");
    let files = repo.files_in_folder(folder).await.unwrap();
    assert_eq!(files.len(), 1, "no duplicate");
    assert_eq!(files[0].id, id);
    assert!(files[0].path.ends_with("Heat (1995).mkv"));
    assert_eq!(
        repo.match_of(id).await.unwrap().unwrap().media_item_id,
        heat
    );

    // Deleted: removed at the end of the scan.
    std::fs::remove_file(root.join("Heat (1995)/Heat (1995).mkv")).unwrap();
    let r = scan(&db, folder, root, fake_probe, &quiet).await.unwrap();
    assert_eq!((r.files, r.removed), (0, 1));
    assert!(repo.files_in_folder(folder).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn ffprobe_runs_only_when_the_name_was_not_enough() {
    let db = Db::in_memory().await.unwrap();
    let old = item(&db, MediaKind::Film, "Halloween", 1978, 91, 300_000).await;
    item(&db, MediaKind::Film, "Halloween", 2018, 106, 300_000).await;
    item(&db, MediaKind::Film, "Heat", 1995, 170, 700_000).await;
    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), "Heat.1995.mkv", 3000);
    file(dir.path(), "Halloween.mkv", 2000);
    let folder = LibraryRepository::new(&db)
        .add_folder(&dir.path().to_string_lossy())
        .await
        .unwrap();
    let before = PROBES.load(Ordering::SeqCst);
    let r = scan(&db, folder, dir.path(), fake_probe, &quiet)
        .await
        .unwrap();
    assert_eq!(
        PROBES.load(Ordering::SeqCst) - before,
        1,
        "only Halloween was probed"
    );
    assert_eq!(r.probed, 1);
    // ...and its 91 minutes settled which Halloween.
    let repo = LibraryRepository::new(&db);
    let halloween = repo
        .files_in_folder(folder)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.path.ends_with("Halloween.mkv"))
        .unwrap();
    assert_eq!(
        repo.match_of(halloween.id)
            .await
            .unwrap()
            .unwrap()
            .media_item_id,
        old
    );
    assert_eq!(halloween.duration_seconds, Some(91 * 60));
}

#[test]
fn removals_come_first_and_a_burst_is_handled_once() {
    let mut q = Quieten::default();
    let t = Instant::now();
    for i in 0..50 {
        q.push(
            Change::Upsert("D:/a/new.mkv".into()),
            t + Duration::from_millis(i),
        );
    }
    q.push(Change::Remove("D:/a/old.mkv".into()), t);
    assert!(q
        .ready(t + Duration::from_millis(500), Duration::from_secs(1))
        .is_empty());
    let ready = q.ready(t + Duration::from_secs(2), Duration::from_secs(1));
    assert_eq!(
        ready,
        vec![
            Change::Remove("D:/a/old.mkv".into()),
            Change::Upsert("D:/a/new.mkv".into())
        ]
    );
    assert!(q.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_renamed_moves_every_file_in_it() {
    let db = Db::in_memory().await.unwrap();
    item(&db, MediaKind::Film, "Heat", 1995, 170, 700_000).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    file(root, "Films/Heat.1995.mkv", 3000);
    let repo = LibraryRepository::new(&db);
    let folder = repo.add_folder(&root.to_string_lossy()).await.unwrap();
    scan(&db, folder, root, fake_probe, &quiet).await.unwrap();
    let id = repo.files_in_folder(folder).await.unwrap()[0].id;

    let applied = apply(
        &db,
        folder,
        root,
        &Change::Rename {
            from: root.join("Films"),
            to: root.join("Crime"),
        },
        fake_probe,
        &Cache::default(),
    )
    .await
    .unwrap();
    assert_eq!(applied, Applied::Moved);
    let f = repo.file(id).await.unwrap().unwrap();
    assert!(
        Path::new(&f.path).starts_with(root.join("Crime")),
        "{}",
        f.path
    );
}

/// The real watcher: a file renamed while watched stays one row, with its match.
#[tokio::test(flavor = "multi_thread")]
async fn the_watcher_sees_a_rename_and_keeps_one_row() {
    let db = Db::in_memory().await.unwrap();
    let heat = item(&db, MediaKind::Film, "Heat", 1995, 170, 700_000).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    file(&root, "Heat.1995.1080p.mkv", 5000);
    let repo = LibraryRepository::new(&db);
    let folder = repo.add_folder(&root.to_string_lossy()).await.unwrap();
    scan(&db, folder, &root, fake_probe, &quiet).await.unwrap();
    let id = repo.files_in_folder(folder).await.unwrap()[0].id;

    let (tx, rx) = mpsc::channel();
    let _watcher = watch(&root, move |c| {
        let _ = tx.send(c);
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    std::fs::rename(
        root.join("Heat.1995.1080p.mkv"),
        root.join("Heat (1995).mkv"),
    )
    .unwrap();

    // Collect for a moment, quieten, apply.
    let mut q = Quieten::default();
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if let Ok(c) = rx.recv_timeout(Duration::from_millis(100)) {
            q.push(c, Instant::now());
        }
    }
    let cache = Cache::default();
    let changes = q.ready(
        Instant::now() + Duration::from_secs(10),
        Duration::from_secs(1),
    );
    assert!(!changes.is_empty(), "the watcher saw something");
    for c in &changes {
        apply(&db, folder, &root, c, fake_probe, &cache)
            .await
            .unwrap();
    }

    let files: Vec<_> = repo
        .files_in_folder(folder)
        .await
        .unwrap()
        .into_iter()
        .filter(|f| f.missing_since.is_none())
        .collect();
    assert_eq!(files.len(), 1, "one row, not two: {changes:?}");
    assert_eq!(files[0].id, id, "the same row");
    assert!(files[0].path.ends_with("Heat (1995).mkv"));
    assert_eq!(
        repo.match_of(id).await.unwrap().unwrap().media_item_id,
        heat
    );
}

/// Found by `eval scan`: a film whose title starts with "$" or "." was skipped as if it
/// were a system folder.
#[test]
fn a_shows_extras_are_left_out_and_its_episodes_and_film_kept() {
    // The author's Twin Peaks layout (Phase 16), in miniature.
    use sinephile_library::scan::walk;
    let dir = tempfile::tempdir().unwrap();
    let show = "Twin Peaks (1990) Season 1-3 S01-S03 (1080p BluRay x265 NORDALE)";
    file(
        dir.path(),
        &format!("{show}/Season 1/Twin Peaks (1990) - S01E03 - Zen.mkv"),
        1000,
    );
    file(
        dir.path(),
        &format!("{show}/Featurettes/Season 2/Cherry Pie.mkv"),
        1000,
    );
    file(
        dir.path(),
        &format!("{show}/Featurettes/Season 1/Behind the Scenes/Tim Hunter.mkv"),
        1000,
    );
    file(
        dir.path(),
        &format!("{show}/Twin Peaks - Fire Walk with Me (1992)/Twin Peaks - Fire Walk with Me (1992).mkv"),
        1000,
    );
    file(
        dir.path(),
        &format!("{show}/Twin Peaks - Fire Walk with Me (1992)/Featurettes/Trailer 1.mkv"),
        1000,
    );
    let mut names: Vec<String> = walk(dir.path(), &quiet)
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "Twin Peaks (1990) - S01E03 - Zen.mkv",
            "Twin Peaks - Fire Walk with Me (1992).mkv"
        ]
    );
}

#[test]
fn titles_that_look_like_system_names_are_still_found() {
    use sinephile_library::scan::walk;
    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), "$9.99 (2008)/$9.99.2008.mkv", 100_000_000);
    file(dir.path(), ".45 (2006)/.45.2006.mkv", 1000);
    file(dir.path(), "$RECYCLE.BIN/deleted.mkv", 1000);
    let found = walk(dir.path(), &quiet);
    let names: Vec<String> = found
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"$9.99.2008.mkv".to_string()), "{names:?}");
    assert!(names.contains(&".45.2006.mkv".to_string()), "{names:?}");
    assert!(
        !names.contains(&"deleted.mkv".to_string()),
        "the recycle bin is not a library"
    );
}
