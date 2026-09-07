//! HNSW over the embedding artefact: built once on the machine, memory-mapped after.
//!
//! The artefact ([`sinephile_embedding::Artefact`]) is 855,703 vectors in a flat file.
//! Finding the ten nearest to a query by reading all of them is a third of a gigabyte
//! of arithmetic per keystroke, which is not a search engine. This crate is the index
//! that makes it one, and [`brute`] is the exact answer the index is measured against.
//!
//! # Why the index is built here and not downloaded
//!
//! ADR-0014 publishes the **vectors**, not the graph. A downloaded HNSW would have to
//! be byte-compatible with the exact usearch version the user's build links, and it
//! would double the download for something the machine can derive in minutes. So the
//! artefact stays the artefact and the index is derived from it — which also means the
//! index can be rebuilt at any time from a file whose checksum is already verified.
//!
//! # Cosine on int8 is not an approximation of cosine on floats — it is the same number
//!
//! The quantiser scales each vector by its own peak (`sinephile_embedding::quantise`),
//! and the artefact stores the int8 values **without** that scale. Discarding it would
//! be fatal for a metric that cares about magnitude, and costs nothing for one that
//! does not: cosine divides by both magnitudes, so a positive per-vector factor cancels
//! top and bottom. `cos(k·a, m·b) == cos(a, b)` for any positive `k` and `m`.
//!
//! That is why the metric is [`MetricKind::Cos`] and why nothing is dequantised in the
//! hot path. Inner product would have been wrong — it keeps the magnitudes the artefact
//! threw away, and would rank by an accident of each title's peak component.
//!
//! # Why the keys are catalogue ids
//!
//! The artefact is *positional*: vector `n` belongs to the nth core title in id order,
//! and nothing in the file says which title that is. That coupling is a real hazard —
//! ingest one new core title and every position after it means a different film.
//!
//! The index ends the coupling rather than inheriting it. Positions are resolved to
//! `media_items.id` **once, at build time**, and stored as usearch keys, so a search
//! returns catalogue ids and no caller ever computes an offset. [`VectorIndex::build`]
//! refuses when the id list and the artefact disagree on length, which is the one
//! moment that mismatch is still visible.

use std::path::Path;
use std::time::Instant;

use sinephile_embedding::{Artefact, ArtefactError};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

pub mod brute;

/// Where the derived index lives inside the data directory.
///
/// Named after the model for the same reason the artefact is
/// ([`sinephile_embedding::artefact_path`]), and it is safe to delete: the graph is
/// derived, so `ingest vector-index` rebuilds it from the artefact.
pub fn index_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(format!(
        "vector-index-{}.usearch",
        sinephile_embedding::MODEL
    ))
}

/// Neighbours per node in the graph (HNSW's *M*).
///
/// 16 is usearch's own default and the value the HNSW paper reports as the knee for
/// mid-dimensional data. Higher means better recall and a bigger graph — and the graph
/// is what makes the index larger than the vectors it indexes, so this is the size dial
/// as much as the quality one.
const CONNECTIVITY: usize = 16;

/// How many candidates the builder keeps while inserting (*efConstruction*).
///
/// Paid once, at build time, and it is the cheapest recall in the design: raising it
/// costs minutes on one machine and nothing at query time, ever.
const EXPANSION_ADD: usize = 128;

/// How many candidates a search keeps (*ef*).
///
/// **The recall/latency dial**, and the one E1's 80 ms budget is spent against. Chosen
/// by sweeping it against brute force over the real 855,703-vector artefact
/// (`eval vector --ef N`, 2026-09-06), not by taking a library default:
///
/// | ef | recall@10 | p95 |
/// |---|---|---|
/// | 64 (the default) | 0.9400 | 2.1 ms |
/// | 128 | 0.9550 | 3.3 ms |
/// | **192** | **0.9670** | **4.3 ms** |
/// | 256 | 0.9750 | 5.0 ms |
/// | 384 | 0.9845 | 6.0 ms |
///
/// 192 is the smallest value that clears the 0.95 gate with real margin. Higher was
/// affordable here — 6 ms against an 80 ms budget — but that budget is a **Tier 0**
/// budget, and Tier 0 also has to pay for the query embedding (P8: 24–33 ms padded)
/// and the keyword pass out of the same 80 ms. The margin is left where it can be
/// spent knowingly rather than pre-committed to one component.
pub const EXPANSION_SEARCH: usize = 192;

