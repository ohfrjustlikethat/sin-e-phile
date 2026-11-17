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

use sinephile_persistence::repositories::NeighbourRepository;
use sinephile_persistence::Db;
use sinephile_recommend::cf::{self, Likes, Rating};

use crate::job::JobError;

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
    pub seconds: f64,
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
    })
}
