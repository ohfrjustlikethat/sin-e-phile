//! The embedding artefact (ADR-0014): what is built, what is published, what is
//! refused.
//!
//! Shared by the producer in `tools/ingest` and by the application, because a format
//! only one side can read is not a format.
//!
//! **No dependency on `ort`.** Producing embeddings needs a 22 MB model; reading,
//! verifying and quantising them does not, and the application must be able to reject
//! a mismatched artefact without loading a model to find out.

pub mod artefact;
pub mod document;
pub mod quantise;

/// The model this build's vectors were produced with, quantisation included.
///
/// The identity names the quantisation because `all-MiniLM-L6-v2-int8` and
/// `all-MiniLM-L6-v2-fp32` produce different vectors and would compare against each
/// other without complaint. [`Header::compatible_with`] refuses on exactly this string.
///
/// It lives here rather than with the producer because **both sides need it**: the
/// producer stamps it into the artefact, and the application checks a downloaded
/// artefact against it before trusting a single vector.
pub const MODEL: &str = "bge-small-en-v1.5-int8";

/// What a QUERY is prefixed with before it is embedded, and documents are not.
///
/// BGE is trained asymmetrically: the query side is instructed, the passage side is
/// bare. **This is not decoration and it is not optional** — omitting it costs most of
/// the model's retrieval advantage, and costs it silently, because the vectors are still
/// perfectly valid and merely worse. It is the reason the model was swapped
/// (ADR-0034), so leaving it out would be swapping the model and discarding the point.
///
/// Empty for a symmetric model: `all-MiniLM-L6-v2` used no prefix at all.
pub const QUERY_PREFIX: &str = "Represent this sentence for searching relevant passages: ";

/// Where the artefact lives inside the data directory.
///
/// **Named after the model**, so a build that changes models cannot silently read the
/// previous one's vectors: it looks for a file that is not there, which fails clearly
/// rather than quietly.
pub fn artefact_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join(format!("embeddings-{MODEL}.bin"))
}

/// Where the document text came from, for artefacts this build produces (ADR-0033).
///
/// Recorded in the artefact header so a rebuild is identifiable. It is deliberately a
/// plain string rather than an enum: ADR-0018 makes the source swappable and
/// composable, and "wikipedia+tmdb" is a value a future build may legitimately want to
/// write without a format change.
pub const TEXT_SOURCE: &str = "wikipedia";

pub use artefact::{Artefact, ArtefactError, Header, Quantisation};
pub use document::{build as build_document, Document};
pub use quantise::{cosine, dequantise, quantise, Quantised};