#[derive(Debug, thiserror::Error)]
pub enum VectorIndexError {
    #[error(
        "the artefact holds {vectors} vectors but the catalogue offered {ids} core ids — \
         they must line up position for position, so either the artefact is stale or the \
         catalogue moved under it"
    )]
    CountMismatch { vectors: u64, ids: usize },
    #[error("catalogue id {0} cannot be an index key — keys are positive")]
    NonPositiveId(i64),
    #[error("the index holds {index}-dimensional vectors, the query has {query}")]
    Dimension { index: usize, query: usize },
    #[error("the artefact has no vector at position {0}")]
    MissingVector(u64),
    #[error("usearch: {0}")]
    Usearch(String),
    #[error(transparent)]
    Artefact(#[from] ArtefactError),
    #[error("{path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

impl From<cxx::Exception> for VectorIndexError {
    fn from(e: cxx::Exception) -> Self {
        VectorIndexError::Usearch(e.to_string())
    }
}

/// One neighbour, as the catalogue knows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbour {
    pub media_item_id: i64,
    /// Cosine **distance**: 0.0 is identical, 1.0 is unrelated, 2.0 is opposite. Lower
    /// is better, which matches the convention BM25 already set in the keyword half.
    pub distance: f32,
}

/// What a build cost — reported because ADR-0014's budget is a number, not a feeling.
#[derive(Debug, Clone, Copy)]
pub struct BuildReport {
    pub vectors: usize,
    pub bytes: u64,
    pub seconds: f64,
}

pub struct VectorIndex {
    index: Index,
}

impl VectorIndex {
    /// Build the graph from a verified artefact and save it.
    ///
    /// `ids` is the catalogue id of each vector **in artefact order** — the order
    /// `tools/ingest`'s embed job wrote them in. `progress` is called with a running
    /// count, because this is minutes of work and a silent minute looks like a hang.
    pub fn build(
        artefact: &Artefact,
        ids: &[i64],
        path: &Path,
        mut progress: impl FnMut(usize),
    ) -> Result<BuildReport, VectorIndexError> {
        if artefact.header.count != ids.len() as u64 {
            return Err(VectorIndexError::CountMismatch {
                vectors: artefact.header.count,
                ids: ids.len(),
            });
        }

        let started = Instant::now();
        let index = Index::new(&IndexOptions {
            dimensions: artefact.header.dimension as usize,
            metric: MetricKind::Cos,
            quantization: ScalarKind::I8,
            connectivity: CONNECTIVITY,
            expansion_add: EXPANSION_ADD,
            expansion_search: EXPANSION_SEARCH,
            multi: false,
        })?;
        // Reserved up front: usearch grows by reallocating, and 855,703 unplanned
        // growth steps is the difference between minutes and an afternoon.
        index.reserve(ids.len())?;

        for (position, id) in ids.iter().enumerate() {
            if *id <= 0 {
                return Err(VectorIndexError::NonPositiveId(*id));
            }
            let vector = artefact
                .vector(position as u64)
                .ok_or(VectorIndexError::MissingVector(position as u64))?;
            index.add(*id as u64, vector)?;

            if position % 10_000 == 0 {
                progress(position);
            }
        }
        progress(ids.len());

        let path_string = path.to_string_lossy().to_string();
        index.save(&path_string)?;

        let bytes = std::fs::metadata(path)
            .map_err(|source| VectorIndexError::Io {
                path: path_string,
                source,
            })?
            .len();

        Ok(BuildReport {
            vectors: ids.len(),
            bytes,
            seconds: started.elapsed().as_secs_f64(),
        })
    }

    /// Open a saved index **without reading it into memory**.
    ///
    /// This is the whole reason usearch won P11. `SPEC.md` §2.3 caps idle RAM at 250 MB
    /// on Tier 0, and Tier 0 is precisely the tier the artefact exists to serve
    /// (ADR-0015) — so an index that loads is an index that cannot ship. `view` maps the
    /// file and lets the operating system page in the parts a search touches, which for
    /// HNSW is a few thousand nodes out of 855,703.
    pub fn view(path: &Path) -> Result<Self, VectorIndexError> {
        let index = Index::restore_view(&path.to_string_lossy())?;
        index.change_expansion_search(EXPANSION_SEARCH);
        Ok(Self { index })
    }

    /// Open a saved index by reading all of it into memory.
    ///
    /// Only here to measure what [`VectorIndex::view`] saves. Nothing in the application
    /// should call it.
    pub fn load(path: &Path) -> Result<Self, VectorIndexError> {
        let index = Index::restore(&path.to_string_lossy())?;
        index.change_expansion_search(EXPANSION_SEARCH);
        Ok(Self { index })
    }

    /// Override the search dial for one open index.
    ///
    /// Exists so the harness can **sweep** it: [`EXPANSION_SEARCH`]'s value is a claim
    /// about a trade-off, and a constant nobody has measured either side of is a guess
    /// with a comment attached. `eval vector --ef N` is how the current value was
    /// chosen.
    pub fn set_expansion_search(&self, candidates: usize) {
        self.index.change_expansion_search(candidates);
    }

    /// The `k` nearest catalogue items to a quantised query vector.
    pub fn search(&self, query: &[i8], k: usize) -> Result<Vec<Neighbour>, VectorIndexError> {
        if query.len() != self.index.dimensions() {
            return Err(VectorIndexError::Dimension {
                index: self.index.dimensions(),
                query: query.len(),
            });
        }
        let matches = self.index.search(query, k)?;
        Ok(matches
            .keys
            .iter()
            .zip(matches.distances.iter())
            .map(|(key, distance)| Neighbour {
                media_item_id: *key as i64,
                distance: *distance,
            })
            .collect())
    }

