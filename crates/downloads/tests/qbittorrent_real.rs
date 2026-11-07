//! Phase 13 E2: one download through a REAL qBittorrent and the same one through librqbit
//! file into the library identically, compared row by row.
//!
//! Needs qBittorrent installed; run explicitly:
//! `cargo test -p sinephile-downloads --test qbittorrent_real -- --ignored --nocapture`
//! (`QBITTORRENT_EXE` overrides the usual install path).
//!
//! The user's own qBittorrent is never touched. Two more instances run, each with its own
//! profile in a temporary folder (`--profile`), its Web UI on a port of its own bound to
//! 127.0.0.1, and DHT, peer exchange, local discovery and port forwarding off. Before
//! anything is added to an instance, the test checks the one answering is that one (its save
//! path is the temporary folder). Only processes the test started are stopped.
//!
//! - **The seeder** is a qBittorrent given the `.torrent` through its watched folder, with
//!   the content already in place: libtorrent seeding to libtorrent, as on the internet.
//!   (The `.torrent` is made by librqbit, whose `create_torrent` writes one piece hash too
//!   many for a size that is a whole number of pieces, D80; `SIZE` is not one.)
//! - **The downloader** is driven only through the app's `Qbittorrent` backend, by link,
//!   exactly as the app drives the user's own.
//!
//! The content is random bytes made here; nothing comes from anywhere.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, Instant};

use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};
use sinephile_downloads::qbittorrent::Qbittorrent;
use sinephile_downloads::{Backend, Manager, Rqbit};
use sinephile_library::scan::Prober;
use sinephile_metadata_api::reqwest_transport::HttpTransport;
use sinephile_metadata_api::transport::{Method, Request};
use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::{
    DownloadRepository, DownloadState, LibraryRepository, MediaRepository, NewDownload, Probe,
};
use sinephile_persistence::{Db, NewMediaItem};
use sinephile_torrent::{AddOptions, Engine, EngineConfig, SeedingPolicy, Source};
use tempfile::TempDir;

const MIB: usize = 1024 * 1024;
const NOW: &str = "2026-11-07T21:00:00Z";
const NAME: &str = "Heat.1995.1080p";
/// Not a whole number of 256 KiB pieces: librqbit's `create_torrent` writes one piece hash
/// too many when the size is (D80), which libtorrent rightly refuses as invalid.
const SIZE: usize = 6 * MIB + 12_345;

/// qBittorrent 5 will not start its Web UI until a login is set, even with local logins
/// off. So each isolated instance gets a throwaway one: the PBKDF2-SHA512 form qBittorrent
/// stores (16-byte salt 0..15, 100,000 rounds) of the password "isolated-test-instance".
/// Not a secret: it guards instances on 127.0.0.1 that live for one test.
const THROWAWAY_LOGIN: &str = "@ByteArray(AAECAwQFBgcICQoLDA0ODw==:Rcq98G93qx+RiVxgAMtinlC4/LF6F6AyYOduAl9qYHMkIkM88SXCAZhoD3glTpdRvYGiRgvZiD/7H9nOfV31GA==)";

fn offline(dir: &Path) -> EngineConfig {
    let mut c = EngineConfig::new(dir);
    c.dht = false;
    c.trackers = false;
    c.lsd = false;
    c.loopback_only = true;
    c.dead_swarm_after = Duration::from_secs(20);
    c
}

