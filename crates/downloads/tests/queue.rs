//! The queue against a real swarm on loopback: no DHT, no trackers, no network. A seeder
//! made here, an engine that downloads from it, and the manager deciding what runs.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};
use sinephile_downloads::{Manager, Rqbit};
use sinephile_library::scan::Prober;
use sinephile_persistence::repositories::{
    DownloadRepository, DownloadState, LibraryRepository, MediaRepository, NewDownload, Probe,
};
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_torrent::{AddOptions, Engine, EngineConfig, SeedingPolicy, Source};
use tempfile::TempDir;

const MIB: usize = 1024 * 1024;
const NOW: &str = "2026-11-07T21:00:00Z";

fn offline(dir: &Path) -> EngineConfig {
    let mut c = EngineConfig::new(dir);
    c.dht = false;
    c.trackers = false;
    c.lsd = false;
    c.loopback_only = true;
    c.dead_swarm_after = Duration::from_secs(5);
    c
}

/// Deterministic bytes, so a run makes the same torrent every time.
fn content(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (0..len)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 56) as u8
        })
        .collect()
}

struct Seed {
    _dir: TempDir,
    _engine: Engine,
    torrent: Vec<u8>,
    peer: SocketAddr,
}

/// One film, seeded on loopback, optionally slowly.
async fn seed(name: &str, len: usize, upload_limit: Option<u32>) -> Seed {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("content").join(name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(format!("{name}.mkv")), content(len as u64, len)).unwrap();
    let created = create_torrent(
        &root,
        CreateTorrentOptions {
            name: Some(name),
            trackers: vec![],
            piece_length: Some(256 * 1024),
        },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap();
    let torrent = created.as_bytes().unwrap().to_vec();
    let mut config = offline(&dir.path().join("seeder"));
    config.seeding = SeedingPolicy::KeepSeeding;
    config.upload_limit = upload_limit;
    let engine = Engine::start(config).await.unwrap();
    engine
        .add(
            Source::File(torrent.clone()),
            AddOptions {
                output_folder: Some(root.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let port = engine.listen_addr().unwrap().port();
    // Let the seeder check the files it was pointed at.
    tokio::time::sleep(Duration::from_millis(500)).await;
    Seed {
        _dir: dir,
        _engine: engine,
        torrent,
        peer: SocketAddr::from(([127, 0, 0, 1], port)),
    }
}

fn no_probe(_: &Path) -> Option<Probe> {
    None
}

fn manager(db: &Db, engine: Arc<Engine>, peers: Vec<SocketAddr>, dir: PathBuf) -> Manager {
    let mut rqbit = Rqbit::new(engine);
    rqbit.initial_peers = peers;
    Manager::new(db.clone(), vec![Arc::new(rqbit)], dir, no_probe as Prober)
}

async fn until_done(m: &Manager, db: &Db, id: i64, within: Duration) {
    let deadline = std::time::Instant::now() + within;
    loop {
        m.tick(NOW).await.unwrap();
        let d = DownloadRepository::new(db).get(id).await.unwrap().unwrap();
        if d.state.is_over() {
            assert_eq!(d.state, DownloadState::Done, "{d:?}");
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "not done in {within:?}: {d:?}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn film<'a>(title: &'a str, torrent: &'a [u8]) -> NewDownload<'a> {
    NewDownload {
        title,
        source: "torrent",
        torrent: Some(torrent),
        backend: "librqbit",
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_finished_download_files_itself_into_the_library_already_matched() {
    let s = seed("Heat.1995.1080p", 3 * MIB, None).await;
    let db = Db::in_memory().await.unwrap();
    let heat = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Heat", 1995))
        .await
        .unwrap();
    let work = TempDir::new().unwrap();
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    let downloads = work.path().join("Downloads");
    let m = manager(&db, engine, vec![s.peer], downloads.clone());

    let id = DownloadRepository::new(&db)
        .add(&NewDownload {
            media_item_id: Some(heat),
            ..film("Heat", &s.torrent)
        })
        .await
        .unwrap();
    until_done(&m, &db, id, Duration::from_secs(60)).await;

    let d = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    let file = d.file_path.expect("the file it made");
    assert!(Path::new(&file).starts_with(&downloads), "{file}");
    // In the library, matched to Heat and confirmed: nothing for the user to do (E6).
    let library = LibraryRepository::new(&db);
    let row = library
        .file_by_path(&file)
        .await
        .unwrap()
        .expect("a library row");
    let matched = library.match_of(row.id).await.unwrap().expect("matched");
    assert_eq!((matched.media_item_id, matched.confirmed), (heat, true));
    assert!(library
        .files_for_item(heat)
        .await
        .unwrap()
        .iter()
        .any(|f| f.path == file));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_most_important_runs_first_when_only_one_may() {
    let a = seed("Thief.1981", 2 * MIB, None).await;
    let b = seed("Collateral.2004", 2 * MIB, None).await;
    let db = Db::in_memory().await.unwrap();
    let work = TempDir::new().unwrap();
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    let mut m = manager(
        &db,
        engine,
        vec![a.peer, b.peer],
        work.path().join("Downloads"),
    );
    m.max_active = 1;
    let repo = DownloadRepository::new(&db);
    let low = repo.add(&film("Thief", &a.torrent)).await.unwrap();
    let high = repo
        .add(&NewDownload {
            priority: 5,
            ..film("Collateral", &b.torrent)
        })
        .await
        .unwrap();
    let first = m.tick(NOW).await.unwrap();
    assert_eq!(first.started, vec![high], "the important one first");
    assert_eq!(
        repo.get(low).await.unwrap().unwrap().state,
        DownloadState::Queued
    );
    until_done(&m, &db, high, Duration::from_secs(60)).await;
    until_done(&m, &db, low, Duration::from_secs(60)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_queue_survives_a_restart_and_carries_on() {
    // A slow seeder, so the restart lands mid-download.
    let s = seed("Manhunter.1986", 16 * MIB, Some(4 * MIB as u32)).await;
    let db = Db::in_memory().await.unwrap();
    let work = TempDir::new().unwrap();
    let engine_dir = work.path().join("engine");
    let downloads = work.path().join("Downloads");
    let engine = Arc::new(Engine::start(offline(&engine_dir)).await.unwrap());
    let m = manager(&db, engine.clone(), vec![s.peer], downloads.clone());
    let id = DownloadRepository::new(&db)
        .add(&film("Manhunter", &s.torrent))
        .await
        .unwrap();
    m.tick(NOW).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    m.tick(NOW).await.unwrap();
    let before = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    assert_eq!(before.state, DownloadState::Active);
    assert!(
        before.done_bytes > 0 && before.done_bytes < 16 * MIB as i64,
        "mid-download: {before:?}"
    );
    drop(m);
    Arc::try_unwrap(engine)
        .ok()
        .expect("one engine")
        .stop()
        .await;

    // The app starts again: a new engine on the same folder, a new manager, the same queue.
    let engine = Arc::new(Engine::start(offline(&engine_dir)).await.unwrap());
    // With DHT and trackers off nothing can find the seeder again, as they would in real
    // use. Stand in for them as crates/torrent's restart test does: the engine forgets the
    // torrent but keeps its files, so the manager must add it back (with the seeder as a
    // peer) from the queue's own row, keeping the pieces already on disk.
    let backend_id = before.backend_id.clone().expect("added");
    engine.remove(&backend_id, false).await.unwrap();
    let m = manager(&db, engine.clone(), vec![s.peer], downloads);
    m.tick(NOW).await.unwrap();
    let after = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    assert_eq!(
        after.state,
        DownloadState::Active,
        "it carried on: {after:?}"
    );
    assert!(engine.has(&backend_id), "added again from the queue's row");
    let kept = engine.status(&backend_id).unwrap().progress_bytes;
    assert!(
        kept >= before.done_bytes as u64,
        "the pieces on disk were kept: {kept} now, {} before",
        before.done_bytes
    );
    until_done(&m, &db, id, Duration::from_secs(60)).await;
}
