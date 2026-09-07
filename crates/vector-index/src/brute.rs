//! The exact answer, computed the slow way — the thing the index is measured against.
//!
//! An approximate index is only as trustworthy as the number that says how approximate
//! it is, and that number cannot come from the index itself. So this module scans every
//! vector in the artefact and computes cosine directly. It is deliberately naive: no
//! graph, no pruning, no usearch. **usearch has its own `exact_search`, and using it
//! would have been the mistake** — it would compare usearch against usearch, over the
//! vectors usearch stored, through the metric usearch implemented. If the keys were
//! mapped wrongly at build time, or the quantisation were misread, that comparison
//! would agree with itself and report perfect recall.
//!
//! This one starts from the artefact bytes and the catalogue ids, which is the only
//! place both halves can be wrong independently.

use sinephile_embedding::Artefact;

use crate::Neighbour;

/// Cosine similarity of two int8 vectors: `+1.0` identical direction, `0.0` unrelated,
/// `-1.0` opposite.
///
/// Accumulated in `i64` rather than `f32`. The dot product of two 384-dimensional int8
/// vectors peaks at 384 × 127 × 127 ≈ 6.2 million, which fits an `i32` but loses exact
/// integer representation in `f32` past 16,777,216 — so the wide accumulator costs
/// nothing and removes the question entirely.
pub fn cosine_i8(a: &[i8], b: &[i8]) -> f32 {
    let mut dot: i64 = 0;
    let mut norm_a: i64 = 0;
    let mut norm_b: i64 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        let (x, y) = (*x as i64, *y as i64);
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0 || norm_b == 0 {
        // An all-zero vector has no direction. It is not "maximally similar to
        // everything", which is what 0/0 would eventually be read as.
        return 0.0;
    }
    dot as f32 / ((norm_a as f32).sqrt() * (norm_b as f32).sqrt())
}

/// The true `k` nearest items, by scanning the whole artefact.
///
/// `ids[n]` is the catalogue id of artefact position `n`, exactly as
/// [`crate::VectorIndex::build`] takes it — **including the `None` holes**, which are
/// positions the index deliberately does not hold. Passing a compacted list instead
/// would silently shift every position onto the wrong vector, and the recall number
/// would still look plausible.
///
/// Distance is `1 - cosine`, matching usearch's `Cos` convention so the two are directly
/// comparable.
///
/// Ties break on `media_item_id`, so a query with duplicate vectors — which a catalogue
/// with two identical documents genuinely has — produces the same answer every run.
pub fn nearest(artefact: &Artefact, ids: &[Option<i64>], query: &[i8], k: usize) -> Vec<Neighbour> {
    let mut scored: Vec<Neighbour> = Vec::with_capacity(ids.len());
    for (position, id) in ids.iter().enumerate() {
        let Some(id) = *id else { continue };
        let Some(vector) = artefact.vector(position as u64) else {
            break;
        };
        scored.push(Neighbour {
            media_item_id: id,
            distance: 1.0 - cosine_i8(query, vector),
        });
    }

    scored.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.media_item_id.cmp(&b.media_item_id))
    });
    scored.truncate(k);
    scored
}

