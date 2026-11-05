//! The resolver: one query, every backend at once, each on its own clock.
//!
//! Phase 6 E4 — "one slow backend never delays results from the others" — decides the
//! shape. Waiting for every backend and then answering would let the slowest set the
//! pace for all of them, so instead:
//!
//! - each backend runs in its OWN task, under its own timeout;
//! - each sends its batch the moment it has one, down a channel ([`Resolver::find`]);
//! - [`Resolver::find_all`] is the same thing, collected, for callers that want one list.
//!
//! A backend runs inside `tokio::spawn`, so a backend that PANICS becomes a failed
//! outcome for that backend, not a crash of the app.

use std::sync::Arc;
use std::time::Duration;

use source_protocol::{MediaQuery, PlayableStream, SourceBackend, SourceCandidate, SourceError};
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};

use crate::merge::Merged;

/// A backend may not flood the app: past this many candidates, the rest are dropped.
pub const MAX_CANDIDATES_PER_BACKEND: usize = 200;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);

/// One backend, as the resolver holds it.
#[derive(Clone)]
pub struct Entry {
    pub backend: Arc<dyn SourceBackend>,
    pub timeout: Duration,
}

impl Entry {
    pub fn new(backend: Arc<dyn SourceBackend>) -> Self {
        Self {
            backend,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
}

/// What happened when one backend was asked. The registry turns these into health.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub backend: String,
    pub elapsed: Duration,
    pub result: Result<usize, SourceError>,
}

/// One message from [`Resolver::find`]: a backend finished, one way or another.
#[derive(Debug, Clone)]
pub struct Batch {
    pub outcome: Outcome,
    /// Empty unless the backend succeeded. Provenance is already stamped.
    pub candidates: Vec<SourceCandidate>,
}

#[derive(Debug, Default)]
pub struct Found {
    pub candidates: Vec<SourceCandidate>,
    pub outcomes: Vec<Outcome>,
}

/// Holds the backends to ask, in the user's order, already filtered to the enabled
/// ones. Which ones those are is the registry's business, not the resolver's.
#[derive(Clone, Default)]
pub struct Resolver {
    entries: Vec<Entry>,
    offline: bool,
}

impl Resolver {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            offline: false,
        }
    }

    /// The backends that will be asked, in the order they were given.
    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.backend.id()).collect()
    }

    /// With no network, backends that need one are not asked at all — they would only
    /// time out, and a local file should not wait eight seconds to appear.
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    fn eligible(&self, query: &MediaQuery) -> Vec<Entry> {
        self.entries
            .iter()
            .filter(|e| {
                let caps = e.backend.capabilities();
                caps.handles(query.kind) && !(self.offline && caps.network)
            })
            .cloned()
            .collect()
    }

    /// Ask every eligible backend at once. Each sends one [`Batch`] when it finishes,
    /// succeeds, fails or times out — so the receiver gets exactly one message per
    /// backend asked, in the order they FINISHED, and the channel closes after the last.
    pub fn find(&self, query: &MediaQuery) -> mpsc::Receiver<Batch> {
        let entries = self.eligible(query);
        let (tx, rx) = mpsc::channel(entries.len().max(1));
        for entry in entries {
            let (tx, query) = (tx.clone(), query.clone());
            tokio::spawn(async move {
                let batch = ask(entry, query).await;
                // The receiver may have stopped listening; that is its right.
                let _ = tx.send(batch).await;
            });
        }
        rx
    }

    /// Everything, merged. Returns once every backend has answered or run out of time,
    /// so it takes as long as the longest TIMEOUT, never longer.
    pub async fn find_all(&self, query: &MediaQuery) -> Found {
        let mut rx = self.find(query);
        let (mut merged, mut outcomes) = (Merged::default(), Vec::new());
        while let Some(batch) = rx.recv().await {
            merged.add(batch.candidates);
            outcomes.push(batch.outcome);
        }
        Found {
            candidates: merged.into_candidates(),
            outcomes,
        }
    }

    /// Turn a candidate into a stream, through the backend that found it.
    pub async fn resolve(
        &self,
        candidate: &SourceCandidate,
    ) -> Result<PlayableStream, SourceError> {
        let id = &candidate.provenance.backend;
        let entry = self
            .entries
            .iter()
            .find(|e| e.backend.id() == id)
            .ok_or_else(|| SourceError::NotFound(format!("no enabled backend called {id}")))?;
        let (backend, candidate) = (entry.backend.clone(), candidate.clone());
        isolated(
            entry.timeout,
            async move { backend.resolve(&candidate).await },
        )
        .await
    }
}

/// One backend, one query, bounded in time, and isolated from panics.
async fn ask(entry: Entry, query: MediaQuery) -> Batch {
    let id = entry.backend.id().to_string();
    let start = Instant::now();
    let backend = entry.backend.clone();
    let result = isolated(entry.timeout, async move { backend.find(&query).await }).await;
    let elapsed = start.elapsed();
    match result {
        Ok(mut found) => {
            found.truncate(MAX_CANDIDATES_PER_BACKEND);
            for c in &mut found {
                // A backend does not get to say who it is; the resolver knows.
                c.provenance.backend = id.clone();
                c.provenance.also_from.clear();
            }
            Batch {
                outcome: Outcome {
                    backend: id,
                    elapsed,
                    result: Ok(found.len()),
                },
                candidates: found,
            }
        }
        Err(e) => {
            tracing::debug!(backend = %id, error = %e, "backend failed");
            Batch {
                outcome: Outcome {
                    backend: id,
                    elapsed,
                    result: Err(e),
                },
                candidates: Vec::new(),
            }
        }
    }
}

/// Run one backend call in a task of its own, under a deadline.
///
/// The task is what turns a backend's panic into an error at the join instead of a
/// crash. And on timeout the task is ABORTED, not merely abandoned: a dropped
/// `JoinHandle` leaves its task running, so a hung backend would otherwise keep a
/// connection and a thread's worth of work alive after everyone stopped waiting.
async fn isolated<T: Send + 'static>(
    limit: Duration,
    work: impl std::future::Future<Output = Result<T, SourceError>> + Send + 'static,
) -> Result<T, SourceError> {
    let mut task = tokio::spawn(work);
    match timeout(limit, &mut task).await {
        Ok(Ok(r)) => r,
        Ok(Err(join)) => Err(SourceError::Unavailable(if join.is_panic() {
            "the backend crashed".into()
        } else {
            "the backend was cancelled".into()
        })),
        Err(_) => {
            task.abort();
            Err(SourceError::Timeout(limit.as_millis() as u64))
        }
    }
}
