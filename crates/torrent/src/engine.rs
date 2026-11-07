//! The torrent engine: librqbit in-process (ADR-0003), configured for this app.
//!
//! librqbit does the protocol and, since Spike B, most of the streaming scheduler: each
//! open `FileStream` keeps a 32 MiB lookahead window at top priority, and a seek moves the
//! window at once (docs/eval-results.md, Spike B). This module is the part librqbit does
//! not decide for us:
//!
//! - **Portable state.** Downloads, session persistence and the DHT's routing table all
//!   live under `data/` (SPEC.md §2.5); librqbit's defaults would put the last two in an
//!   OS folder.
//! - **Seeding stops at completion by default** (§15 Phase 7): seeding is uploading, and
//!   uploading is distribution. librqbit has no such policy, so the engine watches each
//!   torrent and pauses it when it finishes; a finished file is then served from disk.
//! - **A dead swarm fails fast and says so** rather than hanging: adding a magnet waits
//!   for metadata only so long, and a stream with no peers gives up after a deadline.
//! - **Interface binding** (§15 Phase 7) is passed through to librqbit, which needs the
//!   Windows implementation added in this phase (7.6) to honour it.

use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use std::str::FromStr;

use librqbit::dht::DhtPersistenceConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ListenerOptions, Session, SessionOptions,
    SessionPersistenceConfig,
};
use serde::{Deserialize, Serialize};

use crate::select::{self, TorrentFile, Wanted};
use crate::server::{self, OpenFuture, ReadSeek};

type Handle = Arc<librqbit::ManagedTorrent>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedingPolicy {
    /// The default: once a torrent is complete, it stops uploading.
    StopAtCompletion,
    /// Keep sharing after completion, as a swarm-friendly user may choose to.
    KeepSeeding,
}

/// Everything the user can configure, with the defaults SPEC.md asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Root for everything the engine writes: `<data>/torrents/` for files,
    /// `<data>/torrent-session/` for state.
    pub data_dir: PathBuf,
    /// `None` picks a free port.
    pub listen_port: Option<u16>,
    /// Accept peers only from this machine. Tests set it, so a test swarm never asks
    /// Windows Firewall for permission to listen on the network.
    pub loopback_only: bool,
    pub dht: bool,
    pub trackers: bool,
    /// Local service discovery (other peers on the same network).
    pub lsd: bool,
    /// Bytes per second; `None` is unlimited.
    pub download_limit: Option<u32>,
    pub upload_limit: Option<u32>,
    /// Peers per torrent.
    pub peer_limit: Option<usize>,
    pub seeding: SeedingPolicy,
    /// A network adapter's friendly name (e.g. a VPN's). If set and the adapter goes
    /// away, torrent traffic stops rather than falling back to another connection.
    pub bind_interface: Option<String>,
    /// How long to wait for a magnet's metadata, or a stream's first peer, before
    /// saying the swarm is dead.
    pub dead_swarm_after: Duration,
}

impl EngineConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            listen_port: None,
            loopback_only: false,
            dht: true,
            trackers: true,
            lsd: true,
            download_limit: None,
            upload_limit: None,
            peer_limit: Some(128),
            seeding: SeedingPolicy::StopAtCompletion,
            bind_interface: None,
            dead_swarm_after: Duration::from_secs(30),
        }
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.data_dir.join("torrents")
    }

    pub fn session_dir(&self) -> PathBuf {
        self.data_dir.join("torrent-session")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {
    #[error("the torrent engine could not start: {0}")]
    Start(String),
    #[error("that is not a magnet link or a torrent file: {0}")]
    BadInput(String),
    #[error("nobody is sharing this — no peers answered within {0} seconds")]
    DeadSwarm(u64),
    #[error("this torrent has no video file to play")]
    NoVideo,
    #[error("no torrent with id {0}")]
    Unknown(String),
    #[error("{0}")]
    Engine(String),
}

/// A limit in bytes per second; nothing (or zero) is unlimited.
fn limit(v: Option<u32>) -> Option<NonZeroU32> {
    v.and_then(NonZeroU32::new)
}

fn engine_err(e: anyhow::Error) -> TorrentError {
    TorrentError::Engine(format!("{e:#}"))
}

/// What to add.
pub enum Source {
    /// A `magnet:` link.
    Magnet(String),
    /// The bytes of a `.torrent` file.
    File(Vec<u8>),
    /// A v1 info hash, 40 hexadecimal characters, with any trackers the source supplied
    /// (SPEC.md §2.1: the app ships none). DHT finds peers from the hash alone.
    InfoHash { hash: String, trackers: Vec<String> },
}

#[derive(Debug, Clone, Default)]
pub struct AddOptions {
    /// Peers to try first, before DHT and trackers find any. Tests use this to build a
    /// swarm on loopback; nothing in the app ships an address.
    pub initial_peers: Vec<SocketAddr>,
    /// Download only these files (by index). `None` is all.
    pub only_files: Option<Vec<usize>>,
    pub paused: bool,
    /// Where the files are, or go. `None` is `<data>/torrents/`. Set when seeding files
    /// that already exist (the test swarm does).
    pub output_folder: Option<PathBuf>,
    /// This torrent's own download limit, bytes per second (Phase 13). The global limit
    /// still applies on top.
    pub download_limit: Option<u32>,
}

/// A torrent as the UI sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub id: String,
    pub name: Option<String>,
    pub state: State,
    pub progress_bytes: u64,
    pub total_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_bps: f64,
    pub upload_bps: f64,
    pub live_peers: u32,
    pub finished: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Initializing,
    Live,
    Paused,
    Error,
}