/// What fraction of the true neighbours the index actually found.
///
/// Recall, not rank agreement: an index that returns the right ten in a different order
/// has lost nothing, because whatever ranks the results afterwards — Phase 5's rank
/// fusion — re-orders them anyway. Measured against `truth.len()`, so asking for ten
/// and being handed nine at all is already a 10% loss.
pub fn recall(truth: &[Neighbour], found: &[Neighbour]) -> f64 {
    if truth.is_empty() {
        return 1.0;
    }
    let hits = truth
        .iter()
        .filter(|t| found.iter().any(|f| f.media_item_id == t.media_item_id))
        .count();
    hits as f64 / truth.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinephile_embedding::{artefact, Header, Quantisation};

    fn artefact_of(vectors: Vec<Vec<i8>>) -> Artefact {
        let header = Header {
            model: "all-MiniLM-L6-v2-int8".into(),
            dimension: vectors[0].len() as u16,
            quantisation: Quantisation::Int8,
            document_builder_version: 1,
            snapshot_date: "2026-09-06".into(),
            text_source: "wikipedia".into(),
            count: vectors.len() as u64,
        };
        let mut bytes = Vec::new();
        artefact::write(&mut bytes, &header, vectors).expect("write");
        Artefact::read(&mut bytes.as_slice()).expect("read")
    }

    #[test]
    fn cosine_ignores_magnitude_and_notices_direction() {
        assert!(
            (cosine_i8(&[10, 0], &[100, 0]) - 1.0).abs() < 1e-5,
            "same ray"
        );
        assert!(cosine_i8(&[10, 0], &[0, 10]).abs() < 1e-5, "orthogonal");
        assert!(
            (cosine_i8(&[10, 0], &[-50, 0]) + 1.0).abs() < 1e-5,
            "opposed"
        );
        assert_eq!(
            cosine_i8(&[0, 0], &[1, 1]),
            0.0,
            "no direction, no similarity"
        );
    }

    #[test]
    fn the_nearest_neighbours_are_the_ones_pointing_the_same_way() {
        let artefact = artefact_of(vec![
            vec![100, 0],  // id 11 — the query itself
            vec![90, 40],  // id 22 — close
            vec![0, 100],  // id 33 — orthogonal
            vec![-100, 0], // id 44 — opposite
        ]);
        let ids = vec![Some(11), Some(22), Some(33), Some(44)];

        let hits = nearest(&artefact, &ids, &[100, 0], 4);
        assert_eq!(
            hits.iter().map(|h| h.media_item_id).collect::<Vec<_>>(),
            vec![11, 22, 33, 44]
        );
        assert!(hits[0].distance < 1e-5, "{:?}", hits[0]);
        assert!((hits[3].distance - 2.0).abs() < 1e-5, "{:?}", hits[3]);
    }

    #[test]
    fn identical_vectors_are_ordered_by_id_so_a_rerun_agrees_with_itself() {
        let artefact = artefact_of(vec![vec![50, 50], vec![50, 50], vec![50, 50]]);
        let hits = nearest(&artefact, &[Some(70), Some(30), Some(50)], &[50, 50], 3);
        assert_eq!(
            hits.iter().map(|h| h.media_item_id).collect::<Vec<_>>(),
            vec![30, 50, 70]
        );
    }

    #[test]
    fn a_position_with_no_id_is_skipped_without_shifting_the_rest() {
        // The index does not hold items with no descriptive text, so neither may the
        // ground truth — and the holes must stay in place. If `None` shifted the
        // positions instead of skipping them, every vector after the first hole would be
        // attributed to the wrong film and recall would still look plausible.
        let artefact = artefact_of(vec![
            vec![100, 0], // position 0 — id 11
            vec![90, 40], // position 1 — NOT indexed
            vec![0, 100], // position 2 — id 33
        ]);
        let hits = nearest(&artefact, &[Some(11), None, Some(33)], &[0, 100], 3);
        assert_eq!(hits.len(), 2, "the hole is skipped, not filled");
        assert_eq!(
            hits[0].media_item_id, 33,
            "and position 2 is still position 2"
        );
        assert!(hits[0].distance < 1e-5, "{:?}", hits[0]);
    }

    #[test]
    fn recall_counts_the_truth_that_was_found() {
        let truth: Vec<Neighbour> = [1i64, 2, 3, 4]
            .iter()
            .map(|id| Neighbour {
                media_item_id: *id,
                distance: 0.0,
            })
            .collect();
        let found: Vec<Neighbour> = [4i64, 1, 9]
            .iter()
            .map(|id| Neighbour {
                media_item_id: *id,
                distance: 0.0,
            })
            .collect();

        assert_eq!(recall(&truth, &found), 0.5, "two of four, order irrelevant");
        assert_eq!(recall(&truth, &truth), 1.0);
        assert_eq!(recall(&truth, &[]), 0.0);
    }
}
