//! Running the queue: start what the plan says, follow progress, and file a finished
//! download into the library so it is indistinguishable from a file already there.
//!
//! `tick` is called every second or so by the app. Everything it decides is stored, so a
//! restart picks up where it left off: a download the backend no longer holds is added
//! again into the same folder, and the pieces already on disk are kept.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sinephile_library::identify::Cache;
use sinephile_library::scan::{identify_file, on_disk, record, Prober};
use sinephile_persistence::repositories::{
    Download, DownloadRepository, DownloadState, FileMatch, LibraryRepository, MatchMethod,
};
use sinephile_persistence::{Db, DbError};

use crate::backend::{Backend, BackendError};
use crate::queue::{plan, Step};

#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// What one tick did, for the log and the tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tick {
    pub started: Vec<i64>,
    pub held: Vec<i64>,
    pub finished: Vec<i64>,
    pub failed: Vec<i64>,
}

/// A moment as ISO 8601 UTC, the form `not_before` is stored in (Howard Hinnant's
/// civil-from-days; no date library for one function).
pub fn iso_utc(unix_seconds: i64) -> String {
    let secs = unix_seconds.rem_euclid(86_400);
    let z = unix_seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    iso_utc(secs)
}

pub struct Manager {
    db: Db,
    backends: Vec<Arc<dyn Backend>>,
    /// Where downloads land. Registered as a library folder, so they file themselves.
    pub dir: PathBuf,
    /// How many run at once.
    pub max_active: usize,
    probe: Prober,
}

impl Manager {
    pub fn new(db: Db, backends: Vec<Arc<dyn Backend>>, dir: PathBuf, probe: Prober) -> Self {
        Self {
            db,
            backends,
            dir,
            max_active: 3,
            probe,
        }
    }

    fn backend(&self, name: &str) -> Option<&Arc<dyn Backend>> {
        self.backends.iter().find(|b| b.name() == name)
    }

    /// The backend a download should use now: its own if it can take work, else the
    /// first that can (ADR-0029: a configured qBittorrent that is not running falls back
    /// to librqbit rather than failing).
    async fn usable(&self, d: &Download) -> Option<Arc<dyn Backend>> {
        if let Some(own) = self.backend(&d.backend) {
            if own.available().await {
                return Some(own.clone());
            }
        }
        for b in &self.backends {
            if b.available().await {
                return Some(b.clone());
            }
        }
        None
    }

    /// Start a download: added to its backend if that does not hold it yet.
    async fn start(&self, d: &Download) -> Result<(), ManagerError> {
        let repo = DownloadRepository::new(&self.db);
        let Some(backend) = self.usable(d).await else {
            return Err(BackendError::Unavailable("no download backend").into());
        };
        let held = match &d.backend_id {
            Some(id) if d.backend == backend.name() => backend.holds(id).await,
            _ => false,
        };
        if held {
            let id = d.backend_id.as_deref().unwrap_or_default();
            backend.resume(id).await?;
        } else {
            std::fs::create_dir_all(&self.dir).ok();
            let id = backend.add(d, &self.dir).await?;
            repo.set_backend_id(d.id, backend.name(), &id).await?;
        }
        repo.set_state(d.id, DownloadState::Active).await?;
        Ok(())
    }

