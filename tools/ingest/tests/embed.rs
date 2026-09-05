//! Producing the embedding artefact, without a model.
//!
//! Every SQL statement in `embed.rs` runs against a migrated database here (ADR-0026),
//! and the resume path is exercised with a fake embedder — the interesting bugs in a
//! producer are in its batching and its checkpoint, not in its arithmetic.

use std::path::Path;

use sinephile_embedding::{document, Artefact};
use sinephile_ingest::embed::{self, Embedder};
use sinephile_ingest::{Job, JobError};
use sinephile_persistence::{Db, NewMediaItem};

/// Deterministic vectors derived from the text, so a wrong sentence produces a
/// detectably wrong vector.
struct Fake {
    dimension: u16,
    /// Every sentence it was asked to embed, in order.
    seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// Fail once the nth embedding is requested, to simulate a kill mid-run.
    fail_after: Option<usize>,
}

impl Fake {
    fn new(dimension: u16) -> Self {
        Self {
            dimension,
            seen: Default::default(),
            fail_after: None,
        }
    }

    fn failing_after(dimension: u16, n: usize) -> Self {
        Self {
            fail_after: Some(n),
            ..Self::new(dimension)
        }
    }
}

impl Embedder for Fake {
    fn identity(&self) -> &str {
        "fake-model-int8"
    }

    fn dimension(&self) -> u16 {
        self.dimension
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError> {
        let mut seen = self.seen.lock().expect("lock");
        if self.fail_after.is_some_and(|n| seen.len() >= n) {
            return Err(JobError::step("embed", "simulated kill"));
        }
        seen.push(text.to_string());

        // A unit vector that depends on the text, so two different sentences cannot
        // silently produce the same vector.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let raw: Vec<f32> = (0..self.dimension)
            .map(|i| {
                let n = hash.wrapping_add(i as u64).wrapping_mul(2654435761);
                ((n >> 33) as f32 / (1u64 << 30) as f32) - 1.0
            })
            .collect();
        let norm = raw.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
        Ok(raw.into_iter().map(|v| v / norm).collect())
    }
}

async fn catalogue(db: &Db, count: usize) {
    let media = sinephile_persistence::repositories::MediaRepository::new(db);
    for i in 0..count {
        let id = media
            .insert(&NewMediaItem::film(format!("Film {i:03}"), 1950 + i as i64))
            .await
            .expect("insert");
        // Core tier: only these are embedded.
        sqlx::query("UPDATE media_items SET in_core = 1 WHERE id = ?")
            .bind(id)
            .execute(db.pool())
            .await
            .expect("core");
    }
}

async fn produce(
    db: &Db,
    embedder: &mut dyn Embedder,
    path: &Path,
) -> Result<embed::Produced, JobError> {
    let mut job = Job::begin(db, "embed").await.expect("begin");
    let produced = embed::produce(&mut job, db, embedder, path, "2026-09-06").await?;
    job.finish().await.expect("finish");
    Ok(produced)
}

#[tokio::test]
async fn an_artefact_is_produced_and_reads_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 5).await;

    let path = dir.path().join("embeddings.bin");
    let produced = produce(&db, &mut Fake::new(8), &path)
        .await
        .expect("produce");

    assert_eq!(produced.count, 5);
    assert_eq!(produced.checksum_hex().len(), 64);

    let mut file = std::fs::File::open(&path).expect("open artefact");
    let artefact = Artefact::read(&mut file).expect("read");
    assert_eq!(artefact.header.count, 5);
    assert_eq!(artefact.header.dimension, 8);
    assert_eq!(artefact.header.model, "fake-model-int8");
    assert_eq!(artefact.header.snapshot_date, "2026-09-06");
    assert_eq!(
        artefact.header.document_builder_version,
        document::VERSION,
        "the artefact must record the builder that wrote it"
    );
    assert!(artefact.vector(4).is_some());
    assert!(artefact.vector(5).is_none());
}

#[tokio::test]
async fn only_core_tier_titles_are_embedded() {
    // The whole point of the core tier: embedding 2.7 million titles instead of
    // 855,703 would be a 1 GB artefact nobody can download.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 3).await;

    let media = sinephile_persistence::repositories::MediaRepository::new(&db);
    media
        .insert(&NewMediaItem::film("Not Core", 1999))
        .await
        .expect("insert");

    let path = dir.path().join("e.bin");
    let produced = produce(&db, &mut Fake::new(8), &path)
        .await
        .expect("produce");
    assert_eq!(produced.count, 3);
}

