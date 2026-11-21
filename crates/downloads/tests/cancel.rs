//! Cancelling while the queue is mid-pass (the author, 2026-11-20: "downloading and then
//! cancelling a download doesnt work sometimes"). The queue ticks once a second from its own
//! task; a cancel arrives from the page whenever the viewer presses it. A backend that stalls
//! inside a tick holds the pass open, so the cancel lands in the middle of it, every time.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use sinephile_downloads::{Backend, BackendError, Manager, Progress};
use sinephile_library::scan::Prober;
use sinephile_persistence::repositories::{
    Download, DownloadRepository, DownloadState, NewDownload, Probe,
};
use sinephile_persistence::Db;
use tokio::sync::Notify;

const NOW: &str = "2026-11-20T21:00:00Z";

/// Holds what it was given; `holds` can be made to stall once, the way a busy backend does.
#[derive(Default)]
struct Stalling {
    held: Mutex<HashSet<String>>,
    added: Mutex<u32>,
    stall: Mutex<bool>,
    entered: Notify,
    release: Notify,
}

#[async_trait]
impl Backend for Stalling {
    fn name(&self) -> &'static str {
        "librqbit"
    }
    async fn available(&self) -> bool {
        true
    }
    async fn holds(&self, id: &str) -> bool {
        let stall = std::mem::take(&mut *self.stall.lock().unwrap());
        if stall {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.held.lock().unwrap().contains(id)
    }
    async fn add(&self, d: &Download, _dir: &Path) -> Result<String, BackendError> {
        *self.added.lock().unwrap() += 1;
        let id = format!("fake-{}", d.id);
        self.held.lock().unwrap().insert(id.clone());
        Ok(id)
    }
    async fn progress(&self, id: &str) -> Result<Progress, BackendError> {
        if !self.held.lock().unwrap().contains(id) {
            return Err(BackendError::Unknown(id.into()));
        }
        Ok(Progress {
            done: 10,
            total: 100,
            ..Progress::default()
        })
    }
    async fn pause(&self, _id: &str) -> Result<(), BackendError> {
        Ok(())
    }
    async fn resume(&self, _id: &str) -> Result<(), BackendError> {
        Ok(())
    }
    async fn remove(&self, id: &str, _delete_files: bool) -> Result<(), BackendError> {
        self.held.lock().unwrap().remove(id);
        Ok(())
    }
    async fn video_file(&self, _id: &str, _file: Option<usize>) -> Result<PathBuf, BackendError> {
        Err(BackendError::Unknown("never finishes".into()))
    }
}

fn no_probe(_: &Path) -> Option<Probe> {
    None
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cancel_during_a_tick_stays_cancelled_and_is_not_added_again() {
    let db = Db::in_memory().await.unwrap();
    let backend = Arc::new(Stalling::default());
    let dir = tempfile::TempDir::new().unwrap();
    let m = Arc::new(Manager::new(
        db.clone(),
        vec![backend.clone()],
        dir.path().join("Downloads"),
        no_probe as Prober,
    ));
    let id = DownloadRepository::new(&db)
        .add(&NewDownload {
            title: "Heat",
            source: "torrent",
            torrent: Some(b"not read by the fake backend"),
            backend: "librqbit",
            ..Default::default()
        })
        .await
        .unwrap();

    // The first tick starts it.
    m.tick(NOW).await.unwrap();
    let d = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    assert_eq!(d.state, DownloadState::Active);
    assert_eq!(*backend.added.lock().unwrap(), 1);

    // The next tick stalls inside the backend; the viewer cancels meanwhile.
    *backend.stall.lock().unwrap() = true;
    let ticking = tokio::spawn({
        let m = m.clone();
        async move { m.tick(NOW).await.unwrap() }
    });
    backend.entered.notified().await;
    let mut cancelling = tokio::spawn({
        let m = m.clone();
        async move { m.cancel(id, true).await.unwrap() }
    });
    // Give a cancel that does not wait for the pass the time to finish.
    let _ = tokio::time::timeout(Duration::from_millis(200), &mut cancelling).await;
    backend.release.notify_one();
    ticking.await.unwrap();
    if !cancelling.is_finished() {
        cancelling.await.unwrap();
    }

    // And the ticks after it leave it alone.
    for _ in 0..3 {
        m.tick(NOW).await.unwrap();
    }
    let d = DownloadRepository::new(&db).get(id).await.unwrap().unwrap();
    assert_eq!(d.state, DownloadState::Cancelled, "{d:?}");
    assert!(
        backend.held.lock().unwrap().is_empty(),
        "the backend still fetches a cancelled download"
    );
    assert_eq!(
        *backend.added.lock().unwrap(),
        1,
        "a cancelled download was added again"
    );
}
