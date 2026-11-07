//! What moves the bytes (ADR-0029): librqbit in-process by default, the user's own
//! qBittorrent when configured. The queue does not care which.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use sinephile_persistence::repositories::Download;
use sinephile_torrent::{AddOptions, Engine, Source, Wanted};

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("{0} is not running")]
    Unavailable(&'static str),
    #[error("{0}")]
    Refused(String),
    #[error("no download with id {0}")]
    Unknown(String),
}

/// A download's progress as its backend reports it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    /// Bytes per second.
    pub speed: f64,
    pub peers: u32,
    pub finished: bool,
    pub error: Option<String>,
}

#[async_trait]
pub trait Backend: Send + Sync {
    /// "librqbit" | "qbittorrent", as stored in `downloads.backend`.
    fn name(&self) -> &'static str;
    /// Whether it can take work now. librqbit always can; qBittorrent only if running.
    async fn available(&self) -> bool;
    /// Whether it can fetch this download at all (qBittorrent takes links and hashes,
    /// not a `.torrent` file's bytes).
    fn accepts(&self, _d: &Download) -> bool {
        true
    }
    /// Whether it already holds this download (after a restart, it may not).
    async fn holds(&self, id: &str) -> bool;
    /// Start fetching into `dir`; returns the backend's id for it.
    async fn add(&self, d: &Download, dir: &Path) -> Result<String, BackendError>;
    async fn progress(&self, id: &str) -> Result<Progress, BackendError>;
    async fn pause(&self, id: &str) -> Result<(), BackendError>;
    async fn resume(&self, id: &str) -> Result<(), BackendError>;
    async fn remove(&self, id: &str, delete_files: bool) -> Result<(), BackendError>;
    /// The finished download's video file.
    async fn video_file(&self, id: &str) -> Result<PathBuf, BackendError>;
}

/// What a download's `source` and `torrent` columns say to fetch. An info hash may carry
/// the trackers its source named: `infohash:<hex>|<tracker>|<tracker>`. (Supplied by the
/// source at run time, like everything about a torrent; the app ships none.)
pub fn source_of(d: &Download) -> Result<Source, BackendError> {
    if let Some(bytes) = &d.torrent {
        return Ok(Source::File(bytes.clone()));
    }
    if d.source.starts_with("magnet:") {
        return Ok(Source::Magnet(d.source.clone()));
    }
    if let Some(rest) = d.source.strip_prefix("infohash:") {
        let mut parts = rest.split('|');
        let hash = parts.next().unwrap_or_default().to_string();
        let trackers = parts
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        return Ok(Source::InfoHash { hash, trackers });
    }
    Err(BackendError::Refused(format!("cannot fetch {}", d.title)))
}

/// A download's own folder name: its title, with what Windows forbids in a name removed.
pub fn folder_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim();
    if cleaned.is_empty() {
        "Download".into()
    } else {
        cleaned.to_string()
    }
}

/// librqbit, in-process: the engine streaming already uses.
pub struct Rqbit {
    engine: Arc<Engine>,
    /// Peers to try first. Empty in the app; the tests' loopback seeder.
    pub initial_peers: Vec<SocketAddr>,
}

impl Rqbit {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self {
            engine,
            initial_peers: Vec::new(),
        }
    }

    /// The folder a torrent's files are in: a file's path less the path inside the torrent.
    fn output_folder(&self, id: &str) -> Option<PathBuf> {
        let files = self.engine.files(id).ok()?;
        let first = files.first()?;
        let path = self.engine.file_path(id, first).ok()?;
        let depth = Path::new(&first.path).components().count();
        path.ancestors().nth(depth).map(Path::to_path_buf)
    }
}

fn refused(e: impl std::fmt::Display) -> BackendError {
    BackendError::Refused(e.to_string())
}

#[async_trait]
impl Backend for Rqbit {
    fn name(&self) -> &'static str {
        "librqbit"
    }

    async fn available(&self) -> bool {
        true
    }

    async fn holds(&self, id: &str) -> bool {
        self.engine.has(id)
    }

    async fn add(&self, d: &Download, dir: &Path) -> Result<String, BackendError> {
        let limit = d
            .limit_kib
            .and_then(|k| u32::try_from(k.saturating_mul(1024)).ok());
        let target = dir.join(folder_name(&d.title));
        let options = || AddOptions {
            initial_peers: self.initial_peers.clone(),
            output_folder: Some(target.clone()),
            download_limit: limit,
            ..Default::default()
        };
        let id = self
            .engine
            .add(source_of(d)?, options())
            .await
            .map_err(refused)?;
        // The film page starts a torrent to stream before anyone asks to download it
        // (D61), into the engine's own folder. Moved into the downloads folder with the
        // pieces it already has, so the download carries on rather than starting again.
        if let Some(old) = self.output_folder(&id) {
            if old != target {
                self.engine.remove(&id, false).await.map_err(refused)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                if !target.exists() {
                    // Same drive: a rename. Another drive: the pieces are fetched again.
                    std::fs::rename(&old, &target).ok();
                }
                return self
                    .engine
                    .add(source_of(d)?, options())
                    .await
                    .map_err(refused);
            }
        }
        Ok(id)
    }

    async fn progress(&self, id: &str) -> Result<Progress, BackendError> {
        let s = self
            .engine
            .status(id)
            .map_err(|_| BackendError::Unknown(id.into()))?;
        Ok(Progress {
            done: s.progress_bytes,
            total: s.total_bytes,
            speed: s.download_bps,
            peers: s.live_peers,
            finished: s.finished,
            error: s.error,
        })
    }

    async fn pause(&self, id: &str) -> Result<(), BackendError> {
        self.engine.pause(id).await.map_err(refused)
    }

    async fn resume(&self, id: &str) -> Result<(), BackendError> {
        self.engine.resume(id).await.map_err(refused)
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<(), BackendError> {
        self.engine.remove(id, delete_files).await.map_err(refused)
    }

    async fn video_file(&self, id: &str) -> Result<PathBuf, BackendError> {
        let file = self
            .engine
            .main_file(id, Wanted::default())
            .map_err(refused)?;
        self.engine.file_path(id, &file).map_err(refused)
    }
}