    /// One pass: progress for what runs, the plan applied, the finished filed.
    pub async fn tick(&self, now: &str) -> Result<Tick, ManagerError> {
        let repo = DownloadRepository::new(&self.db);
        let mut out = Tick::default();

        for d in repo.open().await? {
            if d.state != DownloadState::Active {
                continue;
            }
            let (Some(backend), Some(id)) = (self.backend(&d.backend), d.backend_id.as_deref())
            else {
                continue;
            };
            if !backend.holds(id).await {
                // After a restart: added again, into the same folder, keeping its pieces.
                if let Err(e) = self.start(&d).await {
                    repo.fail(d.id, &e.to_string()).await?;
                    out.failed.push(d.id);
                }
                continue;
            }
            let p = match backend.progress(id).await {
                Ok(p) => p,
                Err(e) => {
                    repo.fail(d.id, &e.to_string()).await?;
                    out.failed.push(d.id);
                    continue;
                }
            };
            repo.set_progress(d.id, p.done as i64, (p.total > 0).then_some(p.total as i64))
                .await?;
            if let Some(error) = p.error {
                repo.fail(d.id, &error).await?;
                out.failed.push(d.id);
            } else if p.finished {
                let file = backend.video_file(id).await?;
                self.file(&d, &file).await?;
                out.finished.push(d.id);
            }
        }

        let open = repo.open().await?;
        for step in plan(&open, now, self.max_active) {
            match step {
                Step::Start(id) => {
                    let Some(d) = open.iter().find(|d| d.id == id) else {
                        continue;
                    };
                    match self.start(d).await {
                        Ok(()) => out.started.push(id),
                        Err(e) => {
                            repo.fail(id, &e.to_string()).await?;
                            out.failed.push(id);
                        }
                    }
                }
                Step::Hold(id) => {
                    let Some(d) = open.iter().find(|d| d.id == id) else {
                        continue;
                    };
                    if let (Some(b), Some(bid)) =
                        (self.backend(&d.backend), d.backend_id.as_deref())
                    {
                        b.pause(bid).await?;
                    }
                    repo.set_state(id, DownloadState::Queued).await?;
                    out.held.push(id);
                }
            }
        }
        Ok(out)
    }

    /// File a finished download into the library: the downloads folder is a library
    /// folder, so the file is recorded like any other, and when the download knew what it
    /// was, the match is that, confirmed (E6: no user action).
    async fn file(&self, d: &Download, file: &Path) -> Result<(), ManagerError> {
        let repo = DownloadRepository::new(&self.db);
        let library = LibraryRepository::new(&self.db);
        let path = file.to_string_lossy().into_owned();
        repo.finish(d.id, &path).await?;
        let folder_id = library.add_folder(&self.dir.to_string_lossy()).await?;
        let Some(found) = on_disk(file) else {
            return Ok(());
        };
        let (up, _) = record(&self.db, folder_id, &found).await?;
        match d.media_item_id {
            Some(item) => {
                library
                    .set_match(
                        up.id(),
                        &FileMatch {
                            media_item_id: item,
                            confidence: 1.0,
                            method: MatchMethod::Metadata,
                            covers: 1,
                            confirmed: true,
                        },
                    )
                    .await?;
            }
            None => {
                if let Some(f) = library.file(up.id()).await? {
                    identify_file(&self.db, &self.dir, &f, self.probe, &Cache::default()).await?;
                }
            }
        }
        Ok(())
    }

    /// Paused by the user: the queue will not start it again by itself.
    pub async fn pause(&self, id: i64) -> Result<(), ManagerError> {
        let repo = DownloadRepository::new(&self.db);
        if let Some(d) = repo.get(id).await? {
            if let (Some(b), Some(bid)) = (self.backend(&d.backend), d.backend_id.as_deref()) {
                if b.holds(bid).await {
                    b.pause(bid).await?;
                }
            }
            repo.set_state(id, DownloadState::Paused).await?;
        }
        Ok(())
    }

    /// Back in the queue; the next tick starts it if there is room.
    pub async fn resume(&self, id: i64) -> Result<(), ManagerError> {
        DownloadRepository::new(&self.db)
            .set_state(id, DownloadState::Queued)
            .await?;
        Ok(())
    }

    /// Stop it for good, and delete what was fetched unless told to keep it.
    pub async fn cancel(&self, id: i64, delete_files: bool) -> Result<(), ManagerError> {
        let repo = DownloadRepository::new(&self.db);
        if let Some(d) = repo.get(id).await? {
            if let (Some(b), Some(bid)) = (self.backend(&d.backend), d.backend_id.as_deref()) {
                if b.holds(bid).await {
                    b.remove(bid, delete_files).await?;
                }
            }
            repo.set_state(id, DownloadState::Cancelled).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unix_time_reads_as_iso_utc() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_794_268_800), "2026-11-10T00:00:00Z");
        assert_eq!(iso_utc(951_782_400 + 3_661), "2000-02-29T01:01:01Z");
    }
}