    pub fn len(&self) -> usize {
        self.index.size()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dimensions(&self) -> usize {
        self.index.dimensions()
    }

    /// Bytes usearch believes it is holding. Compared between `view` and `load` in the
    /// harness, which is what makes the mmap claim above evidence rather than a
    /// library's marketing.
    pub fn memory_usage(&self) -> usize {
        self.index.memory_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinephile_embedding::{artefact, quantise, Header, Quantisation};

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

    /// Vectors spread around a circle in the first two dimensions, so nearest-neighbour
    /// order is known without running anything: neighbours by angle.
    fn circle(count: usize) -> Vec<Vec<i8>> {
        (0..count)
            .map(|i| {
                let angle = (i as f32) * std::f32::consts::TAU / count as f32;
                vec![
                    (angle.cos() * 120.0) as i8,
                    (angle.sin() * 120.0) as i8,
                    0,
                    0,
                ]
            })
            .collect()
    }

    #[test]
    fn an_index_round_trips_through_a_file_and_answers_by_catalogue_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.usearch");
        let vectors = circle(64);
        let artefact = artefact_of(vectors.clone());
        // Ids deliberately not 0..n: the whole point of the keys is that position and
        // catalogue id are different things.
        let ids: Vec<i64> = (0..64).map(|i| 5_000 + i * 7).collect();

        let report = VectorIndex::build(&artefact, &ids, &path, |_| {}).expect("build");
        assert_eq!(report.vectors, 64);
        assert!(report.bytes > 0, "an index that saved nothing is not saved");

        let index = VectorIndex::view(&path).expect("view");
        assert_eq!(index.len(), 64);
        assert_eq!(index.dimensions(), 4);

        // Querying with a stored vector must return that item first, and its neighbours
        // on the circle next — the id mapping and the metric checked in one assertion.
        let hits = index.search(&vectors[10], 3).expect("search");
        assert_eq!(hits[0].media_item_id, ids[10]);
        assert!(hits[0].distance < 1e-4, "{:?}", hits[0]);
        let neighbours = [ids[9], ids[11]];
        assert!(neighbours.contains(&hits[1].media_item_id), "{hits:?}");
        assert!(neighbours.contains(&hits[2].media_item_id), "{hits:?}");
    }

    #[test]
    fn a_stale_artefact_is_refused_rather_than_indexed_against_the_wrong_titles() {
        // The failure this check exists for: the catalogue gained a core title after the
        // artefact was built, so position 40,000 is now a different film. Silent if
        // unchecked, because every offset still reads a valid vector.
        let dir = tempfile::tempdir().expect("tempdir");
        let artefact = artefact_of(circle(16));
        let ids: Vec<i64> = (1..=17).collect();

        let err = VectorIndex::build(&artefact, &ids, &dir.path().join("i.usearch"), |_| {})
            .expect_err("must refuse");
        assert!(
            matches!(
                err,
                VectorIndexError::CountMismatch {
                    vectors: 16,
                    ids: 17
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn a_query_of_the_wrong_shape_is_refused_rather_than_answered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.usearch");
        let artefact = artefact_of(circle(8));
        VectorIndex::build(&artefact, &(1..=8).collect::<Vec<i64>>(), &path, |_| {})
            .expect("build");

        let index = VectorIndex::view(&path).expect("view");
        let err = index.search(&[1i8, 2, 3], 5).expect_err("must refuse");
        assert!(
            matches!(err, VectorIndexError::Dimension { index: 4, query: 3 }),
            "{err}"
        );
    }

    #[test]
    fn quantisation_does_not_change_the_cosine_ordering() {
        // The claim the whole metric choice rests on: the artefact throws away each
        // vector's scale, and cosine cannot tell. If this ever fails, the index is
        // ranking by an accident of quantisation and every relevance number is void.
        let a = vec![0.9f32, 0.1, 0.4, 0.05];
        let b = vec![0.85f32, 0.2, 0.35, 0.10];
        let c = vec![-0.7f32, 0.6, -0.2, 0.30];

        let float_ab = quantise::cosine(&a, &b);
        let float_ac = quantise::cosine(&a, &c);

        let qa = quantise::quantise(&a);
        let qb = quantise::quantise(&b);
        let qc = quantise::quantise(&c);
        let int_ab = brute::cosine_i8(&qa.values, &qb.values);
        let int_ac = brute::cosine_i8(&qa.values, &qc.values);

        assert!(float_ab > float_ac, "the float ordering is the premise");
        assert!(int_ab > int_ac, "int8 must agree: {int_ab} vs {int_ac}");
        assert!(
            (float_ab - int_ab).abs() < 0.01,
            "and agree closely: {float_ab} vs {int_ab}"
        );
    }
}
