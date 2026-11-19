//! Countries and movements for the catalogue (Phase 18, ADR-0055): where each title comes
//! from and which film movement it belongs to, asked of Wikidata (P495 and P135), so the
//! discovery engine can name a blind spot ("Iranian New Wave") or a stretch ("but from
//! Japan"). The IMDb datasets carry neither.
//!
//! # In order, each run
//!
//! 1. **What profiles have met** and was never asked ([`RegionRepository::awaiting_met`]),
//!    whatever its votes: "you have never watched anything from Iran" may be said only when
//!    every title the profile met was asked.
//! 2. **The most voted** never asked, from [`MIN_VOTES`] ([`RegionRepository::awaiting`]):
//!    the acclaimed and widely seen are what the arms choose from. About 57,000 films in the
//!    development catalogue.
//!
//! [`PER_RUN`] titles a run, [`REGIONS_PER_REQUEST`] a query, at WDQS's one query in three
//! seconds: about three minutes of the hourly check's background, so every film with 1,000
//! votes is asked within the first five hours the app is open. A title Wikidata does not
//! know, or says nothing of, is recorded as asked with nothing, and not asked again.

use std::collections::HashMap;
use std::sync::Arc;

use sinephile_metadata_api::wikipedia::{Wikipedia, REGIONS_PER_REQUEST};
use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::{ProfileRepository, RegionRepository, Regions};
use sinephile_persistence::Db;

use crate::job::JobError;

/// Titles asked per run.
pub const PER_RUN: usize = 12_000;

/// The fewest votes a title needs to be asked about unless a profile has met it.
pub const MIN_VOTES: i64 = 1_000;

/// Where a run records what it did (JSON, a [`Report`]); the setting's write time is the
/// "last updated" Settings shows.
pub const LAST_KEY: &str = "regions.last";

/// What one run did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    /// Titles asked about, answered or not.
    pub asked: usize,
    /// Of those, how many Wikidata gave a country, and a movement.
    pub countries: usize,
    pub movements: usize,
    /// False when Wikidata stopped answering part way: the next run resumes.
    pub finished: bool,
}

fn step(e: impl std::fmt::Display) -> JobError {
    JobError::step("regions", e.to_string())
}

/// Ask Wikidata about up to `limit` titles: what profiles have met first, then the most voted.
pub async fn ask(db: &Db, wiki: &Wikipedia<'_>, limit: usize) -> Result<Report, JobError> {
    let repo = RegionRepository::new(db);
    let mut waiting = repo.awaiting_met(limit as i64).await.map_err(step)?;
    if waiting.len() < limit {
        let more = repo
            .awaiting(MIN_VOTES, (limit - waiting.len()) as i64)
            .await
            .map_err(step)?;
        for w in more {
            if !waiting.iter().any(|(id, _)| *id == w.0) {
                waiting.push(w);
            }
        }
    }
    let mut report = Report {
        finished: true,
        ..Default::default()
    };
    for group in waiting.chunks(REGIONS_PER_REQUEST) {
        let imdb_ids: Vec<String> = group.iter().map(|(_, imdb)| imdb.clone()).collect();
        let placed = match wiki.regions_for(&imdb_ids).await {
            Ok(placed) => placed,
            Err(error) => {
                tracing::info!(%error, "regions: Wikidata stopped answering; the next run resumes");
                report.finished = false;
                break;
            }
        };
        let by_imdb: HashMap<&str, _> = placed.iter().map(|p| (p.imdb_id.as_str(), p)).collect();
        let answers: Vec<Regions> = group
            .iter()
            .map(|(id, imdb)| {
                let p = by_imdb.get(imdb.as_str());
                Regions {
                    media_item_id: *id,
                    countries: p.map(|p| p.countries.clone()).unwrap_or_default(),
                    movements: p.map(|p| p.movements.clone()).unwrap_or_default(),
                }
            })
            .collect();
        repo.record(&answers).await.map_err(step)?;
        report.asked += answers.len();
        report.countries += answers.iter().filter(|a| !a.countries.is_empty()).count();
        report.movements += answers.iter().filter(|a| !a.movements.is_empty()).count();
    }
    if report.asked > 0 {
        let record = format!(
            r#"{{"asked":{},"countries":{},"movements":{},"finished":{}}}"#,
            report.asked, report.countries, report.movements, report.finished
        );
        ProfileRepository::new(db)
            .set_setting(LAST_KEY, &record)
            .await
            .map_err(step)?;
    }
    Ok(report)
}

/// The hourly check's run: [`PER_RUN`] titles.
pub async fn at_launch(db: &Db, transport: Arc<dyn Transport>) -> Result<Report, JobError> {
    let wiki = Wikipedia::new(transport.as_ref()).await;
    ask(db, &wiki, PER_RUN).await
}
