//! Producing the embedding artefact (ADR-0014, subtask 4.10).
//!
//! Walks the core tier, turns each title into a sentence
//! ([`sinephile_embedding::build_document`]), embeds it, quantises to int8 and writes
//! the file the application will later refuse to load if it does not match.
//!
//! # Resumability, and where the checkpoint actually lives
//!
//! 855,703 titles is a long run and ADR-0014 requires it to be resumable. The vectors
//! go to a `.part` file, append-only, and the job cursor is simply how many have been
//! written — so a resume truncates to `cursor * dimension` and carries on. The finished
//! artefact is assembled at the end, which is also when the checksum is computed, so an
//! interrupted run never leaves a file that looks complete.
//!
//! # Determinism
//!
//! Titles are read in `media_items.id` order, which is the order the artefact indexes
//! by. Nothing here consults the clock: the snapshot date is passed in.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use sinephile_embedding::{artefact, document, quantise, Document, Header, Quantisation};
use sinephile_persistence::Db;

use crate::job::{Job, JobError};

/// Titles per transaction. Each is an inference, so this is about how much work a crash
/// discards rather than about database cost.
const BATCH: i64 = 500;

/// Anything that turns a sentence into a vector.
///
/// A trait so the producer is testable without a 22 MB model — the same reason
/// `metadata-api` has a `Transport` trait, and it earns its keep the same way: every
/// bug found here so far was in the batching and the resume, not in the arithmetic.
pub trait DocumentEmbedder {
    /// The model's identity, recorded in the artefact and compared on load.
    fn identity(&self) -> &str;
    fn dimension(&self) -> u16;
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError>;
}

/// `(id, primary_title, release_year, kind, synopsis)` as the core-tier query returns
/// it. Named because the tuple is wide enough to be unreadable inline.
type CoreRow = (i64, String, Option<i64>, String, Option<String>);

/// One catalogue row, owned, because the document builder borrows.
struct Row {
    id: i64,
    title: String,
    alternative_titles: Vec<String>,
    year: Option<i64>,
    kind: String,
    genres: Vec<String>,
    people: Vec<String>,
    synopsis: Option<String>,
}

impl Row {
    fn sentence(&self) -> String {
        let alternatives: Vec<&str> = self.alternative_titles.iter().map(String::as_str).collect();
        let genres: Vec<&str> = self.genres.iter().map(String::as_str).collect();
        let people: Vec<&str> = self.people.iter().map(String::as_str).collect();
        document::build(&Document {
            title: &self.title,
            alternative_titles: &alternatives,
            year: self.year,
            kind: &self.kind,
            genres: &genres,
            people: &people,
            synopsis: self.synopsis.as_deref(),
        })
    }
}

/// The core-tier predicate, borrowed from the crate that owns it.
///
/// It lives in `crates/persistence` rather than here because the **application** needs
/// the same definition to map artefact positions onto catalogue ids after a download,
/// and the application cannot depend on a dev tool. Producer and consumer therefore
/// share one string; see `CatalogueRepository::core_ids`.
use sinephile_persistence::repositories::CORE_TIER as CORE;

/// How many core-tier titles will be embedded.
pub async fn core_count(db: &Db) -> Result<i64, JobError> {
    Ok(
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM media_items WHERE {CORE}"))
            .fetch_one(db.pool())
            .await?,
    )
}

/// Read one batch of core titles, in id order.
async fn rows(db: &Db, after: i64, limit: i64) -> Result<Vec<Row>, JobError> {
    let base: Vec<CoreRow> = sqlx::query_as(&format!(
        "SELECT id, primary_title, release_year, kind, synopsis
           FROM media_items
          WHERE {CORE} AND id > ?
          ORDER BY id LIMIT ?"
    ))
    .bind(after)
    .bind(limit)
    .fetch_all(db.pool())
    .await?;

    let mut out = Vec::with_capacity(base.len());
    for (id, title, year, kind, synopsis) in base {
        // Alternative titles, genres and billing, each ordered so the sentence is the
        // same on every run. An unordered read would produce a different document —
        // and therefore a different vector — for the same catalogue.
        let alternative_titles: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT title FROM titles
              WHERE media_item_id = ? AND variant IN ('romaji', 'native', 'english', 'original')
              ORDER BY variant, title LIMIT 4",
        )
        .bind(id)
        .fetch_all(db.pool())
        .await?;

        let genres: Vec<String> = sqlx::query_scalar(
            "SELECT g.name FROM media_genres mg
               JOIN genres g ON g.id = mg.genre_id
              WHERE mg.media_item_id = ? ORDER BY g.name",
        )
        .bind(id)
        .fetch_all(db.pool())
        .await?;

        // Directors first, then billed cast. `ordering` is IMDb's billing position,
        // which is the closest thing to "who this film is about".
        let people: Vec<String> = sqlx::query_scalar(
            "SELECT p.name FROM credits c
               JOIN people p ON p.id = c.person_id
              WHERE c.media_item_id = ?
              ORDER BY CASE c.role WHEN 'director' THEN 0 ELSE 1 END, c.billing, p.name
              LIMIT 6",
        )
        .bind(id)
        .fetch_all(db.pool())
        .await?;

        out.push(Row {
            id,
            title,
            alternative_titles,
            year,
            kind,
            genres,
            people,
            synopsis,
        });
    }
    Ok(out)
}

