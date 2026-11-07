//! Phase 13 E7: bandwidth limits are actually respected, measured. A loopback seeder with
//! no limit of its own, so whatever caps the rate is the limit under test.
//!
//! `cargo test -p sinephile-downloads --test limits -- --nocapture` prints the rates.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};
use sinephile_downloads::{Manager, Rqbit};
use sinephile_library::scan::Prober;
use sinephile_persistence::repositories::{DownloadRepository, NewDownload, Probe};
use sinephile_persistence::Db;
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

fn content(len: usize) -> Vec<u8> {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

async fn seed(dir: &Path, name: &str, len: usize) -> (Engine, Vec<u8>, SocketAddr) {
    let root = dir.join("content").join(name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(format!("{name}.mkv")), content(len)).unwrap();
    let torrent = create_torrent(
        &root,
        CreateTorrentOptions {
            name: Some(name),
            trackers: vec![],
            piece_length: Some(256 * 1024),
        },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap()
    .as_bytes()
    .unwrap()
    .to_vec();
    let mut config = offline(&dir.join("seeder"));
    config.seeding = SeedingPolicy::KeepSeeding;
    let engine = Engine::start(config).await.unwrap();
    engine
        .add(
            Source::File(torrent.clone()),
            AddOptions {
                output_folder: Some(root),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let port = engine.listen_addr().unwrap().port();
    (engine, torrent, SocketAddr::from(([127, 0, 0, 1], port)))
}

fn no_probe(_: &Path) -> Option<Probe> {
    None
}

/// Download under the given limits; the mean rate from 1 s to 7 s after it started.
async fn rate(per_download_kib: Option<i64>, global_bps: Option<u32>) -> f64 {
    let work = TempDir::new().unwrap();
    let (_seeder, torrent, peer) = seed(work.path(), "Limit.Test", 48 * MIB).await;
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    engine.set_limits(global_bps, None);
    let mut rqbit = Rqbit::new(engine);
    rqbit.initial_peers = vec![peer];
    let db = Db::in_memory().await.unwrap();
    let m = Manager::new(
        db.clone(),
        vec![Arc::new(rqbit)],
        work.path().join("Downloads"),
        no_probe as Prober,
    );
    let id = DownloadRepository::new(&db)
        .add(&NewDownload {
            title: "Limit test",
            source: "torrent",
            torrent: Some(&torrent),
            backend: "librqbit",
            limit_kib: per_download_kib,
            ..Default::default()
        })
        .await
        .unwrap();
    m.tick(NOW).await.unwrap();
    let done = |db: Db| async move {
        DownloadRepository::new(&db)
            .get(id)
            .await
            .unwrap()
            .unwrap()
            .done_bytes
    };
    tokio::time::sleep(Duration::from_secs(1)).await;
    m.tick(NOW).await.unwrap();
    let (t0, b0) = (Instant::now(), done(db.clone()).await);
    for _ in 0..12 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        m.tick(NOW).await.unwrap();
    }
    let (t1, b1) = (Instant::now(), done(db.clone()).await);
    (b1 - b0) as f64 / (t1 - t0).as_secs_f64()
}

fn within(measured: f64, limit: f64, tolerance: f64) -> bool {
    (measured - limit).abs() <= limit * tolerance
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_downloads_own_limit_holds() {
    let limit = MIB as f64;
    let r = rate(Some(1024), None).await;
    println!(
        "per-download limit 1.00 MiB/s: measured {:.2} MiB/s",
        r / MIB as f64
    );
    assert!(
        within(r, limit, 0.15),
        "{:.2} MiB/s against 1 MiB/s",
        r / MIB as f64
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_global_limit_holds() {
    let limit = 2.0 * MIB as f64;
    let r = rate(None, Some(2 * MIB as u32)).await;
    println!(
        "global limit 2.00 MiB/s: measured {:.2} MiB/s",
        r / MIB as f64
    );
    assert!(
        within(r, limit, 0.15),
        "{:.2} MiB/s against 2 MiB/s",
        r / MIB as f64
    );
}

/// The control: with no limit the same swarm is far faster, so the limit is what held the
/// others. Timed over the whole download, which finishes before a sampling window opens.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_a_limit_it_runs_faster_so_the_limit_is_what_held_it() {
    let work = TempDir::new().unwrap();
    let (_seeder, torrent, peer) = seed(work.path(), "Limit.Test", 48 * MIB).await;
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    let mut rqbit = Rqbit::new(engine);
    rqbit.initial_peers = vec![peer];
    let db = Db::in_memory().await.unwrap();
    let m = Manager::new(
        db.clone(),
        vec![Arc::new(rqbit)],
        work.path().join("Downloads"),
        no_probe as Prober,
    );
    let id = DownloadRepository::new(&db)
        .add(&NewDownload {
            title: "Limit test",
            source: "torrent",
            torrent: Some(&torrent),
            backend: "librqbit",
            ..Default::default()
        })
        .await
        .unwrap();
    let started = Instant::now();
    loop {
        m.tick(NOW).await.unwrap();
        let d = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
        if d.state.is_over() {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "not done: {d:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let r = 48.0 * MIB as f64 / started.elapsed().as_secs_f64();
    println!(
        "no limit: 48 MiB in {:.2} s, {:.1} MiB/s",
        started.elapsed().as_secs_f64(),
        r / MIB as f64
    );
    assert!(
        r > 4.0 * MIB as f64,
        "the loopback swarm is too slow to show a limit: {:.2} MiB/s",
        r / MIB as f64
    );
}

/// A limit changed on a running download takes effect, and what it had is kept.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_limit_changed_while_running_takes_effect_and_keeps_the_pieces() {
    let work = TempDir::new().unwrap();
    let (_seeder, torrent, peer) = seed(work.path(), "Limit.Change", 64 * MIB).await;
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("engine")))
            .await
            .unwrap(),
    );
    let mut rqbit = Rqbit::new(engine);
    rqbit.initial_peers = vec![peer];
    let db = Db::in_memory().await.unwrap();
    let m = Manager::new(
        db.clone(),
        vec![Arc::new(rqbit)],
        work.path().join("Downloads"),
        no_probe as Prober,
    );
    let repo = DownloadRepository::new(&db);
    let id = repo
        .add(&NewDownload {
            title: "Limit change",
            source: "torrent",
            torrent: Some(&torrent),
            backend: "librqbit",
            limit_kib: Some(1024),
            ..Default::default()
        })
        .await
        .unwrap();
    m.tick(NOW).await.unwrap();
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        m.tick(NOW).await.unwrap();
    }
    let before = repo.get(id).await.unwrap().unwrap().done_bytes;
    assert!(before > 2 * MIB as i64, "it was running: {before}");

    m.set_limit(id, Some(3 * 1024)).await.unwrap();
    m.tick(NOW).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    m.tick(NOW).await.unwrap();
    let (t0, b0) = (
        Instant::now(),
        repo.get(id).await.unwrap().unwrap().done_bytes,
    );
    assert!(b0 >= before, "the pieces were kept: {before} then {b0}");
    for _ in 0..8 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        m.tick(NOW).await.unwrap();
    }
    let b1 = repo.get(id).await.unwrap().unwrap().done_bytes;
    let r = (b1 - b0) as f64 / t0.elapsed().as_secs_f64();
    println!(
        "limit 1 then 3 MiB/s: {:.2} MiB/s after the change",
        r / MIB as f64
    );
    assert!(
        within(r, 3.0 * MIB as f64, 0.2),
        "{:.2} MiB/s against 3 MiB/s",
        r / MIB as f64
    );
}