fn content(len: usize) -> Vec<u8> {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// The film's files under `<dir>/content/<NAME>/`, and its `.torrent`.
async fn make(dir: &Path, len: usize) -> (PathBuf, Vec<u8>) {
    let parent = dir.join("content");
    let root = parent.join(NAME);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(format!("{NAME}.mkv")), content(len)).unwrap();
    let torrent = create_torrent(
        &root,
        CreateTorrentOptions {
            name: Some(NAME),
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
    (parent, torrent)
}

/// A librqbit seeder on loopback, for the librqbit side; returns it, the hash, its address.
async fn rqbit_seeder(dir: &Path, parent: &Path, torrent: &[u8]) -> (Engine, String, SocketAddr) {
    let mut config = offline(&dir.join("seeder"));
    config.seeding = SeedingPolicy::KeepSeeding;
    let engine = Engine::start(config).await.unwrap();
    let hash = engine
        .add(
            Source::File(torrent.to_vec()),
            AddOptions {
                output_folder: Some(parent.join(NAME)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let port = engine.listen_addr().unwrap().port();
    (engine, hash, SocketAddr::from(([127, 0, 0, 1], port)))
}

fn no_probe(_: &Path) -> Option<Probe> {
    None
}

/// What the library holds for a finished download, with its paths made relative to the
/// downloads folder: what "identically" is judged on.
#[derive(Debug, PartialEq)]
struct Filed {
    state: DownloadState,
    done_bytes: i64,
    total_bytes: Option<i64>,
    file: String,
    size_bytes: i64,
    media_item_id: i64,
    confidence: f64,
    confirmed: bool,
    method: String,
    covers: i64,
    folder_registered: bool,
}

async fn filed(db: &Db, id: i64, downloads: &Path) -> Filed {
    let d = DownloadRepository::new(db).get(id).await.unwrap().unwrap();
    let path = d.file_path.clone().expect("a file");
    let library = LibraryRepository::new(db);
    let row = library
        .file_by_path(&path)
        .await
        .unwrap()
        .expect("a library row");
    let m = library.match_of(row.id).await.unwrap().expect("a match");
    let folders = library.folders().await.unwrap();
    Filed {
        state: d.state,
        done_bytes: d.done_bytes,
        total_bytes: d.total_bytes,
        file: Path::new(&path)
            .strip_prefix(downloads)
            .unwrap_or_else(|_| panic!("{path} is not in {}", downloads.display()))
            .to_string_lossy()
            .replace('\\', "/"),
        size_bytes: row.size_bytes,
        media_item_id: m.media_item_id,
        confidence: m.confidence,
        confirmed: m.confirmed,
        method: format!("{:?}", m.method),
        covers: m.covers,
        folder_registered: folders.iter().any(|f| Path::new(&f.path) == downloads),
    }
}

/// Tick until the download is over; it must be done.
async fn run_to_done(m: &Manager, db: &Db, id: i64, within: Duration) {
    let deadline = Instant::now() + within;
    loop {
        m.tick(NOW).await.unwrap();
        let d = DownloadRepository::new(db).get(id).await.unwrap().unwrap();
        if d.state.is_over() {
            assert_eq!(d.state, DownloadState::Done, "{d:?}");
            return;
        }
        assert!(Instant::now() < deadline, "not done in {within:?}: {d:?}");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

/// A database with "Heat" in it, and the download of it by `backend`.
async fn film(hash: &str, backend: &str) -> (Db, i64, i64) {
    let db = Db::in_memory().await.unwrap();
    let heat = MediaRepository::new(&db)
        .insert(&NewMediaItem::film("Heat", 1995))
        .await
        .unwrap();
    let source = format!("infohash:{hash}");
    let id = DownloadRepository::new(&db)
        .add(&NewDownload {
            media_item_id: Some(heat),
            title: "Heat",
            source: &source,
            backend,
            ..Default::default()
        })
        .await
        .unwrap();
    (db, heat, id)
}

/// One isolated qBittorrent. Stopped (only it) when dropped.
struct Isolated {
    child: Child,
    base: String,
    /// Where it listens for peers, on loopback.
    peer: SocketAddr,
    root: PathBuf,
}

impl Drop for Isolated {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Isolated {
    fn log(&self) -> String {
        let log = self
            .root
            .join("profile/qBittorrent/data/logs/qbittorrent.log");
        std::fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.contains("listening on IP"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Start one with `save` as its save path and, if given, `watch` as a watched folder whose
/// torrents are saved to `save`. Waits for its Web UI and checks it is this instance.
async fn start(exe: &Path, root: &Path, save: &Path, watch: Option<&Path>, port: u16) -> Isolated {
    let config = root.join("profile").join("qBittorrent").join("config");
    std::fs::create_dir_all(&config).unwrap();
    let fwd = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let save_ini = fwd(save);
    let peer_port = port + 1;
    let ini = format!(
        "[LegalNotice]\r\nAccepted=true\r\n\r\n\
         [BitTorrent]\r\nSession\\DefaultSavePath={save_ini}\r\nSession\\DHTEnabled=false\r\n\
         Session\\LSDEnabled=false\r\nSession\\PeXEnabled=false\r\nSession\\Port={peer_port}\r\n\
         Session\\QueueingSystemEnabled=false\r\n\r\n\
         [Network]\r\nPortForwardingEnabled=false\r\n\r\n\
         [Preferences]\r\nWebUI\\Enabled=true\r\nWebUI\\Port={port}\r\nWebUI\\Address=127.0.0.1\r\n\
         WebUI\\Username=sinephile-test\r\nWebUI\\Password_PBKDF2=\"{THROWAWAY_LOGIN}\"\r\n\
         WebUI\\LocalHostAuth=false\r\nWebUI\\HostHeaderValidation=false\r\n\
         Downloads\\SavePath={save_ini}\r\nConnection\\UPnP=false\r\nGeneral\\Locale=en\r\n"
    );
    std::fs::write(config.join("qBittorrent.ini"), ini).unwrap();
    if let Some(w) = watch {
        std::fs::create_dir_all(w).unwrap();
        let watched = serde_json::json!({
            fwd(w): {
                "add_torrent_params": {
                    "save_path": save_ini,
                    "use_auto_tmm": false,
                    "stopped": false,
                    "content_layout": "Original"
                },
                "recursive": false
            }
        });
        std::fs::write(config.join("watched_folders.json"), watched.to_string()).unwrap();
    }
    let child = Command::new(exe)
        .arg(format!("--profile={}", root.join("profile").display()))
        .arg(format!("--webui-port={port}"))
        .arg("--confirm-legal-notice")
        .arg("--no-splash")
        .spawn()
        .expect("qBittorrent starts");
    let me = Isolated {
        child,
        base: format!("http://127.0.0.1:{port}"),
        peer: SocketAddr::from(([127, 0, 0, 1], peer_port)),
        root: root.to_path_buf(),
    };
    let http = HttpTransport::untrusted();
    let t0 = Instant::now();
    while api(&http, &me.base, Method::Get, "app/version", None)
        .await
        .0
        != 200
    {
        assert!(
            t0.elapsed() < Duration::from_secs(60),
            "its Web UI did not come up:\n{}",
            me.log()
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    // The instance answering must be this one before anything is added to it.
    let (_, prefs) = api(&http, &me.base, Method::Get, "app/preferences", None).await;
    let prefs: serde_json::Value = serde_json::from_str(&prefs).expect("preferences");
    let answered = prefs["save_path"]
        .as_str()
        .unwrap_or_default()
        .replace('\\', "/");
    assert_eq!(
        answered.trim_end_matches('/').to_lowercase(),
        save_ini.trim_end_matches('/').to_lowercase(),
        "the instance answering on {} is not the isolated one; stopping before touching it",
        me.base
    );
    me
}

async fn api(
    http: &HttpTransport,
    base: &str,
    method: Method,
    path: &str,
    body: Option<String>,
) -> (u16, String) {
    let mut r = Request::get(format!("{base}/api/v2/{path}")).limit(1024 * 1024);
    r.method = method;
    r = r.header("referer", base.to_string());
    if let Some(b) = body {
        r.body = Some(b);
        r = r.header("content-type", "application/x-www-form-urlencoded");
    }
    match http.send(r).await {
        Ok(resp) => (resp.status, resp.body),
        Err(_) => (0, String::new()),
    }
}

/// One torrent's state and progress, as qBittorrent reports it.
async fn state(http: &HttpTransport, base: &str, hash: &str) -> (String, f64) {
    let (_, info) = api(
        http,
        base,
        Method::Get,
        &format!("torrents/info?hashes={hash}"),
        None,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&info).unwrap_or_default();
    (
        v[0]["state"].as_str().unwrap_or("absent").to_string(),
        v[0]["progress"].as_f64().unwrap_or(0.0),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs qBittorrent installed; run with --ignored (Phase 13 E2)"]
async fn a_download_through_qbittorrent_files_exactly_as_one_through_librqbit() {
    let exe = std::env::var_os("QBITTORRENT_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\qBittorrent\qbittorrent.exe"));
    assert!(exe.exists(), "qBittorrent is not at {}", exe.display());
    let work = TempDir::new().unwrap();
    let (parent, torrent) = make(work.path(), SIZE).await;

    // librqbit, from a librqbit seeder.
    let (_seeder, hash, peer) = rqbit_seeder(work.path(), &parent, &torrent).await;
    let rq_dir = work.path().join("rqbit").join("Downloads");
    let (rq_db, heat, rq_id) = film(&hash, "librqbit").await;
    let engine = Arc::new(
        Engine::start(offline(&work.path().join("rqbit").join("engine")))
            .await
            .unwrap(),
    );
    let mut rqbit = Rqbit::new(engine);
    rqbit.initial_peers = vec![peer];
    let m = Manager::new(
        rq_db.clone(),
        vec![Arc::new(rqbit)],
        rq_dir.clone(),
        no_probe as Prober,
    );
    run_to_done(&m, &rq_db, rq_id, Duration::from_secs(60)).await;
    let by_rqbit = filed(&rq_db, rq_id, &rq_dir).await;
    println!("librqbit:    {by_rqbit:?}");

    // qBittorrent: a seeding instance, then the downloading one the app drives.
    let http = Arc::new(HttpTransport::untrusted());
    let port = 18_000 + (std::process::id() % 1000) as u16 * 4;
    let watch = work.path().join("seed-watch");
    let seeder = start(
        &exe,
        &work.path().join("qb-seed"),
        &parent,
        Some(&watch),
        port,
    )
    .await;
    std::fs::write(watch.join(format!("{NAME}.torrent")), &torrent).unwrap();
    let t0 = Instant::now();
    loop {
        let (s, p) = state(&http, &seeder.base, &hash).await;
        if p >= 1.0 && s.contains("UP") {
            break;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(60),
            "the seeding instance did not take the torrent ({s}, {p}):\n{}",
            seeder.log()
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    println!("seeding instance: {} at {}", seeder.base, seeder.peer);

    let qb_root = work.path().join("qbit");
    let qb_dir = qb_root.join("Downloads");
    std::fs::create_dir_all(&qb_dir).unwrap();
    let downloader = start(&exe, &qb_root, &qb_dir, None, port + 2).await;
    let (qb_db, qb_heat, qb_id) = film(&hash, "qbittorrent").await;
    assert_eq!(qb_heat, heat);
    let qbit = Qbittorrent::new(http.clone(), &downloader.base, None, None);
    assert!(qbit.available().await, "the backend sees it");
    let m = Manager::new(
        qb_db.clone(),
        vec![Arc::new(qbit)],
        qb_dir.clone(),
        no_probe as Prober,
    );
    m.tick(NOW).await.unwrap();
    let d = DownloadRepository::new(&qb_db)
        .get(qb_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (d.backend.as_str(), d.state),
        ("qbittorrent", DownloadState::Active),
        "{d:?}"
    );
    // No trackers and no DHT here, so the seeding instance's address is given directly
    // (test only; the app's link carries the source's trackers).
    let peers = format!(
        "hashes={hash}&peers={}",
        url::form_urlencoded::byte_serialize(seeder.peer.to_string().as_bytes())
            .collect::<String>()
    );
    let (status, _) = api(
        &http,
        &downloader.base,
        Method::Post,
        "torrents/addPeers",
        Some(peers),
    )
    .await;
    assert_eq!(status, 200, "addPeers");
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        m.tick(NOW).await.unwrap();
        let d = DownloadRepository::new(&qb_db)
            .get(qb_id)
            .await
            .unwrap()
            .unwrap();
        if d.state.is_over() {
            assert_eq!(
                d.state,
                DownloadState::Done,
                "{d:?}\nqBittorrent says {:?}\n{}",
                state(&http, &downloader.base, &hash).await,
                downloader.log()
            );
            break;
        }
        assert!(Instant::now() < deadline, "not done: {d:?}");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let by_qbittorrent = filed(&qb_db, qb_id, &qb_dir).await;
    println!("qBittorrent: {by_qbittorrent:?}");

    assert_eq!(
        by_qbittorrent, by_rqbit,
        "filed identically, whichever moved the bytes"
    );
    assert_eq!(by_rqbit.size_bytes, SIZE as i64);
}
