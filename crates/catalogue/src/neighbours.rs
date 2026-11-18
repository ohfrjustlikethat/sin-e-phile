//! The item-item matrix, built on this machine (SPEC.md Phase 17, subtask 17.1).
//!
//! MovieLens' ratings are streamed from the archive the user downloaded (ADR-0019: never
//! redistributed, and nothing derived from them is shipped), mapped onto catalogue titles
//! through the `external_ids` join that [`crate::movielens::load`] wrote, and turned into
//! each title's best neighbours by `sinephile_recommend::cf`, with the score chosen on
//! MovieLens' tuning users (`cf::CHOSEN`, docs/eval-results.md). The matrix replaces the
//! stored one whole; the ratings themselves are never stored.

use std::path::Path;
use std::time::Instant;

use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::NeighbourRepository;
use sinephile_persistence::Db;
use sinephile_recommend::cf::{self, Likes, Rating};

use crate::freshness::{self, Freshness};
use crate::job::JobError;
use crate::movielens::Release;

/// What the launch step found (ADR-0054).
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The matrix is built from the release GroupLens publishes now.
    Current,
    /// GroupLens could not be asked; the matrix that exists stays.
    Unreachable,
    /// A new release (or the first): fetched, joined, built.
    Built(Built),
    /// It did not finish; the next launch tries again. Never stops the rest of the app.
    Failed(String),
}

/// At every launch, after the catalogue: is the matrix built from MovieLens' newest
/// release? One HEAD request when it is. When GroupLens has published a new release, or
/// there is no matrix yet: fetch it (resumable), join it onto the catalogue, build, store,
/// remember the release's validator, and delete the archive (nothing reads it again until
/// the next release). Never an error: a launch must not fail on a dataset host.
///
/// `on_build` is called once it is known there is something to fetch and build, for the
/// setup bar; a launch with nothing to do shows nothing.
pub async fn keep_current(
    db: &Db,
    datasets: &Path,
    transport: &dyn Transport,
    on_build: &(dyn Fn() + Sync),
) -> Outcome {
    keep_current_from(db, datasets, transport, &Release::Latest.url(), on_build).await
}

/// [`keep_current`] with the release's address given: a test serves it from this machine.
pub async fn keep_current_from(
    db: &Db,
    datasets: &Path,
    transport: &dyn Transport,
    url: &str,
    on_build: &(dyn Fn() + Sync),
) -> Outcome {
    match current(db, datasets, transport, url, on_build).await {
        Ok(outcome) => outcome,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

async fn current(
    db: &Db,
    datasets: &Path,
    transport: &dyn Transport,
    url: &str,
    on_build: &(dyn Fn() + Sync),
) -> Result<Outcome, JobError> {
    let have = NeighbourRepository::new(db).titles().await? > 0;
    let validator = match freshness::check_url(db, transport, url).await {
        Freshness::UpToDate if have => return Ok(Outcome::Current),
        Freshness::Unknown if have => return Ok(Outcome::Unreachable),
        Freshness::Stale { validator } => validator,
        // Remembered but nothing stored, or never asked: build.
        Freshness::UpToDate | Freshness::Unknown => None,
    };
    // No download without the release's validator: without it the next launch could not
    // tell this release from a new one, and would fetch it again every time.
    let validator = match validator {
        Some(v) => v,
        None => match freshness::current_validator(transport, url).await {
            Some(v) => v,
            None => return Ok(Outcome::Unreachable),
        },
    };
    on_build();
    let started = Instant::now();
    let path = datasets.join(Release::Latest.filename());
    let fetched = crate::download::Downloader::new()
        .fetch(url, &path, |_| {})
        .await?
        .bytes;
    let built = async {
        crate::movielens::join(db, &path).await?;
        build(db, &path).await
    }
    .await;
    // Whatever happened, the archive goes: a corrupt one would fail the same way again,
    // and a good one is not read again until GroupLens publishes the next.
    let _ = std::fs::remove_file(&path);
    let built = Built {
        fetched,
        total_seconds: started.elapsed().as_secs_f64(),
        ..built?
    };
    freshness::remember_url(db, "movielens", url, &validator).await?;
    Ok(Outcome::Built(built))
}

/// Neighbours kept per title.
pub const TOP: usize = 50;

/// What a build did, for the log and for docs/eval-results.md.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub ratings: u64,
    pub likes: usize,
    pub people: usize,
    pub titles: usize,
    pub rows: u64,
    /// The build alone: reading the ratings to storing the rows.
    pub seconds: f64,
    /// Bytes downloaded for it; 0 when built from an archive already on disk.
    pub fetched: u64,
    /// The download and the join as well: what a launch spends.
    pub total_seconds: f64,
}

/// Build the matrix from the MovieLens `archive` and store it. Leaves one core free: it is
/// background work.
pub async fn build(db: &Db, archive: &Path) -> Result<Built, JobError> {
    let started = Instant::now();
    let titles = crate::movielens::by_movielens_id(db).await?;
    let path = archive.to_path_buf();
    let matrix = tokio::task::spawn_blocking(move || {
        let mut liked = Vec::new();
        let ratings = crate::movielens::ratings(&path, |user, movie, stars| {
            if stars < cf::LIKED {
                return;
            }
            // Only films we hold, by catalogue id; ids beyond u32 cannot occur (SQLite
            // rowids from 1, a catalogue of millions), but are skipped rather than wrapped.
            if let Some(item) = titles.get(&movie).and_then(|&id| u32::try_from(id).ok()) {
                liked.push(Rating { user, item, stars });
            }
        })?;
        let likes = Likes::from_ratings(liked, cf::MIN_LIKES);
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .saturating_sub(1)
            .max(1);
        let matrix = cf::build(&likes, cf::CHOSEN, TOP, threads);
        Ok::<_, JobError>((ratings, likes.likes(), likes.people(), matrix))
    })
    .await
    .map_err(|e| JobError::step("neighbours", e.to_string()))??;
    let (ratings, likes, people, matrix) = matrix;
    let rows: Vec<(i64, i64, f32)> = matrix
        .rows()
        .flat_map(|(item, row)| {
            row.iter()
                .map(move |&(neighbour, score)| (i64::from(item), i64::from(neighbour), score))
        })
        .collect();
    let written = NeighbourRepository::new(db).replace_all(&rows).await?;
    Ok(Built {
        ratings,
        likes,
        people,
        titles: matrix.items(),
        rows: written,
        seconds: started.elapsed().as_secs_f64(),
        fetched: 0,
        total_seconds: started.elapsed().as_secs_f64(),
    })
}
