//! Automatic fallback — SPEC.md Phase 9: "if the top candidate fails to start within a
//! timeout, silently try the next, and log why."
//!
//! [`start`] opens the plan's choice and watches it. A copy that fails outright (mpv
//! reports the error at once: a 404, a refused connection, a file that is not a video)
//! is left immediately. A copy that never loads is left after [`Patience::load`]. A copy
//! that has loaded, whose bytes are arriving, is given [`Patience::frame`] more: slow is
//! not broken, and the Internet Archive takes up to 8 s to a first frame (Phase 8 E3).
//! Each next copy is resolved only when it is needed, so a swarm nobody plays is never
//! joined.
//!
//! The player is behind [`Screen`] so the loop is tested with a fake one, and the app's
//! implementation can say when a newer Play has made this one stale.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sinephile_sources::Resolver;
use sinephile_torrent::{Engine, StreamServer};
use source_protocol::SourceCandidate;

/// What the loop needs from a player. Every call blocks; the loop runs them off the
/// async threads.
pub trait Screen: Send + Sync + 'static {
    fn open(&self, target: &str, start: Option<f64>) -> Result<(), String>;
    /// Until the file has loaded, or it failed, or `timeout`.
    fn wait_loaded(&self, timeout: Duration) -> Result<(), String>;
    /// Until the first frame, or it failed, or `timeout`.
    fn wait_first_frame(&self, timeout: Duration) -> Result<(), String>;
    /// A newer Play has replaced this one: stop, and open nothing more.
    fn superseded(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patience {
    /// To load (headers read, format known). Nothing in that time means nothing is coming.
    pub load: Duration,
    /// From loaded to the first frame, while bytes arrive.
    pub frame: Duration,
}

impl Default for Patience {
    /// 8 s to load keeps a failing first choice plus the switch inside Phase 9 E4's 10 s.
    fn default() -> Self {
        Self {
            load: Duration::from_secs(8),
            frame: Duration::from_secs(30),
        }
    }
}

/// One copy that did not start, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub backend: String,
    pub reason: String,
}

/// What is playing, and what was passed over on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Started {
    pub open: String,
    pub backend: String,
    pub failed: Vec<Attempt>,
    /// From the first open to the first frame of what plays.
    pub after: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FallbackError {
    #[error("a newer choice replaced this one")]
    Superseded,
    #[error("no copy would start: {}", describe(.0))]
    Exhausted(Vec<Attempt>),
}

fn describe(failed: &[Attempt]) -> String {
    failed
        .iter()
        .map(|a| format!("{}: {}", a.backend, a.reason))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Open `first` (already resolved by the plan, from `backend`), then each of `rest` in
/// order until one shows a frame.
pub async fn start<S: Screen>(
    screen: Arc<S>,
    first: (String, String),
    rest: Vec<SourceCandidate>,
    resolver: &Resolver,
    torrent: Option<(&Arc<Engine>, &StreamServer)>,
    start_at: Option<f64>,
    patience: Patience,
) -> Result<Started, FallbackError> {
    let t0 = Instant::now();
    let mut failed = Vec::new();
    let mut next = Some(first);
    let mut rest = rest.into_iter();
    loop {
        let Some((target, backend)) = next.take() else {
            return Err(FallbackError::Exhausted(failed));
        };
        if screen.superseded() {
            return Err(FallbackError::Superseded);
        }
        match try_one(screen.clone(), target.clone(), start_at, patience).await {
            Ok(()) => {
                return Ok(Started {
                    open: target,
                    backend,
                    failed,
                    after: t0.elapsed(),
                })
            }
            Err(reason) => {
                tracing::info!(%backend, %reason, "source did not start; trying the next");
                failed.push(Attempt { backend, reason });
            }
        }
        // The next copy that resolves.
        for candidate in rest.by_ref() {
            let backend = candidate.provenance.backend.clone();
            match crate::target_for(resolver, torrent, &candidate).await {
                Ok(target) => {
                    next = Some((target, backend));
                    break;
                }
                Err(reason) => failed.push(Attempt { backend, reason }),
            }
        }
    }
}

async fn try_one<S: Screen>(
    screen: Arc<S>,
    target: String,
    start_at: Option<f64>,
    patience: Patience,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        screen.open(&target, start_at)?;
        screen
            .wait_loaded(patience.load)
            .map_err(|e| format!("did not load: {e}"))?;
        screen
            .wait_first_frame(patience.frame)
            .map_err(|e| format!("no picture: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
