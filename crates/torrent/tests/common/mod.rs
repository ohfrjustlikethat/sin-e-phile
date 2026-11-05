//! A two-node swarm on loopback, built per test (Phase 7, subtask 7.2).
//!
//! The content is generated, the torrent is created here, one engine seeds it and
//! another downloads it from `127.0.0.1` — no DHT, no trackers, no URL, no network. It is
//! the only way to test streaming in CI, and the only way to make a swarm behave badly on
//! purpose (a slow seeder, a dead one).

#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use sinephile_torrent::{AddOptions, Engine, EngineConfig, SeedingPolicy, Source};
use tempfile::TempDir;

pub const KIB: usize = 1024;
pub const MIB: usize = 1024 * 1024;

pub struct Swarm {
    pub seeder: Engine,
    pub seed_dir: TempDir,
    pub torrent: Vec<u8>,
    /// (path inside the torrent, contents)
    pub files: Vec<(String, Vec<u8>)>,
    pub peer: SocketAddr,
    pub id: String,
}

/// An engine with everything that reaches outside the machine turned off.
pub fn offline_config(dir: &Path) -> EngineConfig {
    let mut c = EngineConfig::new(dir);
    c.dht = false;
    c.trackers = false;
    c.lsd = false;
    c.dead_swarm_after = Duration::from_secs(5);
    c
}

fn content(seed: u64, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    StdRng::seed_from_u64(seed).fill_bytes(&mut v);
    v
}

/// Build the swarm: `files` are (relative path, size). The seeder optionally uploads at
/// no more than `upload_limit` bytes per second, to make a slow swarm on purpose.
pub async fn swarm(files: &[(&str, usize)], upload_limit: Option<u32>) -> Swarm {
    let seed_dir = TempDir::new().unwrap();
    let root = seed_dir.path().join("content").join("Test.Film.1922");
    let mut made = Vec::new();
    for (i, (rel, len)) in files.iter().enumerate() {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let bytes = content(i as u64 + 1, *len);
        std::fs::write(&p, &bytes).unwrap();
        made.push((rel.to_string(), bytes));
    }
    let created = create_torrent(
        &root,
        CreateTorrentOptions {
            name: Some("Test.Film.1922"),
            trackers: vec![],
            piece_length: Some(256 * KIB as u32),
        },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap();
    let torrent = created.as_bytes().unwrap().to_vec();

    let mut config = offline_config(&seed_dir.path().join("seeder"));
    config.seeding = SeedingPolicy::KeepSeeding;
    config.upload_limit = upload_limit;
    let seeder = Engine::start(config).await.unwrap();
    let id = seeder
        .add(
            Source::File(torrent.clone()),
            AddOptions {
                // An explicit output folder holds the files directly, with no
                // subfolder named after the torrent.
                output_folder: Some(root.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // The seeder checks the files it was pointed at; it must hold everything.
    wait_finished(&seeder, &id, Duration::from_secs(30)).await;
    let port = seeder.listen_addr().expect("seeder listens").port();
    Swarm {
        seeder,
        seed_dir,
        torrent,
        files: made,
        peer: SocketAddr::from(([127, 0, 0, 1], port)),
        id,
    }
}

impl Swarm {
    /// A leecher, in its own directory, pointed at the seeder.
    pub async fn leecher(&self, dir: &Path, seeding: SeedingPolicy) -> (Engine, String) {
        let mut config = offline_config(dir);
        config.seeding = seeding;
        let engine = Engine::start(config).await.unwrap();
        let id = engine
            .add(
                Source::File(self.torrent.clone()),
                AddOptions {
                    initial_peers: vec![self.peer],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        (engine, id)
    }

    pub fn bytes(&self, rel: &str) -> &[u8] {
        &self.files.iter().find(|(p, _)| p == rel).unwrap().1
    }
}

pub async fn wait_finished(engine: &Engine, id: &str, within: Duration) {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let s = engine.status(id).unwrap();
        if s.finished {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "not finished in {within:?}: {s:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub fn temp() -> TempDir {
    TempDir::new().unwrap()
}

pub fn path(dir: &TempDir) -> PathBuf {
    dir.path().to_path_buf()
}
