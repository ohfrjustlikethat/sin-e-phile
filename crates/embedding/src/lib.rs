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

pub use artefact::{Artefact, ArtefactError, Header, Quantisation};
pub use document::{build as build_document, Document};
pub use quantise::{cosine, dequantise, quantise, Quantised};
