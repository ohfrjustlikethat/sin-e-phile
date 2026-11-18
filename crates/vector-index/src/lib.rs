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
//! returns catalogue ids and no caller ever computes an offset.
//!
//! Build time is the one moment that mapping is still checkable, and it is checked in
//! two places because one check cannot do it. [`VectorIndex::build`] refuses a catalogue
//! with **fewer** ids than the artefact has vectors — that means titles were removed and
//! everything after the gap has shifted. It permits **more**, because the catalogue
//! refreshes itself (ADR-0030) and the artefact is a published snapshot, so the artefact
//! is a prefix of the catalogue by construction and the surplus is simply not in the
//! graph yet.
//!
//! What length cannot see — and never could — is a title promoted into the core tier in
//! the *middle* of the sequence: positions shift and the count still goes up.
//! `sinephile_catalogue::index::verify_prefix` catches that by re-deriving documents at
//! sampled positions and requiring them to quantise to the artefact's own bytes.
//!
//! # The companion: what the catalogue gained since the published file
//!
//! The published graph is mapped read-only and never changes. A title the catalogue gains
//! later gets its vector at launch (ADR-0054) and goes into a second, small graph beside
//! it ([`Companion`], [`added_path`]), held in memory: tens to a few thousand vectors a
//! launch, against the published graph's hundred thousand. [`VectorIndex::open`] reads
//! both and a search merges them by distance, so a new title is found by meaning the
//! launch it gains a synopsis.

use std::path::{Path, PathBuf};
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

/// Where the titles added since the published file keep their vectors (ADR-0054): beside
/// the published graph, named after it (`vector-index-<model>.added.usearch`).
pub fn added_path(index_path: &Path) -> PathBuf {
    index_path.with_extension("added.usearch")
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
        "the artefact holds {vectors} vectors but the catalogue offered only {ids} core \
         ids — titles have been REMOVED, so every position after the first gap points at \
         a different title and nothing positional can be trusted"
    )]
    CatalogueShorterThanArtefact { vectors: u64, ids: usize },
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
    /// How many made it into the graph.
    pub vectors: usize,
    /// How many positions were offered — the whole catalogue, including the ones with
    /// nothing to say. The gap between the two is the point.
    pub considered: usize,
    pub bytes: u64,
    pub seconds: f64,
}

pub struct VectorIndex {
    index: Index,
    /// The companion graph ([`Companion`]), when [`VectorIndex::open`] found one.
    added: Option<Index>,
}

/// The one shape every graph here has: cosine over int8, the same dials.
fn options(dimensions: usize) -> IndexOptions {
    IndexOptions {
        dimensions,
        metric: MetricKind::Cos,
        quantization: ScalarKind::I8,
        connectivity: CONNECTIVITY,
        expansion_add: EXPANSION_ADD,
        expansion_search: EXPANSION_SEARCH,
        multi: false,
    }
}

/// A stored vector by catalogue id, from one graph.
fn stored(index: &Index, media_item_id: i64) -> Option<Vec<i8>> {
    let key = u64::try_from(media_item_id).ok()?;
    if !index.contains(key) {
        return None;
    }
    let mut out = vec![0i8; index.dimensions()];
    match index.get(key, &mut out) {
        Ok(n) if n > 0 => Some(out),
        _ => None,
    }
}