#[tokio::test]
async fn an_interrupted_run_resumes_without_losing_or_repeating_a_vector() {
    // The requirement ADR-0014 states, and the one a 855,703-title run depends on.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 12).await;
    let path = dir.path().join("e.bin");

    // Die after 7.
    let mut dying = Fake::failing_after(8, 7);
    assert!(produce(&db, &mut dying, &path).await.is_err());
    assert_eq!(dying.seen.lock().expect("lock").len(), 7);

    // Resume. The second run must embed only what is missing.
    let mut resumed = Fake::new(8);
    let produced = produce(&db, &mut resumed, &path).await.expect("resume");
    let embedded_on_resume = resumed.seen.lock().expect("lock").len();

    assert_eq!(produced.count, 12, "every title is in the artefact");
    assert!(
        embedded_on_resume <= 7,
        "resume re-embedded {embedded_on_resume} of 12 — a resume may redo a little \
         work, never the whole run"
    );

    // And the artefact is complete and correct.
    let mut file = std::fs::File::open(&path).expect("open");
    let artefact = Artefact::read(&mut file).expect("read");
    assert_eq!(artefact.header.count, 12);
    for i in 0..12 {
        assert!(artefact.vector(i).is_some(), "vector {i} is missing");
    }
}

#[tokio::test]
async fn a_partial_vector_left_by_a_kill_is_discarded() {
    // A process killed mid-write leaves a few bytes of a vector. Without truncating
    // them, every subsequent vector is misaligned by that many bytes and the artefact
    // is silently, uniformly wrong.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 4).await;
    let path = dir.path().join("e.bin");

    let mut dying = Fake::failing_after(8, 2);
    assert!(produce(&db, &mut dying, &path).await.is_err());

    // Simulate the torn write: three stray bytes on the end of the part file.
    let part = path.with_extension("vectors.part");
    let mut bytes = std::fs::read(&part).expect("part");
    assert_eq!(bytes.len(), 2 * 8, "two whole vectors so far");
    bytes.extend_from_slice(&[1, 2, 3]);
    std::fs::write(&part, &bytes).expect("write part");

    let produced = produce(&db, &mut Fake::new(8), &path)
        .await
        .expect("resume");
    assert_eq!(produced.count, 4);

    let mut file = std::fs::File::open(&path).expect("open");
    let artefact = Artefact::read(&mut file).expect("read");
    assert_eq!(artefact.header.count, 4);
    assert_eq!(
        artefact.vector(3).map(|v| v.len()),
        Some(8),
        "the torn bytes must not have shifted the last vector"
    );
}

#[tokio::test]
async fn the_same_catalogue_produces_a_byte_identical_artefact() {
    // ADR-0014's determinism requirement, end to end.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 6).await;

    let first = dir.path().join("a.bin");
    let second = dir.path().join("b.bin");
    let a = produce(&db, &mut Fake::new(8), &first).await.expect("a");
    Job::reset(&db, "embed").await.expect("reset");
    let b = produce(&db, &mut Fake::new(8), &second).await.expect("b");

    assert_eq!(a.checksum, b.checksum, "checksums must match");
    assert_eq!(
        std::fs::read(&first).expect("a"),
        std::fs::read(&second).expect("b"),
        "the files must be byte-identical"
    );
}

#[tokio::test]
async fn the_sentence_embedded_is_the_one_the_document_builder_makes() {
    // If the producer built its own string, the artefact's recorded builder version
    // would be a lie and a query embedded through `build_document` would be comparing
    // against something else.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    catalogue(&db, 1).await;

    let mut fake = Fake::new(8);
    produce(&db, &mut fake, &dir.path().join("e.bin"))
        .await
        .expect("produce");

    let seen = fake.seen.lock().expect("lock");
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0],
        document::build(&sinephile_embedding::Document {
            title: "Film 000",
            year: Some(1950),
            kind: "film",
            ..Default::default()
        })
    );
}

#[tokio::test]
async fn an_empty_core_tier_produces_a_valid_empty_artefact() {
    // A catalogue mid-ingestion has no core tier yet. That must be an artefact with
    // nothing in it, not a crash and not a corrupt file.
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Db::open_in(dir.path()).await.expect("open");
    let path = dir.path().join("e.bin");

    let produced = produce(&db, &mut Fake::new(8), &path)
        .await
        .expect("produce");
    assert_eq!(produced.count, 0);

    let mut file = std::fs::File::open(&path).expect("open");
    let artefact = Artefact::read(&mut file).expect("read");
    assert_eq!(artefact.header.count, 0);
    assert_eq!(artefact.vector(0), None);
}
