//! Turning a downloaded artefact into a searchable index — and checking first that the
//! two still describe the same catalogue.
//!
//! # Why this exists at all
//!
//! ADR-0014 publishes **vectors**, not the graph over them: a downloaded HNSW would have
//! to be byte-compatible with the exact usearch build the user links, and it would double
//! a 313 MB download for something a machine derives in minutes. So the artefact is
//! downloaded and the index is derived here.
//!
//! That step was missing. Until 2026-09-22 the only code that built an index lived in
//! `tools/ingest`, a dev tool that does not ship, while the application only ever *viewed*
//! a prebuilt file. A user consented to 346 MB, downloaded it, and searched with BM25
//! alone while 313 MB of vectors sat unread on disk (D44). It worked on exactly one
//! machine — the author's, where the index had been built by hand.
//!
//! # The check that length could never do
//!
//! The artefact is positional: vector *n* is the nth core title in id order, and nothing
//! in the file says which title that is. `VectorIndex::build` refuses a catalogue with
//! *fewer* ids than the artefact has vectors, and permits more, because the catalogue
//! refreshes itself (ADR-0030) and grows at the tail.
//!
//! Growing at the tail is not guaranteed, only true. `media_items.in_core` is a function
//! of vote count (`imdb::Scope::in_core`), and the ratings refresh updates `rating_votes`
//! **without** recomputing membership — so today no existing row is ever promoted, and
//! the artefact is a strict prefix. The day that omission is fixed, a title crossing the
//! popularity threshold will enter the sequence in the *middle*, every position after it
//! will name a different film, and the count will merely go up. Length cannot tell that
//! from an append, and never could.
//!
//! So [`verify_prefix`] checks it by **content**: re-derive the document at a sampled
//! position, embed it, quantise it, and require the bytes to equal the artefact's own
//! vector there. It is the same technique `eval embed --report` uses for producer/query
//! drift, pointed at a different question.
//!
//! **The last position is the important sample.** An insertion anywhere before the end
//! shifts what sits at the final position, so checking it alone catches every
//! mid-sequence insertion. The rest are defence in depth and cost one inference each.

use std::path::Path;

use sinephile_embedding::{quantise, Artefact};
use sinephile_persistence::repositories::CatalogueRepository;
use sinephile_persistence::Db;
use sinephile_vector_index::VectorIndex;

use crate::embed::DocumentEmbedder;
use crate::job::JobError;

/// Positions re-embedded before the artefact is trusted.
///
/// Eight, always including the first and the last. The last is the one that does the
/// work; the others narrow down *where* a mismatch starts, which is the difference
/// between "this artefact is stale" and a bug report nobody can act on.
const VERIFY_SAMPLES: usize = 8;

/// What a build did.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// Vectors actually added to the graph.
    pub indexed: usize,
    /// Positions offered — the artefact's whole range, including titles with too little
    /// text to embed well. The gap between this and `indexed` is the point.
    pub considered: usize,
    /// Core titles the catalogue holds beyond the artefact's reach — searchable by BM25
    /// and by exact title, but not by meaning until the next artefact.
    pub beyond_artefact: usize,
    pub bytes: u64,
    pub seconds: f64,
    /// Where it was written.
    pub path: std::path::PathBuf,
}

/// Confirm the artefact still describes the head of this catalogue.
///
/// Returns the position of the first disagreement, as an error. See the module note for
/// why a sampled content check is the right shape and a length check is not.
pub async fn verify_prefix(
    db: &Db,
    embedder: &mut (dyn DocumentEmbedder + Send),
    artefact: &Artefact,
    ids: &[i64],
) -> Result<(), JobError> {
    if artefact.header.model != embedder.identity() {
        return Err(JobError::step(
            "vector-index",
            format!(
                "the artefact was built with {} and this build embeds with {} — \
                 the two are different spaces and comparing them is meaningless",
                artefact.header.model,
                embedder.identity()
            ),
        ));
    }

    let count = artefact.header.count as usize;
    if ids.len() < count {
        return Err(JobError::step(
            "vector-index",
            format!(
                "the artefact holds {count} vectors and the catalogue offers only {} \
                 core ids — titles have been removed and positions have shifted",
                ids.len()
            ),
        ));
    }

    for position in sample_positions(count, VERIFY_SAMPLES) {
        let id = ids[position];
        let Some(document) = crate::embed::document_for(db, id).await? else {
            // The id came from the core-tier query a moment ago, so its absence now means
            // the two disagree about what the core tier IS — which is the failure this
            // whole function is looking for.
            return Err(JobError::step(
                "vector-index",
                format!("position {position} maps to id {id}, which is not in the core tier"),
            ));
        };

        let fresh = quantise::quantise(&embedder.embed(&document)?);
        let stored = artefact.vector(position as u64).ok_or_else(|| {
            JobError::step(
                "vector-index",
                format!("the artefact has no vector at position {position}"),
            )
        })?;

        if fresh.values != stored {
            return Err(JobError::step(
                "vector-index",
                format!(
                    "position {position} (catalogue id {id}) does not match the artefact. \
                     A title has entered or left the core tier ahead of this position, so \
                     every vector after it belongs to a different film. Rebuild the \
                     artefact, or fetch one built from this catalogue."
                ),
            ));
        }
    }
    Ok(())
}

