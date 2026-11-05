//! SPEC.md Phase 8 E1, below the UI: from a catalogue row, through the sources and the
//! plan, to libmpv decoding the first frame — headless, so it runs in CI.
//!
//! - **P2P**: a generated film is seeded on a loopback swarm; the plan resolves a torrent
//!   candidate to the local streaming server, and the player plays it.
//! - **Internet Archive**: the same, live (ignored by default; needs the network).

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};
use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_player::{dll_path, Options, Player};
use sinephile_sources::{Entry, Resolver};
use sinephile_torrent::{AddOptions, Engine, EngineConfig, SeedingPolicy, Source, StreamServer};
use source_protocol::{
    async_trait, BackendCapabilities, Health, MediaKind, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError, Transport,
};

fn offline(dir: &Path) -> EngineConfig {
    let mut c = EngineConfig::new(dir);
    c.dht = false;
    c.trackers = false;
    c.lsd = false;
    c.loopback_only = true;
    c.seeding = SeedingPolicy::KeepSeeding;
    c.dead_swarm_after = Duration::from_secs(10);
    c
}

/// A source that knows one swarm by its info hash, as a P2P addon would.
struct Swarm(String);

#[async_trait]
impl SourceBackend for Swarm {
    fn id(&self) -> &str {
        "test-p2p"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Transport::P2p],
            network: false,
        }
    }
    async fn find(&self, _q: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        Ok(vec![SourceCandidate {
            key: self.0.clone(),
            title: "Test Film".into(),
            year: Some(1922),
            transport: Transport::P2p,
            quality: Default::default(),
            languages: vec![],
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            health: Health::Swarm {
                seeders: 1,
                peers: 1,
            },
            locator: None,
            provenance: Default::default(),
        }])
    }
    async fn resolve(&self, c: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        Ok(PlayableStream::Torrent {
            info_hash: c.key.clone(),
            file_index: None,
            trackers: vec![],
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn search_to_first_frame_over_a_swarm() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("content").join("Test.Film.1922");
    std::fs::create_dir_all(&content).unwrap();
    let film = content.join("Test.Film.1922.mkv");
    let ok = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=24:duration=20",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=20",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "48",
            "-c:a",
            "aac",
        ])
        .arg(&film)
        .status()
        .expect("FFmpeg must be on PATH")
        .success();
    assert!(ok);

    // The swarm: a seeder holding the film, a leecher that knows the seeder.
    let created = create_torrent(
        &content,
        CreateTorrentOptions {
            name: Some("Test.Film.1922"),
            trackers: vec![],
            piece_length: Some(256 * 1024),
        },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap();
    let torrent = created.as_bytes().unwrap().to_vec();
    let seeder = Engine::start(offline(&dir.path().join("seeder")))
        .await
        .unwrap();
    let hash = seeder
        .add(
            Source::File(torrent.clone()),
            AddOptions {
                output_folder: Some(content.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let peer = std::net::SocketAddr::from(([127, 0, 0, 1], seeder.listen_addr().unwrap().port()));
    let leecher = Arc::new(
        Engine::start(offline(&dir.path().join("leecher")))
            .await
            .unwrap(),
    );
    // With DHT off the plan cannot find peers from the hash alone; the engine holding
    // the torrent already (with its peer) stands in for DHT.
    leecher
        .add(
            Source::File(torrent),
            AddOptions {
                initial_peers: vec![peer],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let server = StreamServer::start().await.unwrap();

    // The catalogue row, the resolver, the plan.
    let db = Db::in_memory().await.unwrap();
    let id = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Test Film", 1922))
        .await
        .unwrap();
    let resolver = Resolver::new(vec![Entry::new(Arc::new(Swarm(hash)))]);
    let plan = sinephile_playback::plan(&db, &resolver, Some((&leecher, &server)), id)
        .await
        .unwrap();
    assert!(plan.open.starts_with("http://127.0.0.1:"), "{}", plan.open);

    // The player, headless, on what the plan opened.
    let player = Player::new(
        &dll_path(),
        &Options {
            headless: true,
            ..Default::default()
        },
    )
    .unwrap();
    player.load(&plan.open, None).unwrap();
    let first = player.wait_first_frame(Duration::from_secs(30)).unwrap();
    println!("swarm: plan -> first frame {first:?}");
    assert!((player.state().duration.unwrap() - 20.0).abs() < 0.5);
    // A seek into the part not yet downloaded, served through the torrent.
    player
        .seek_to_and_wait(15.0, Duration::from_secs(15))
        .unwrap();
    assert!((player.state().position.unwrap() - 15.0).abs() < 0.05);
}

/// Live: `cargo test -p sinephile-playback --test end_to_end -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn live_archive_search_to_first_frame() {
    use sinephile_metadata_api::HttpTransport;
    use sinephile_sources::backends::archive::InternetArchiveBackend;
    let db = Db::in_memory().await.unwrap();
    let id = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Night of the Living Dead", 1968))
        .await
        .unwrap();
    let resolver = Resolver::new(vec![Entry::new(Arc::new(InternetArchiveBackend::new(
        Arc::new(HttpTransport::new()),
    )))]);
    let t0 = std::time::Instant::now();
    let plan = sinephile_playback::plan(&db, &resolver, None, id)
        .await
        .unwrap();
    let planned = t0.elapsed();
    let player = Player::new(
        &dll_path(),
        &Options {
            headless: true,
            ..Default::default()
        },
    )
    .unwrap();
    player.load(&plan.open, None).unwrap();
    let first = player.wait_first_frame(Duration::from_secs(30)).unwrap();
    println!(
        "archive: plan {planned:?} ({} offered, opened {}), then first frame {first:?}",
        plan.offered, plan.open
    );
}
