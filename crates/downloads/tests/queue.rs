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

/// Phase 13 E3: qBittorrent configured but not running. The download meant for it is
/// fetched by librqbit instead of failing, and the row says which backend moved it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_configured_qbittorrent_that_is_not_running_falls_back_to_librqbit() {
    use sinephile_downloads::qbittorrent::Qbittorrent;
    use sinephile_downloads::Backend;
    use sinephile_metadata_api::transport::FakeTransport;

    let s = seed("Thief.1981.Fallback", 2 * MIB, None).await;
    let db = Db::in_memory().await.unwrap();
    let work = TempDir::new().unwrap();
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    let mut rqbit = Rqbit::new(engine);
    rqbit.initial_peers = vec![s.peer];
    // Nothing answers at qBittorrent's address: an empty stand-in is an unreachable server.
    let qbit = Qbittorrent::new(
        Arc::new(FakeTransport::new()),
        "http://qbit.test:8080",
        None,
        None,
    );
    let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(qbit), Arc::new(rqbit)];
    let m = Manager::new(
        db.clone(),
        backends,
        work.path().join("Downloads"),
        no_probe as Prober,
    );

    let source = format!("infohash:{}", "0".repeat(40));
    let id = DownloadRepository::new(&db)
        .add(&NewDownload {
            title: "Thief",
            source: &source,
            torrent: Some(&s.torrent),
            backend: "qbittorrent",
            ..Default::default()
        })
        .await
        .unwrap();
    let first = m.tick(NOW).await.unwrap();
    assert_eq!(first.started, vec![id], "started, not failed: {first:?}");
    let d = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    assert_eq!(d.backend, "librqbit", "librqbit moved the bytes");
    until_done(&m, &db, id, Duration::from_secs(60)).await;
}

/// The film page starts a torrent to stream before anyone asks to download it (D61). A
/// download of the same torrent moves it into the downloads folder with what it has.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_torrent_already_streaming_becomes_the_download_and_keeps_its_pieces() {
    let s = seed("Heat.1995.Stream", 16 * MIB, Some(4 * MIB as u32)).await;
    let db = Db::in_memory().await.unwrap();
    let work = TempDir::new().unwrap();
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    // Streaming: added the way prepare_play adds it, into the engine's own folder.
    let streaming = engine
        .add(
            Source::File(s.torrent.clone()),
            AddOptions {
                initial_peers: vec![s.peer],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let had = engine.status(&streaming).unwrap().progress_bytes;
    assert!(had > 0 && had < 16 * MIB as u64, "mid-stream: {had}");

    let downloads = work.path().join("Downloads");
    let m = manager(&db, engine.clone(), vec![s.peer], downloads.clone());
    let id = DownloadRepository::new(&db)
        .add(&film("Heat", &s.torrent))
        .await
        .unwrap();
    m.tick(NOW).await.unwrap();
    let d = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    assert_eq!(
        d.backend_id.as_deref(),
        Some(streaming.as_str()),
        "the same torrent"
    );
    let kept = engine.status(&streaming).unwrap().progress_bytes;
    assert!(
        kept >= had,
        "the pieces came along: {kept} now, {had} before"
    );
    until_done(&m, &db, id, Duration::from_secs(60)).await;
    let file = DownloadRepository::new(&db)
        .get(id)
        .await
        .unwrap()
        .unwrap()
        .file_path
        .unwrap();
    assert!(
        Path::new(&file).starts_with(downloads.join("Heat")),
        "{file}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_running_download_moved_to_tonight_waits_then_carries_on() {
    // A slow seeder, so it is mid-download when it is moved.
    let s = seed("Ronin.1998", 16 * MIB, Some(4 * MIB as u32)).await;
    let db = Db::in_memory().await.unwrap();
    let work = TempDir::new().unwrap();
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    let m = manager(&db, engine, vec![s.peer], work.path().join("Downloads"));
    let repo = DownloadRepository::new(&db);
    let id = repo.add(&film("Ronin", &s.torrent)).await.unwrap();
    m.tick(NOW).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    m.tick(NOW).await.unwrap();

    m.schedule(id, Some("2026-11-08T01:00:00Z")).await.unwrap();
    let held = repo.get(id).await.unwrap().unwrap();
    assert_eq!(held.state, DownloadState::Queued);
    assert!(
        held.done_bytes > 0 && held.done_bytes < 16 * MIB as i64,
        "{held:?}"
    );
    // Before one in the morning nothing runs, and nothing arrives.
    for _ in 0..5 {
        let t = m.tick(NOW).await.unwrap();
        assert!(t.started.is_empty(), "{t:?}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert_eq!(
        repo.get(id).await.unwrap().unwrap().state,
        DownloadState::Queued
    );
    assert!(m.live().is_empty(), "nothing is shown as running");
    // At one, it carries on from where it was.
    let t = m.tick("2026-11-08T01:00:00Z").await.unwrap();
    assert_eq!(t.started, vec![id]);
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        m.tick("2026-11-08T01:00:01Z").await.unwrap();
        let d = repo.get(id).await.unwrap().unwrap();
        if d.state == DownloadState::Done {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{d:?}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
