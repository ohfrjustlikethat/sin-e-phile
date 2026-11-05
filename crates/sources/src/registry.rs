//! Between the stored registry and the resolver: which backends to ask, in what
//! order, and what their answers did to their health.
//!
//! Here rather than in `src-tauri` because it is logic — "a backend the user switched
//! off is never asked" is a rule with a test, and `src-tauri` cannot run tests
//! (ADR-0022).

use std::sync::Arc;

use sinephile_persistence::repositories::{Sample, SourceRepository};
use sinephile_persistence::{Db, DbError};
use source_protocol::{SourceBackend, SourceError};

use crate::resolver::{Entry, Outcome, Resolver};

/// Build the resolver from what is available and what the user chose.
///
/// `available` is every backend the app could ask right now — the built-ins plus one
/// per installed addon. Each gets a registry row if it lacks one; then the enabled
/// rows, in the user's order, become the resolver's entries. A row with no matching
/// backend (an addon whose manifest failed to load) is skipped, not an error.
pub async fn resolver_from(
    db: &Db,
    available: Vec<Arc<dyn SourceBackend>>,
) -> Result<Resolver, DbError> {
    let repo = SourceRepository::new(db);
    let ids: Vec<&str> = available.iter().map(|b| b.id()).collect();
    repo.ensure(&ids).await?;
    let entries = repo
        .all()
        .await?
        .into_iter()
        .filter(|row| row.enabled)
        .filter_map(|row| {
            available
                .iter()
                .find(|b| b.id() == row.id)
                .map(|b| Entry::new(b.clone()))
        })
        .collect();
    Ok(Resolver::new(entries))
}

/// What an outcome means for health.
pub fn sample(outcome: &Outcome) -> Sample {
    match &outcome.result {
        Ok(_) => Sample::Answered {
            latency_ms: outcome.elapsed.as_secs_f64() * 1000.0,
        },
        Err(SourceError::Timeout(_)) => Sample::TimedOut,
        Err(e) => Sample::Failed {
            error: e.to_string(),
        },
    }
}

/// Write a search's outcomes back as health. Called after results are shown, so a
/// slow disk never delays them.
pub async fn record(db: &Db, outcomes: &[Outcome]) -> Result<(), DbError> {
    let repo = SourceRepository::new(db);
    for o in outcomes {
        repo.record(&o.backend, &sample(o)).await?;
    }
    Ok(())
}
