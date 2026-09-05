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
pub trait Embedder {
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

/// How many core-tier titles will be embedded.
pub async fn core_count(db: &Db) -> Result<i64, JobError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM media_items WHERE in_core = 1 AND kind <> 'episode'",
    )
    .fetch_one(db.pool())
    .await?)
}

/// Read one batch of core titles, in id order.
async fn rows(db: &Db, after: i64, limit: i64) -> Result<Vec<Row>, JobError> {
    let base: Vec<CoreRow> = sqlx::query_as(
        "SELECT id, primary_title, release_year, kind, synopsis
           FROM media_items
          WHERE in_core = 1 AND kind <> 'episode' AND id > ?
          ORDER BY id LIMIT ?",
    )
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

/// Produce the artefact.
///
/// `snapshot_date` is an input rather than today's date: ADR-0014 requires the same
/// catalogue, model and document builder to yield byte-identical output, and a file
/// that stamps itself with the build date cannot be diffed against a rebuild.
pub async fn produce(
    job: &mut Job<'_>,
    db: &Db,
    embedder: &mut dyn Embedder,
    artefact_path: &Path,
    snapshot_date: &str,
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
    Ok(sqlx::query_scalar(
        "SELECT id FROM media_items
          WHERE in_core = 1 AND kind <> 'episode'
          ORDER BY id LIMIT 1 OFFSET ?",
    )
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
pub const MODEL_IDENTITY: &str = "all-MiniLM-L6-v2-int8";

/// Where the model came from, and what it must hash to.
///
/// Pinned after the first download rather than taken on trust: ADR-0014 requires a
/// checksum on the artefact, and a model that silently changed underneath would
/// invalidate every vector in it while the artefact's own checksum stayed valid.
/// 22,972,370 bytes is the 21.9 MiB Phase 1 Spike C measured, which is how we know it
/// is the same model the R3 latency number was taken with.
pub const MODEL_URL: &str =
    "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/onnx/model_quantized.onnx";
pub const MODEL_SHA256: &str = "afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1";
pub const TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/tokenizer.json";
pub const TOKENIZER_SHA256: &str =
    "da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0";

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
pub struct OnnxEmbedder {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
    identity: String,
    dimension: u16,
}

impl OnnxEmbedder {
    /// Load a model and its tokenizer.
    ///
    /// `identity` is written into the artefact and compared on load, so it must name
    /// the model AND its quantisation: `all-MiniLM-L6-v2-int8` and
    /// `all-MiniLM-L6-v2-fp32` produce different vectors and must not be interchangeable.
    pub fn load(model: &Path, tokenizer: &Path, identity: &str) -> Result<Self, JobError> {
        fn fail(what: &str, e: impl std::fmt::Display) -> JobError {
            JobError::step("embed", format!("{what}: {e}"))
        }

        let session = ort::session::Session::builder()
            .map_err(|e| fail("session builder", e))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| fail("optimisation level", e))?
            .commit_from_file(model)
            .map_err(|e| fail(&format!("loading {}", model.display()), e))?;

        let mut tok = tokenizers::Tokenizer::from_file(tokenizer)
            .map_err(|e| fail(&format!("loading {}", tokenizer.display()), e))?;
        // This tokenizer.json pads to a fixed 128 tokens. Spike C found that it makes
        // the model do roughly four times the work a short input needs; over 855,703
        // documents that is the difference between twenty minutes and an hour.
        tok.with_padding(None);
        // Documents are longer than queries and the model's limit is 256. Truncating
        // here makes the cut deterministic rather than leaving it to a tokenizer
        // version, which matters because the artefact must be byte-reproducible.
        tok.with_truncation(Some(tokenizers::TruncationParams {
            max_length: 256,
            ..Default::default()
        }))
        .map_err(|e| fail("truncation", e))?;

        Ok(Self {
            session,
            tokenizer: tok,
            identity: identity.to_string(),
            // Filled in on the first embedding; MiniLM is 384 and this is asserted
            // against the artefact header rather than assumed.
            dimension: 384,
        })
    }
}

impl Embedder for OnnxEmbedder {
    fn identity(&self) -> &str {
        &self.identity
    }

    fn dimension(&self) -> u16 {
        self.dimension
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, JobError> {
        use ort::value::TensorRef;

        let encoded = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| JobError::step("embed", format!("tokenize: {e}")))?;
        let len = encoded.len().max(1);

        let ids: Vec<i64> = encoded.get_ids().iter().map(|i| *i as i64).collect();
        let mask: Vec<i64> = encoded
            .get_attention_mask()
            .iter()
            .map(|i| *i as i64)
            .collect();
        let types: Vec<i64> = vec![0; len];
        let shape = [1_i64, len as i64];

        let map = |e: ort::Error| JobError::step("embed", e.to_string());
        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => TensorRef::from_array_view((shape, ids.as_slice())).map_err(map)?,
                "attention_mask" => TensorRef::from_array_view((shape, mask.as_slice())).map_err(map)?,
                "token_type_ids" => TensorRef::from_array_view((shape, types.as_slice())).map_err(map)?,
            ])
            .map_err(map)?;

        let (out_shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(map)?;
        let hidden = *out_shape.last().unwrap_or(&384) as usize;

        // Mean-pool over non-padding tokens, then L2-normalise. Padding tokens carry
        // real activations, so averaging them in makes every long document drift
        // toward the same point.
        let mut pooled = vec![0f32; hidden];
        let mut counted = 0f32;
        for t in 0..len {
            if mask.get(t).copied().unwrap_or(0) == 0 {
                continue;
            }
            counted += 1.0;
            for h in 0..hidden {
                pooled[h] += data[t * hidden + h];
            }
        }
        for v in pooled.iter_mut() {
            *v /= counted.max(1.0);
        }
        let norm = pooled.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
        for v in pooled.iter_mut() {
            *v /= norm;
        }
        Ok(pooled)
    }
}
