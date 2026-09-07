//! Loading Wikipedia lead extracts into the catalogue (ADR-0033).
//!
//! Two steps:
//!
//! 1. **map** — walk IMDb-id prefixes through Wikidata, recording which catalogue items
//!    have an English article. Roughly an hour.
//! 2. **extracts** — fetch the lead text for mapped articles, most-voted first, and
//!    write it to `media_items.synopsis`. Hours.
//!
//! # Where the resumability actually is
//!
//! Not in the [`Job`] runner. `run_step` takes a higher-ranked closure over a
//! transaction and cannot hold an HTTP client across await points — the same wall
//! `embed.rs` hit — so the loops are here and progress lives in the data instead:
//!
//! - **map** is idempotent per id (`INSERT … ON CONFLICT DO UPDATE`), so a crash costs
//!   a re-query and writes nothing new. That is the cheap half.
//! - **extracts** records `wikipedia_article.fetched_at` per article as it goes, so a
//!   resumed run asks only for what is still missing. That is the half measured in
//!   hours, and it is the one that had to be right.
//!
//! # Why the mapping is chunked, and why the chunks subdivide themselves
//!
//! WDQS times out at 60 seconds and the unfiltered join needs 59 of them (measured
//! 2026-09-07: `COUNT(*)` returned 509,464 in 58.9 s). A prefix filter makes each query
//! small, but IMDb ids are not evenly distributed — `tt0` covers decades of cinema and
//! `tt3` a few years — so a fixed prefix length is either too coarse for the dense
//! ranges or wastefully fine for the sparse ones.
//!
//! So a chunk that fails **subdivides**: `tt0` becomes `tt00`…`tt09` and each is tried
//! again. The shape of the tree ends up matching the shape of the data, and no constant
//! has to encode a guess about IMDb's id allocation — the same lesson ADR-0032 taught
//! about assuming that dataset's ordering.

use sinephile_metadata_api::wikipedia::{Wikipedia, EXTRACTS_PER_REQUEST};
use sinephile_persistence::repositories::WikipediaRepository;
use sinephile_persistence::Db;

use crate::job::JobError;

/// How deep a prefix may get before the chunk is abandoned.
///
/// `tt` plus six digits is a million-id window; if that still will not answer, the
/// problem is not chunk size and subdividing further only multiplies failing requests.
const MAX_PREFIX: usize = 8;

/// Articles fetched per transaction. Each is an HTTP round trip, so this is about how
/// much work a crash discards, not about database cost.
const EXTRACT_BATCH: i64 = 200;

/// What a run did.
#[derive(Debug, Default, Clone, Copy)]
pub struct Loaded {
    pub queried: usize,
    pub mapped: usize,
    pub matched: usize,
    pub fetched: usize,
    pub empty: usize,
}

/// Step 1: which catalogue items have an English Wikipedia article.
///
/// Every mapping Wikidata returns is offered to the catalogue; the ones we do not hold
/// are counted and dropped. `matched` versus `mapped` is therefore the overlap between
/// Wikidata's 509,464 and this catalogue, which is a number worth reporting rather than
/// assuming.
pub async fn map(
    db: &Db,
    wiki: &Wikipedia<'_>,
    mut progress: impl FnMut(&str, &Loaded),
) -> Result<Loaded, JobError> {
    let repo = WikipediaRepository::new(db);
    let mut loaded = Loaded::default();

    // Depth-first over prefixes, so progress is monotonic and a resume can restart the
    // current chunk without redoing finished ones — `map` is idempotent per id, so a
    // repeated chunk costs a query and writes nothing new.
    let mut queue: Vec<String> = (0..10).map(|d| format!("tt{d}")).rev().collect();

    while let Some(prefix) = queue.pop() {
        loaded.queried += 1;
        match wiki.mappings(&prefix).await {
            Ok(mappings) => {
                for mapping in &mappings {
                    loaded.mapped += 1;
                    if repo
                        .map(&mapping.imdb_id, &mapping.article)
                        .await
                        .map_err(|e| JobError::step("wikipedia", e.to_string()))?
                    {
                        loaded.matched += 1;
                    }
                }
                progress(&prefix, &loaded);
            }
            Err(error) if prefix.len() < MAX_PREFIX => {
                // Too big, or the service refused. Either way the answer is smaller
                // questions — not a failed run.
                tracing::info!("{prefix} did not answer ({error}); subdividing");
                for digit in (0..10).rev() {
                    queue.push(format!("{prefix}{digit}"));
                }
            }
            Err(error) => {
                return Err(JobError::step(
                    "wikipedia",
                    format!("{prefix} failed at the maximum prefix depth: {error}"),
                ));
            }
        }
    }

    Ok(loaded)
}

/// Step 2: fetch lead extracts for mapped articles, most-voted first.
///
/// `limit` bounds the run. A full pass is hours, and the ordering means a bounded one
/// covers what people actually search for rather than an arbitrary slice.
pub async fn extracts(
    db: &Db,
    wiki: &Wikipedia<'_>,
    limit: i64,
    mut progress: impl FnMut(&Loaded),
) -> Result<Loaded, JobError> {
    let repo = WikipediaRepository::new(db);
    let mut loaded = Loaded::default();

    while (loaded.fetched + loaded.empty) < limit as usize {
        let remaining = limit - (loaded.fetched + loaded.empty) as i64;
        let batch = repo
            .pending(remaining.min(EXTRACT_BATCH))
            .await
            .map_err(|e| JobError::step("wikipedia", e.to_string()))?;
        if batch.is_empty() {
            break;
        }

        for group in batch.chunks(EXTRACTS_PER_REQUEST) {
            let titles: Vec<String> = group.iter().map(|p| p.article.clone()).collect();
            let found = wiki
                .extracts(&titles)
                .await
                .map_err(|e| JobError::step("wikipedia", e.to_string()))?;

            for pending in group {
                // Matched on what the extract ANSWERS, not on its current title:
                // `redirects=1` means a moved article comes back under its new name, and
                // pairing on the new name alone would record thousands of renamed films
                // as having no article.
                let extract = found
                    .iter()
                    .find(|e| e.answers().eq_ignore_ascii_case(&pending.article));

                match extract {
                    Some(extract) => {
                        repo.store(pending.media_item_id, &extract.text)
                            .await
                            .map_err(|e| JobError::step("wikipedia", e.to_string()))?;
                        loaded.fetched += 1;
                    }
                    None => {
                        // Marked rather than skipped: an unanswerable request is still a
                        // request, and leaving it pending means asking again on every
                        // future run, forever.
                        repo.mark_empty(pending.media_item_id)
                            .await
                            .map_err(|e| JobError::step("wikipedia", e.to_string()))?;
                        loaded.empty += 1;
                    }
                }
            }
            progress(&loaded);
        }
    }

    Ok(loaded)
}