pub struct Engine {
    session: Arc<Session>,
    config: EngineConfig,
    watcher: tokio::task::JoinHandle<()>,
}

impl Engine {
    pub async fn start(config: EngineConfig) -> Result<Self, TorrentError> {
        let downloads = config.downloads_dir();
        let state = config.session_dir();
        for dir in [&downloads, &state] {
            std::fs::create_dir_all(dir).map_err(|e| TorrentError::Start(e.to_string()))?;
        }
        let mut opts = SessionOptions {
            dht: if config.dht {
                Some(librqbit::DhtSessionConfig {
                    bootstrap_addrs: None,
                    port: None,
                    persistence: Some(DhtPersistenceConfig {
                        dump_interval: None,
                        config_filename: Some(state.join("dht.json")),
                    }),
                })
            } else {
                None
            },
            disable_trackers: !config.trackers,
            disable_local_service_discovery: !config.lsd,
            fastresume: true,
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(state.clone()),
            }),
            listen: Some(ListenerOptions {
                listen_addr: (
                    if config.loopback_only {
                        std::net::Ipv4Addr::LOCALHOST
                    } else {
                        std::net::Ipv4Addr::UNSPECIFIED
                    },
                    config.listen_port.unwrap_or(0),
                )
                    .into(),
                enable_upnp_port_forwarding: false,
                ..Default::default()
            }),
            peer_limit: config.peer_limit,
            bind_device_name: config.bind_interface.clone(),
            ipv4_only: false,
            client_name_and_version: Some(format!("sin-e-phile/{}", env!("CARGO_PKG_VERSION"))),
            ..Default::default()
        };
        opts.ratelimits.download_bps = limit(config.download_limit);
        opts.ratelimits.upload_bps = limit(config.upload_limit);

        let session = Session::new_with_opts(downloads, opts)
            .await
            .map_err(|e| TorrentError::Start(format!("{e:#}")))?;
        let watcher = tokio::spawn(watch_completion(session.clone(), config.seeding));
        Ok(Self {
            session,
            config,
            watcher,
        })
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// The address this engine accepts peers on.
    /// Change the global limits while running, bytes per second; `None` is unlimited.
    pub fn set_limits(&self, download: Option<u32>, upload: Option<u32>) {
        self.session.ratelimits.set_download_bps(limit(download));
        self.session.ratelimits.set_upload_bps(limit(upload));
    }

    pub fn listen_addr(&self) -> Option<SocketAddr> {
        self.session.listen_addr()
    }

    /// Add a torrent and wait until it is live. A magnet whose metadata does not arrive
    /// within `dead_swarm_after` is a dead swarm, said plainly.
    pub async fn add(&self, source: Source, opts: AddOptions) -> Result<String, TorrentError> {
        let trackers = match &source {
            Source::InfoHash { trackers, .. } if !trackers.is_empty() => Some(trackers.clone()),
            _ => None,
        };
        let add = match source {
            Source::Magnet(m) => {
                if !m.starts_with("magnet:?") {
                    return Err(TorrentError::BadInput("expected a magnet: link".into()));
                }
                AddTorrent::from_url(m)
            }
            Source::File(bytes) => AddTorrent::from_bytes(bytes),
            Source::InfoHash { ref hash, .. } => {
                if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(TorrentError::BadInput(
                        "an info hash is 40 hexadecimal characters".into(),
                    ));
                }
                AddTorrent::from_url(hash.to_ascii_lowercase())
            }
        };
        let options = AddTorrentOptions {
            paused: opts.paused,
            only_files: opts.only_files,
            overwrite: true,
            initial_peers: (!opts.initial_peers.is_empty()).then_some(opts.initial_peers),
            output_folder: opts.output_folder.map(|p| p.to_string_lossy().into_owned()),
            trackers,
            ratelimits: librqbit::limits::LimitsConfig {
                download_bps: limit(opts.download_limit),
                upload_bps: None,
            },
            ..Default::default()
        };
        let deadline = self.config.dead_swarm_after;
        let added = tokio::time::timeout(deadline, self.session.add_torrent(add, Some(options)))
            .await
            .map_err(|_| TorrentError::DeadSwarm(deadline.as_secs()))?
            .map_err(|e| TorrentError::BadInput(format!("{e:#}")))?;
        let handle = match added {
            AddTorrentResponse::Added(_, h) | AddTorrentResponse::AlreadyManaged(_, h) => h,
            AddTorrentResponse::ListOnly(_) => {
                return Err(TorrentError::Engine("list-only".into()))
            }
        };
        if !opts.paused {
            handle.wait_until_initialized().await.map_err(engine_err)?;
        }
        Ok(handle.info_hash().as_string())
    }

    fn handle(&self, id: &str) -> Result<Handle, TorrentError> {
        let hash =
            librqbit::dht::Id20::from_str(id).map_err(|_| TorrentError::Unknown(id.into()))?;
        self.session
            .get(librqbit::api::TorrentIdOrHash::Hash(hash))
            .ok_or_else(|| TorrentError::Unknown(id.into()))
    }

    pub fn files(&self, id: &str) -> Result<Vec<TorrentFile>, TorrentError> {
        self.handle(id)?
            .with_metadata(|m| {
                m.file_infos
                    .iter()
                    .enumerate()
                    .map(|(index, f)| TorrentFile {
                        index,
                        path: f.relative_filename.to_string_lossy().replace('\\', "/"),
                        len: f.len,
                    })
                    .collect()
            })
            .map_err(engine_err)
    }

    /// Whether the engine already has this torrent.
    pub fn has(&self, id: &str) -> bool {
        self.handle(id).is_ok()
    }

    /// The file to play (`select::choose`), or `NoVideo`.
    pub fn main_file(&self, id: &str, wanted: Wanted) -> Result<TorrentFile, TorrentError> {
        let files = self.files(id)?;
        let index = select::choose(&files, wanted).ok_or(TorrentError::NoVideo)?;
        Ok(files[index].clone())
    }

    /// Where a file of this torrent is, or will be, on disk. librqbit's output folder
    /// already holds the torrent's own subfolder, so this is a plain join (checked by
    /// `a_finished_file_on_disk_matches_the_source` in the swarm tests).
    pub fn file_path(&self, id: &str, file: &TorrentFile) -> Result<PathBuf, TorrentError> {
        Ok(self.handle(id)?.output_folder().join(&file.path))
    }

    /// A reader over one file that pulls the pieces it needs at top priority.
    pub async fn stream(&self, id: &str, file: usize) -> Result<Box<dyn ReadSeek>, TorrentError> {
        let h = self.handle(id)?;
        let s = h.stream(file).await.map_err(engine_err)?;
        Ok(Box::new(s))
    }

    /// What the streaming server serves for one file of a torrent: the file on disk
    /// once the torrent has finished, otherwise a live stream that pulls the pieces it
    /// needs. Each request gets its own reader, so concurrent seeks do not interfere.
    pub fn stream_source(
        self: &Arc<Self>,
        id: &str,
        file: &TorrentFile,
    ) -> Result<Arc<dyn server::Source>, TorrentError> {
        self.handle(id)?;
        Ok(Arc::new(TorrentStream {
            engine: self.clone(),
            id: id.to_string(),
            path: self.file_path(id, file)?,
            file: file.clone(),
        }))
    }

    /// Wait until the torrent has a live peer or is complete. A torrent nobody is
    /// sharing would otherwise make a stream wait forever, which the player would show
    /// as a spinner that never ends (E4).
    pub async fn wait_for_peers(&self, id: &str) -> Result<(), TorrentError> {
        let deadline = tokio::time::Instant::now() + self.config.dead_swarm_after;
        loop {
            let s = self.status(id)?;
            if s.finished || s.live_peers > 0 {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(TorrentError::DeadSwarm(
                    self.config.dead_swarm_after.as_secs(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Wait until a torrent has finished checking what is already on disk — after a
    /// restart, its progress is not known until then.
    pub async fn wait_ready(&self, id: &str) -> Result<(), TorrentError> {
        self.handle(id)?
            .wait_until_initialized()
            .await
            .map_err(engine_err)
    }

    pub fn status(&self, id: &str) -> Result<Status, TorrentError> {
        Ok(status_of(&self.handle(id)?))
    }

    pub fn list(&self) -> Vec<Status> {
        self.session
            .with_torrents(|it| it.map(|(_, h)| status_of(h)).collect())
    }

    pub async fn pause(&self, id: &str) -> Result<(), TorrentError> {
        let h = self.handle(id)?;
        self.session.pause(&h).await.map_err(engine_err)
    }

    pub async fn resume(&self, id: &str) -> Result<(), TorrentError> {
        let h = self.handle(id)?;
        self.session.unpause(&h).await.map_err(engine_err)
    }

    pub async fn remove(&self, id: &str, delete_files: bool) -> Result<(), TorrentError> {
        let h = self.handle(id)?;
        self.session
            .delete(librqbit::api::TorrentIdOrHash::Id(h.id()), delete_files)
            .await
            .map_err(engine_err)
    }

    /// Stop the session, saving its state for the next start.
    pub async fn stop(self) {
        self.watcher.abort();
        self.session.stop().await;
    }
}

fn status_of(h: &Handle) -> Status {
    let s = h.stats();
    let state = match s.state {
        librqbit::TorrentStatsState::Initializing { .. } => State::Initializing,
        librqbit::TorrentStatsState::Live => State::Live,
        librqbit::TorrentStatsState::Paused => State::Paused,
        librqbit::TorrentStatsState::Error => State::Error,
    };
    let (down, up, peers) = s
        .live
        .as_ref()
        .map(|l| {
            (
                l.download_speed.mbps * 1_000_000.0 / 8.0,
                l.upload_speed.mbps * 1_000_000.0 / 8.0,
                l.snapshot.peer_stats.live,
            )
        })
        .unwrap_or((0.0, 0.0, 0));
    Status {
        id: h.info_hash().as_string(),
        name: h.name(),
        state,
        progress_bytes: s.progress_bytes,
        total_bytes: s.total_bytes,
        uploaded_bytes: s.uploaded_bytes,
        download_bps: down,
        upload_bps: up,
        live_peers: peers,
        finished: s.finished,
        error: s.error,
    }
}

/// The seeding policy, enforced: a torrent that finishes is paused, which stops it
/// uploading. Polls rather than subscribes because librqbit exposes no completion
/// event; once a second is far finer than the policy needs.
async fn watch_completion(session: Arc<Session>, policy: SeedingPolicy) {
    if policy == SeedingPolicy::KeepSeeding {
        return;
    }
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tick.tick().await;
        let finished: Vec<_> = session.with_torrents(|it| {
            it.filter(|(_, h)| {
                let s = h.stats();
                s.finished && matches!(s.state, librqbit::TorrentStatsState::Live)
            })
            .map(|(_, h)| h.clone())
            .collect()
        });
        for h in finished {
            if let Err(e) = session.pause(&h).await {
                tracing::warn!(error = %e, "could not stop seeding a finished torrent");
            } else {
                tracing::info!(torrent = %h.info_hash().as_string(), "finished; seeding stopped");
            }
        }
    }
}

struct TorrentStream {
    engine: Arc<Engine>,
    id: String,
    path: PathBuf,
    file: TorrentFile,
}

impl server::Source for TorrentStream {
    fn len(&self) -> u64 {
        self.file.len
    }

    fn content_type(&self) -> &str {
        server::content_type_for(&self.file.path)
    }

    fn open(&self) -> OpenFuture<'_> {
        Box::pin(async move {
            let io = |e: TorrentError| std::io::Error::other(e.to_string());
            let status = self.engine.status(&self.id).map_err(io)?;
            if status.finished {
                // Complete, and possibly paused by the seeding policy: read the disk.
                let f = tokio::fs::File::open(&self.path).await?;
                return Ok(Box::new(f) as Box<dyn ReadSeek>);
            }
            self.engine.wait_for_peers(&self.id).await.map_err(io)?;
            self.engine
                .stream(&self.id, self.file.index)
                .await
                .map_err(io)
        })
    }
}