/// Where the in-progress vectors live.
fn part_path(artefact_path: &Path) -> PathBuf {
    artefact_path.with_extension("vectors.part")
}

/// The exact sentence the producer would embed for one catalogue item.
///
/// Public so the harness can re-derive a document and compare its vector against the
/// one already in the artefact — which is the only way to catch the producer and the
/// query path having drifted apart. It reuses [`rows`] rather than reimplementing the
/// assembly, because a second copy of "what a document is" would agree on the day it
/// was written and never again.
pub async fn document_for(db: &Db, media_item_id: i64) -> Result<Option<String>, JobError> {
    // `rows` reads *after* an id, so ask for the one before it and take the first row —
    // which also confirms the item is in the core tier at all, since a non-core id
    // simply returns its next core neighbour and the id check below catches it.
    let batch = rows(db, media_item_id - 1, 1).await?;
    Ok(batch
        .into_iter()
        .find(|r| r.id == media_item_id)
        .map(|r| r.sentence()))
}

/// Produce the artefact.
///
/// `snapshot_date` is an input rather than today's date: ADR-0014 requires the same
/// catalogue, model and document builder to yield byte-identical output, and a file
/// that stamps itself with the build date cannot be diffed against a rebuild.
pub async fn produce(
    job: &mut Job<'_>,
    db: &Db,
    embedder: &mut dyn DocumentEmbedder,
    artefact_path: &Path,
    snapshot_date: &str,
    text_source: &str,
) -> Result<Produced, JobError> {
    let count = core_count(db).await?;
    let dimension = embedder.dimension();
    let part = part_path(artefact_path);

    // `run_step` cannot hold the embedder — it is `&mut dyn` and the closure is
    // higher-ranked — so the loop is here and the checkpoint is written through the
    // step. The cursor is "vectors written", which is all a resume needs.
    let mut written: i64 = existing_vectors(&part, dimension)?;
    if written > 0 {
        tracing::info!("resuming with {written} vectors already embedded");
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        // NOT truncate: the whole point is to keep what a previous run wrote. The
        // length is set explicitly below, to the last WHOLE vector.
        .truncate(false)
        .read(true)
        .write(true)
        .open(&part)
        .map_err(|e| JobError::step("embed", format!("{}: {e}", part.display())))?;
    // Truncate any partial vector from a kill mid-write. Without this the file is a
    // byte or two long and every subsequent vector is misaligned.
    file.set_len(written as u64 * dimension as u64)
        .map_err(|e| JobError::step("embed", e.to_string()))?;
    file.seek(SeekFrom::End(0))
        .map_err(|e| JobError::step("embed", e.to_string()))?;

    let mut after = last_id_for(db, written).await?;
    while written < count {
        let batch = rows(db, after, BATCH).await?;
        if batch.is_empty() {
            break;
        }
        for row in &batch {
            let vector = embedder.embed(&row.sentence())?;
            let quantised = quantise(&vector);
            let bytes: Vec<u8> = quantised.values.iter().map(|v| *v as u8).collect();
            file.write_all(&bytes)
                .map_err(|e| JobError::step("embed", e.to_string()))?;
            after = row.id;
            written += 1;
        }
        file.flush()
            .map_err(|e| JobError::step("embed", e.to_string()))?;
        job.checkpoint("embed.vectors", &written.to_string(), written)
            .await?;
        tracing::info!("{written}/{count} embedded");
    }
    drop(file);

    // Assemble. The checksum is computed here, over the finished file, so an
    // interrupted run never leaves something that looks complete.
    let header = Header {
        model: embedder.identity().to_string(),
        dimension,
        quantisation: Quantisation::Int8,
        document_builder_version: document::VERSION,
        snapshot_date: snapshot_date.to_string(),
        text_source: text_source.to_string(),
        count: written as u64,
    };
    let vectors = std::fs::read(&part)
        .map_err(|e| JobError::step("embed", format!("{}: {e}", part.display())))?;
    let mut out = std::fs::File::create(artefact_path)
        .map_err(|e| JobError::step("embed", format!("{}: {e}", artefact_path.display())))?;

    let checksum = artefact::write(
        &mut out,
        &header,
        vectors
            .chunks(dimension as usize)
            .map(|chunk| chunk.iter().map(|b| *b as i8).collect::<Vec<i8>>()),
    )
    .map_err(|e| JobError::step("embed", e.to_string()))?;
    out.flush()
        .map_err(|e| JobError::step("embed", e.to_string()))?;
    let _ = std::fs::remove_file(&part);

    let bytes = std::fs::metadata(artefact_path)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(Produced {
        count: written as u64,
        bytes,
        checksum,
    })
}

#[derive(Debug, Clone)]
pub struct Produced {
    pub count: u64,
    pub bytes: u64,
    pub checksum: [u8; 32],
}

impl Produced {
    pub fn checksum_hex(&self) -> String {
        self.checksum.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// How many complete vectors a part file already holds.
fn existing_vectors(part: &Path, dimension: u16) -> Result<i64, JobError> {
    match std::fs::metadata(part) {
        // Integer division deliberately discards a trailing partial vector.
        Ok(meta) => Ok((meta.len() / dimension as u64) as i64),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(JobError::step("embed", format!("{}: {e}", part.display()))),
    }
}

/// The catalogue id the nth core title has, so a resume knows where to read from.
///
/// Derived from the data rather than stored, because the two must agree and only one
/// of them can be authoritative. If the cursor said 40,000 and the catalogue had since
/// changed, a stored id would resume in the wrong place silently.
async fn last_id_for(db: &Db, written: i64) -> Result<i64, JobError> {
    if written == 0 {
        return Ok(0);
    }
    Ok(sqlx::query_scalar(&format!(
        "SELECT id FROM media_items
          WHERE {CORE}
          ORDER BY id LIMIT 1 OFFSET ?"
    ))
    .bind(written - 1)
    .fetch_optional(db.pool())
    .await?
    .unwrap_or(0))
}

/// The model this project embeds with, and its identity in the artefact.
///
/// The identity names the QUANTISATION as well as the model, because
/// `all-MiniLM-L6-v2-int8` and `all-MiniLM-L6-v2-fp32` produce different vectors and
/// must never be interchangeable — the artefact header compares this verbatim.
pub use sinephile_embedding::MODEL as MODEL_IDENTITY;

/// Where the model came from, and what it must hash to.
///
/// Pinned after the first download rather than taken on trust: ADR-0014 requires a
/// checksum on the artefact, and a model that silently changed underneath would
/// invalidate every vector in it while the artefact's own checksum stayed valid.
/// 22,972,370 bytes is the 21.9 MiB Phase 1 Spike C measured, which is how we know it
/// is the same model the R3 latency number was taken with.
pub const MODEL_URL: &str =
    "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/onnx/model_quantized.onnx";
pub const MODEL_SHA256: &str = "6c9c6101a956d62dfb5e7190c538226c0c5bb9cb27b651234b6df063ee7dbfe4";
pub const TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/tokenizer.json";
pub const TOKENIZER_SHA256: &str =
    "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66";

/// Verify a downloaded file against its pinned hash.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), JobError> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)
        .map_err(|e| JobError::step("embed", format!("{}: {e}", path.display())))?;
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        return Err(JobError::step(
            "embed",
            format!(
                "{} has sha256 {actual}, expected {expected} — refusing to embed with                  a model that is not the one pinned",
                path.display()
            ),
        ));
    }
    Ok(())
}

/// The real embedder: ONNX Runtime plus a sentence-transformer.
///
/// Reuses the inference path proven in Phase 1 Spike C — tokenize, run, mean-pool over
/// non-padding tokens, L2-normalise — because that is the code the R3 measurement was
/// taken with and rewriting it would invalidate the number.
/// The ONNX embedder, from the crate the APPLICATION can also use.
///
/// It moved out of this tool in Phase 5: a query is embedded on every tier (ADR-0015)
/// and the application cannot depend on a dev tool. Producer and query path now share
/// one tokenizer configuration and one pooling implementation, which is what stops a
/// query from landing somewhere the documents are not.
pub use sinephile_embedder::Embedder as OnnxEmbedder;

/// The producer's seam, kept: `produce` takes `&mut dyn DocumentEmbedder` so its
/// batching and resume can be tested without a 22 MB model. Every bug found in this
/// module so far was in the batching or the resume, never in the arithmetic.
impl DocumentEmbedder for sinephile_embedder::Embedder {
    fn identity(&self) -> &str {
        sinephile_embedder::Embedder::identity(self)
    }

    fn dimension(&self) -> u16 {
        sinephile_embedder::Embedder::dimension(self)
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError> {
        // embed_DOCUMENT. The producer must never apply the query prefix: the model's
        // asymmetry is the whole reason it was chosen (ADR-0034), and prefixing both
        // sides discards it exactly as thoroughly as prefixing neither.
        sinephile_embedder::Embedder::embed_document(self, text)
            .map_err(|e| JobError::step("embed", e.to_string()))
    }
}