/// Evenly spaced positions, always including the first and the last.
fn sample_positions(count: usize, samples: usize) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    if count <= samples {
        return (0..count).collect();
    }
    let last = count - 1;
    let mut out: Vec<usize> = (0..samples)
        .map(|i| i * last / (samples - 1).max(1))
        .collect();
    out.dedup();
    out
}

/// Verify, then build the index and put it where the application looks for it.
///
/// `progress` is called with `(added, total)`, because this is minutes of work and a
/// silent minute looks like a hang. It carries the total rather than leaving the caller
/// to guess: the denominator is the number of titles with enough text to embed, which
/// only this function is in a position to know.
pub async fn build(
    db: &Db,
    embedder: &mut (dyn DocumentEmbedder + Send),
    data_dir: &Path,
    mut progress: impl FnMut(usize, usize),
) -> Result<Built, JobError> {
    let artefact_path = sinephile_embedding::artefact_path(data_dir);
    let mut file = std::fs::File::open(&artefact_path)?;
    let artefact =
        Artefact::read(&mut file).map_err(|e| JobError::step("vector-index", e.to_string()))?;

    let repository = CatalogueRepository::new(db);
    let all = repository.core_ids().await?;
    verify_prefix(db, embedder, &artefact, &all).await?;

    // Positions line up with `core_ids` because both order by id over the same predicate;
    // `None` marks a title with too little text to embed well, which is skipped WITHOUT
    // shifting anything after it.
    let ids = repository.core_ids_for_vectors().await?;
    let beyond_artefact = ids.len().saturating_sub(artefact.header.count as usize);

    let final_path = sinephile_vector_index::index_path(data_dir);
    let building = final_path.with_extension("usearch.part");

    // Built under a .part name and renamed, so a killed build never leaves a file that
    // `VectorIndex::view` would happily open and search against half a graph.
    let indexable = ids
        .iter()
        .take(artefact.header.count as usize)
        .filter(|id| id.is_some())
        .count();
    let report = VectorIndex::build(&artefact, &ids, &building, |done| progress(done, indexable))
        .map_err(|e| JobError::step("vector-index", e.to_string()))?;
    std::fs::rename(&building, &final_path)?;

    Ok(Built {
        indexed: report.vectors,
        considered: report.considered,
        beyond_artefact,
        bytes: report.bytes,
        seconds: report.seconds,
        path: final_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_samples_always_include_the_first_and_the_last() {
        // The last is the one that catches a mid-sequence insertion: anything inserted
        // before the end shifts what sits at the end.
        for count in [1usize, 2, 7, 8, 9, 1_000, 855_703] {
            let positions = sample_positions(count, VERIFY_SAMPLES);
            assert_eq!(positions[0], 0, "count {count}");
            assert_eq!(
                *positions.last().expect("non-empty"),
                count - 1,
                "count {count}"
            );
            assert!(positions.len() <= VERIFY_SAMPLES, "count {count}");
            assert!(
                positions.windows(2).all(|w| w[0] < w[1]),
                "not ascending at count {count}: {positions:?}"
            );
        }
    }

    #[test]
    fn an_empty_catalogue_samples_nothing_rather_than_panicking() {
        assert!(sample_positions(0, VERIFY_SAMPLES).is_empty());
    }
}