fn neighbours(index: &Index, query: &[i8], k: usize) -> Result<Vec<Neighbour>, VectorIndexError> {
    let matches = index.search(query, k)?;
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

impl VectorIndex {
    /// Build the graph from a verified artefact and save it.
    ///
    /// `ids` is the catalogue id of each vector **in artefact order** — the order
    /// `tools/ingest`'s embed job wrote them in — and **`None` means do not index this
    /// position**. `progress` is called with a running count, because this is minutes of
    /// work and a silent minute looks like a hang.
    ///
    /// # Why positions are skipped rather than the artefact being rebuilt smaller
    ///
    /// The artefact is published and positional: vector *n* is the nth core title, and
    /// every consumer depends on that. The index is derived locally, so it is the right
    /// place to be selective. Skipping keeps the two in step — the length check below
    /// still sees the full catalogue — while the graph holds only items worth searching
    /// semantically. See `CatalogueRepository::core_ids_for_vectors` for why that is not
    /// all of them.
    pub fn build(
        artefact: &Artefact,
        ids: &[Option<i64>],
        path: &Path,
        mut progress: impl FnMut(usize),
    ) -> Result<BuildReport, VectorIndexError> {
        // THE ARTEFACT IS A PREFIX OF THE CATALOGUE, NOT A COPY OF IT.
        //
        // This demanded equality until 2026-09-22, which was right when the catalogue
        // only changed during an ingest the author ran. Subtask 5.7 made it refresh
        // itself on every launch (ADR-0030), so it outgrows the published artefact
        // continuously — 2,467 titles in the first week — and equality meant the index
        // could never be built again on any machine that had been opened twice. That
        // made the whole 313 MB download inert (D44/D45).
        //
        // Extra ids at the TAIL are harmless: position n still names the same title, and
        // the new titles are simply not in the graph until the next artefact — BM25 and
        // the exact-title tier already cover them. FEWER ids is a different thing
        // entirely: something was removed, every position after the gap has shifted, and
        // no amount of care downstream can recover which title a vector belongs to.
        //
        // What this check CANNOT see is a title promoted into the core tier in the
        // MIDDLE of the sequence, which shifts positions while leaving the count larger.
        // Length was never able to distinguish that from an append, so this is not a
        // protection being given up. It is checked where it can actually be checked —
        // by content, against the artefact's own vectors, in
        // `sinephile_catalogue::index::verify_prefix`.
        if (ids.len() as u64) < artefact.header.count {
            return Err(VectorIndexError::CatalogueShorterThanArtefact {
                vectors: artefact.header.count,
                ids: ids.len(),
            });
        }
        let ids = &ids[..artefact.header.count as usize];

        let started = Instant::now();
        let index = Index::new(&options(artefact.header.dimension as usize))?;
        // Reserved up front: usearch grows by reallocating, and 855,703 unplanned
        // growth steps is the difference between minutes and an afternoon.
        let indexable = ids.iter().filter(|id| id.is_some()).count();
        index.reserve(indexable)?;

        let mut added = 0usize;
        for (position, id) in ids.iter().enumerate() {
            let Some(id) = *id else { continue };
            if id <= 0 {
                return Err(VectorIndexError::NonPositiveId(id));
            }
            let vector = artefact
                .vector(position as u64)
                .ok_or(VectorIndexError::MissingVector(position as u64))?;
            index.add(id as u64, vector)?;

            added += 1;
            if added.is_multiple_of(10_000) {
                progress(added);
            }
        }
        progress(added);

        let path_string = path.to_string_lossy().to_string();
        index.save(&path_string)?;

        let bytes = std::fs::metadata(path)
            .map_err(|source| VectorIndexError::Io {
                path: path_string,
                source,
            })?
            .len();

        Ok(BuildReport {
            vectors: indexable,
            considered: ids.len(),
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
        Ok(Self { index, added: None })
    }

    /// What the application opens: the published graph, mapped ([`VectorIndex::view`]),
    /// and the companion of titles added since ([`added_path`]) when there is one.
    ///
    /// The harnesses keep [`VectorIndex::view`]: their exact answer is the published
    /// artefact, which the companion is not part of.
    pub fn open(path: &Path) -> Result<Self, VectorIndexError> {
        let mut index = Self::view(path)?;
        let added = added_path(path);
        if added.is_file() {
            let graph = Index::restore(&added.to_string_lossy())?;
            if graph.dimensions() != index.dimensions() {
                return Err(VectorIndexError::Dimension {
                    index: index.dimensions(),
                    query: graph.dimensions(),
                });
            }
            graph.change_expansion_search(EXPANSION_SEARCH);
            index.added = Some(graph);
        }
        Ok(index)
    }

    /// Open a saved index by reading all of it into memory.
    ///
    /// Only here to measure what [`VectorIndex::view`] saves. Nothing in the application
    /// should call it.
    pub fn load(path: &Path) -> Result<Self, VectorIndexError> {
        let index = Index::restore(&path.to_string_lossy())?;
        index.change_expansion_search(EXPANSION_SEARCH);
        Ok(Self { index, added: None })
    }

    /// Override the search dial for one open index.
    ///
    /// Exists so the harness can **sweep** it: [`EXPANSION_SEARCH`]'s value is a claim
    /// about a trade-off, and a constant nobody has measured either side of is a guess
    /// with a comment attached. `eval vector --ef N` is how the current value was
    /// chosen.
    pub fn set_expansion_search(&self, candidates: usize) {
        self.index.change_expansion_search(candidates);
        if let Some(added) = &self.added {
            added.change_expansion_search(candidates);
        }
    }

    /// The `k` nearest catalogue items to a quantised query vector, from the published
    /// graph and the companion together, nearest first.
    pub fn search(&self, query: &[i8], k: usize) -> Result<Vec<Neighbour>, VectorIndexError> {
        if query.len() != self.index.dimensions() {
            return Err(VectorIndexError::Dimension {
                index: self.index.dimensions(),
                query: query.len(),
            });
        }
        let mut hits = neighbours(&self.index, query, k)?;
        if let Some(added) = self.added.as_ref().filter(|a| a.size() > 0) {
            // A title is in one graph or the other (the companion holds only ids past the
            // published file's last), but a duplicate would cost a slot, so the nearer wins.
            hits.extend(neighbours(added, query, k)?);
            hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            let mut seen = std::collections::HashSet::new();
            hits.retain(|n| seen.insert(n.media_item_id));
            hits.truncate(k);
        }
        Ok(hits)
    }

    /// A catalogue item's own stored vector, if it is in either graph (Phase 14: the seed
    /// of a taste is built from the vectors of what a person liked).
    pub fn vector(&self, media_item_id: i64) -> Option<Vec<i8>> {
        stored(&self.index, media_item_id)
            .or_else(|| self.added.as_ref().and_then(|a| stored(a, media_item_id)))
    }

    /// Titles in both graphs.
    pub fn len(&self) -> usize {
        self.index.size() + self.added_len()
    }

    /// Titles in the companion: those given a vector since the published file.
    pub fn added_len(&self) -> usize {
        self.added.as_ref().map_or(0, Index::size)
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

/// The companion graph opened for writing (ADR-0054): the launch job inserts the vectors of
/// titles the catalogue gained and saves it; [`VectorIndex::open`] reads it.
pub struct Companion {
    index: Index,
    path: PathBuf,
}

impl Companion {
    /// The companion beside the published graph at `index_path`, or a new empty one.
    pub fn open(index_path: &Path, dimensions: usize) -> Result<Self, VectorIndexError> {
        let path = added_path(index_path);
        let index = if path.is_file() {
            let index = Index::restore(&path.to_string_lossy())?;
            if index.dimensions() != dimensions {
                return Err(VectorIndexError::Dimension {
                    index: dimensions,
                    query: index.dimensions(),
                });
            }
            index
        } else {
            Index::new(&options(dimensions))?
        };
        Ok(Self { index, path })
    }

    pub fn contains(&self, media_item_id: i64) -> bool {
        u64::try_from(media_item_id).is_ok_and(|key| self.index.contains(key))
    }

    /// Add a title's vector, replacing the one it had: a synopsis can be rewritten.
    pub fn insert(&mut self, media_item_id: i64, vector: &[i8]) -> Result<(), VectorIndexError> {
        if media_item_id <= 0 {
            return Err(VectorIndexError::NonPositiveId(media_item_id));
        }
        if vector.len() != self.index.dimensions() {
            return Err(VectorIndexError::Dimension {
                index: self.index.dimensions(),
                query: vector.len(),
            });
        }
        let key = media_item_id as u64;
        if self.index.contains(key) {
            self.index.remove(key)?;
        }
        // usearch adds only into reserved room; doubling keeps the growth steps few.
        if self.index.size() + 1 > self.index.capacity() {
            self.index.reserve((self.index.capacity() * 2).max(64))?;
        }
        self.index.add(key, vector)?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.index.size()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write it beside the published graph, whole or not at all: to a part file, then
    /// renamed over the last one, so a crash mid-save leaves the previous companion.
    pub fn save(&self) -> Result<(), VectorIndexError> {
        let part = self.path.with_extension("usearch.part");
        let part_string = part.to_string_lossy().to_string();
        self.index.save(&part_string)?;
        std::fs::rename(&part, &self.path).map_err(|source| VectorIndexError::Io {
            path: self.path.to_string_lossy().to_string(),
            source,
        })
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
            watermark: 0,
            id_digest: [0; 32],
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
        let ids: Vec<Option<i64>> = (0..64).map(|i| Some(5_000 + i * 7)).collect();

        let report = VectorIndex::build(&artefact, &ids, &path, |_| {}).expect("build");
        assert_eq!(report.vectors, 64);
        assert!(report.bytes > 0, "an index that saved nothing is not saved");

        let index = VectorIndex::view(&path).expect("view");
        assert_eq!(index.len(), 64);
        assert_eq!(index.dimensions(), 4);

        // Querying with a stored vector must return that item first, and its neighbours
        // on the circle next — the id mapping and the metric checked in one assertion.
        let hits = index.search(&vectors[10], 3).expect("search");
        assert_eq!(hits[0].media_item_id, ids[10].expect("indexed"));
        assert!(hits[0].distance < 1e-4, "{:?}", hits[0]);
        let neighbours = [ids[9].expect("indexed"), ids[11].expect("indexed")];
        assert!(neighbours.contains(&hits[1].media_item_id), "{hits:?}");
        assert!(neighbours.contains(&hits[2].media_item_id), "{hits:?}");

        // A stored vector reads back by catalogue id, from a viewed (mapped) index too.
        assert_eq!(
            index.vector(ids[10].expect("indexed")),
            Some(vectors[10].clone())
        );
        assert_eq!(index.vector(4_999), None, "not in the graph");
        assert_eq!(index.vector(-1), None);
    }

    #[test]
    fn a_title_added_since_the_published_file_is_found_beside_it() {
        // ADR-0054: the published graph never changes; a new title's vector goes into the
        // companion, and the app's index answers from both.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.usearch");
        let vectors = circle(64);
        let ids: Vec<Option<i64>> = (0..64).map(|i| Some(5_000 + i)).collect();
        VectorIndex::build(&artefact_of(vectors.clone()), &ids, &path, |_| {}).expect("build");

        // Two new titles, each a copy of a published one's direction rotated a little.
        let new_a: Vec<i8> = vec![118, 22, 0, 0];
        let new_b: Vec<i8> = vec![-119, 10, 0, 0];
        let mut companion = Companion::open(&path, 4).expect("open");
        assert!(companion.is_empty());
        companion.insert(90_001, &new_a).expect("insert");
        companion.insert(90_002, &new_b).expect("insert");
        // A rewritten synopsis replaces the vector rather than being refused or doubled.
        companion.insert(90_002, &new_b).expect("replace");
        assert_eq!(companion.len(), 2);
        assert!(companion.contains(90_001));
        companion.save().expect("save");
        assert!(added_path(&path).is_file());
        assert!(!added_path(&path).with_extension("usearch.part").exists());

        let index = VectorIndex::open(&path).expect("open both");
        assert_eq!(index.len(), 66);
        assert_eq!(index.added_len(), 2);
        let hits = index.search(&new_a, 3).expect("search");
        assert_eq!(hits[0].media_item_id, 90_001, "{hits:?}");
        assert!(
            hits.windows(2).all(|w| w[0].distance <= w[1].distance),
            "{hits:?}"
        );
        assert_eq!(index.vector(90_002), Some(new_b.clone()));
        assert_eq!(
            index.vector(5_010),
            Some(vectors[10].clone()),
            "published still read"
        );

        // The harnesses' view is the published graph alone.
        let published = VectorIndex::view(&path).expect("view");
        assert_eq!(published.len(), 64);
        assert_eq!(published.vector(90_001), None);

        // Reopened for writing, it keeps what it had and grows.
        let mut again = Companion::open(&path, 4).expect("reopen");
        assert_eq!(again.len(), 2);
        again.insert(90_003, &[0, 120, 0, 0]).expect("insert");
        assert_eq!(again.len(), 3);
        assert!(
            Companion::open(&path, 8).is_err(),
            "a different model's companion"
        );
        assert!(again.insert(0, &new_a).is_err());
        assert!(again.insert(90_004, &[1, 2]).is_err());
    }

    #[test]
    fn a_catalogue_that_lost_titles_is_refused_rather_than_indexed_against_the_wrong_ones() {
        // Fewer ids than vectors means something was REMOVED. Every position after the
        // gap now names a different title, and no offset arithmetic can recover which.
        // Silent if unchecked, because every offset still reads a valid vector.
        let dir = tempfile::tempdir().expect("tempdir");
        let artefact = artefact_of(circle(16));
        let ids: Vec<Option<i64>> = (1..=15).map(Some).collect();

        let err = VectorIndex::build(&artefact, &ids, &dir.path().join("i.usearch"), |_| {})
            .expect_err("must refuse");
        assert!(
            matches!(
                err,
                VectorIndexError::CatalogueShorterThanArtefact {
                    vectors: 16,
                    ids: 15
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn a_catalogue_that_grew_past_the_artefact_indexes_the_prefix_and_ignores_the_rest() {
        // The ordinary case since subtask 5.7: the catalogue refreshes itself and runs
        // ahead of a published artefact. The surplus titles are not in the graph — BM25
        // covers them — and the ones that ARE indexed must still answer by their own id.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.usearch");
        let vectors = circle(16);
        let artefact = artefact_of(vectors.clone());
        let ids: Vec<Option<i64>> = (1..=40).map(|i| Some(100 + i)).collect();

        let report = VectorIndex::build(&artefact, &ids, &path, |_| {}).expect("build");
        assert_eq!(
            report.vectors, 16,
            "indexed the surplus as well as the prefix"
        );

        let index = VectorIndex::view(&path).expect("view");
        assert_eq!(index.len(), 16);

        // Position 10 must still be the id at position 10, not at some shifted offset.
        let hits = index.search(&vectors[10], 1).expect("search");
        assert_eq!(hits[0].media_item_id, ids[10].expect("indexed"));
    }

    #[test]
    fn a_query_of_the_wrong_shape_is_refused_rather_than_answered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.usearch");
        let artefact = artefact_of(circle(8));
        VectorIndex::build(
            &artefact,
            &(1..=8).map(Some).collect::<Vec<Option<i64>>>(),
            &path,
            |_| {},
        )
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
