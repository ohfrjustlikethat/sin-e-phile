//! The optional downloads, what they cost, and refusing the wrong one (ADR-0014, D27).
//!
//! # What "optional" means here
//!
//! The application is complete without any of this. `SPEC.md` §8 and ADR-0014 make
//! FTS5-only search the floor — diminished, genuinely useful, never broken or empty —
//! and `Engine::keyword_only` is that floor taking the same code path as everything
//! else. What these assets buy is the *meaning* half of search.
//!
//! # Consent, with the size shown, before anything happens
//!
//! ADR-0014 is explicit: the download is consented to, with the size shown, before it
//! starts. No background fetch, no "we'll just grab this". So this module's first job is
//! to answer *what is missing and what will it cost*, and its second is to fetch it.
//!
//! **There are two downloads, not one.** The artefact is the well-known 313 MB; the model
//! that embeds the query is a further 32.4 MB, and it had never been scoped because it
//! was copied into place by hand during development. Both are listed, and the total is
//! what a person is asked about.
//!
//! # Why a checksum is the least interesting check here
//!
//! A corrupt download fails loudly — a bad hash, an obvious error. A **mismatched**
//! artefact fails silently: vectors produced by one model, compared against queries
//! produced by another, land in different regions of a space that has no idea anything
//! is wrong, and search simply gets worse with nothing to point at. So an artefact is
//! verified three times over — transport integrity, then its own internal checksum, then
//! **model identity and document-builder version** — and the last of those is the one
//! that matters.

use std::path::{Path, PathBuf};

use sinephile_embedding::{Artefact, MODEL};

use crate::download::{Downloader, Progress};
use crate::job::JobError;

/// Where the published artefact lives.
///
/// A versioned GitHub Release asset (ADR-0014): not a server this project operates, the
/// same category as the model download and the IMDb datasets, and nothing that can fail
/// in a way that degrades a running installation.
pub const ARTEFACT_RELEASE: &str = "embeddings-v2";

/// One optional download.
#[derive(Debug, Clone)]
pub struct Asset {
    /// What to call it when asking permission.
    pub name: &'static str,
    /// One line on what it buys, in a person's terms.
    pub purpose: &'static str,
    pub url: String,
    pub path: PathBuf,
    /// What it should weigh, for the consent screen — shown BEFORE the download, so it
    /// cannot come from the download.
    pub bytes: u64,
    pub sha256: &'static str,
}

/// Present, missing, or there and wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetState {
    Absent,
    Present,
    /// On disk and unusable. **Carries why**, because "search is worse than it should be"
    /// is not a diagnosis and this is the only place the real reason is known.
    Unusable(String),
}

/// Everything the semantic half needs and does not ship with.
pub fn optional_assets(data_dir: &Path) -> Vec<Asset> {
    let models = data_dir.join("models");
    vec![
        Asset {
            name: "Meaning index",
            purpose: "Lets you search by describing a film rather than naming it.",
            url: format!(
                "https://github.com/ohfrjustlikethat/sin-e-phile/releases/download/{ARTEFACT_RELEASE}/embeddings-{MODEL}.bin"
            ),
            path: sinephile_embedding::artefact_path(data_dir),
            bytes: 328_590_240,
            sha256: "b17f15dab916816be1e4b959cbfce6f5d369ab6e7bd5c676e6760b0a2980b7bb",
        },
        Asset {
            name: "Language model",
            purpose: "Turns what you type into something the meaning index can match.",
            url: "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/onnx/model_quantized.onnx".into(),
            path: models.join(format!("{MODEL}.onnx")),
            bytes: 33_961_249,
            sha256: "6c9c6101a956d62dfb5e7190c538226c0c5bb9cb27b651234b6df063ee7dbfe4",
        },
        Asset {
            name: "Tokenizer",
            purpose: "Splits your words the same way the index was built.",
            url: "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/tokenizer.json".into(),
            path: models.join(format!("{MODEL}-tokenizer.json")),
            bytes: 711_661,
            sha256: "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66",
        },
    ]
}

/// What a person is being asked to agree to, in total.
pub fn total_bytes(assets: &[Asset]) -> u64 {
    assets.iter().map(|a| a.bytes).sum()
}

/// Is this asset usable as it stands?
///
/// Deliberately does **not** re-hash a 313 MB file on every call — that would put half a
/// second of disk on a screen that merely wants to know whether to offer a download. The
/// artefact's own header is read instead, which is 256 bytes and answers the question
/// that actually matters.
pub fn state_of(asset: &Asset) -> AssetState {
    if !asset.path.is_file() {
        return AssetState::Absent;
    }
    // Only the artefact has a header to check. The model and the tokenizer are opaque,
    // and their pinned checksums are verified when they are fetched.
    if asset.path
        != sinephile_embedding::artefact_path(asset.path.parent().unwrap_or(Path::new(".")))
    {
        return AssetState::Present;
    }

    match read_header(&asset.path) {
        Ok(()) => AssetState::Present,
        Err(reason) => AssetState::Unusable(reason),
    }
}

/// Read just the header and ask whether this build can use it.
///
/// **This is the check E7 is about.** A previously published artefact — a different
/// model, or a different document builder — is intact, checksums correctly, and is
/// useless. Refusing it loudly is the entire point.
fn read_header(path: &Path) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let header = sinephile_embedding::Header::peek(&mut file).map_err(|e| e.to_string())?;
    header
        .compatible_with(MODEL, sinephile_embedding::document::VERSION)
        .map_err(|e| e.to_string())
}

/// Fetch one asset and verify it.
///
/// Resumable, because `Downloader` resumes — a 313 MB download that has to restart from
/// zero on a dropped connection is one a person on a poor link can never finish.
pub async fn download(
    asset: &Asset,
    mut on_progress: impl FnMut(Progress),
) -> Result<(), JobError> {
    if let Some(parent) = asset.path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| JobError::step("assets", format!("{}: {e}", parent.display())))?;
    }
    // A stale file would be skipped by the downloader, which is right for a resume and
    // wrong for replacing something that failed verification.
    if matches!(state_of(asset), AssetState::Unusable(_)) {
        let _ = std::fs::remove_file(&asset.path);
    }

    let downloader = Downloader::new();
    downloader
        .fetch(&asset.url, &asset.path, &mut on_progress)
        .await?;

    verify(asset)
}

/// Transport integrity, then the artefact's own checks.
pub fn verify(asset: &Asset) -> Result<(), JobError> {
    crate::embed::verify_sha256(&asset.path, asset.sha256)?;

    // The artefact gets the two checks a checksum cannot make: that it is internally
    // consistent, and that it belongs to THIS build.
    if asset.path
        == sinephile_embedding::artefact_path(asset.path.parent().unwrap_or(Path::new(".")))
    {
        let mut file = std::fs::File::open(&asset.path)
            .map_err(|e| JobError::step("assets", format!("{}: {e}", asset.path.display())))?;
        let artefact =
            Artefact::read(&mut file).map_err(|e| JobError::step("assets", e.to_string()))?;
        artefact
            .header
            .compatible_with(MODEL, sinephile_embedding::document::VERSION)
            .map_err(|e| JobError::step("assets", e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_consent_screen_can_state_a_total_before_anything_is_fetched() {
        // ADR-0014 requires the size shown BEFORE the download, so it cannot be derived
        // from one. 367 MB, and the model is 34 MB of it — the part nobody had scoped.
        let assets = optional_assets(Path::new("data"));
        assert_eq!(assets.len(), 3);
        let total = total_bytes(&assets);
        assert!(
            (360_000_000..375_000_000).contains(&total),
            "total was {total}"
        );
    }

    #[test]
    fn a_missing_asset_is_absent_rather_than_an_error() {
        let assets = optional_assets(Path::new("./definitely-not-a-real-directory"));
        for asset in &assets {
            assert_eq!(state_of(asset), AssetState::Absent, "{}", asset.name);
        }
    }

    #[test]
    fn an_artefact_from_another_model_is_refused_and_says_why() {
        // THE CHECK E7 IS ABOUT. Intact, correctly checksummed, and useless: vectors from
        // one model compared against queries from another land in different regions of a
        // space that has no idea anything is wrong.
        use sinephile_embedding::{artefact, Header, Quantisation};

        let dir = tempfile::tempdir().expect("tempdir");
        let path = sinephile_embedding::artefact_path(dir.path());
        let header = Header {
            model: "some-other-model-int8".into(),
            dimension: 4,
            quantisation: Quantisation::Int8,
            document_builder_version: sinephile_embedding::document::VERSION,
            snapshot_date: "2026-09-14".into(),
            text_source: "wikipedia".into(),
            count: 1,
        };
        let mut bytes = Vec::new();
        artefact::write(&mut bytes, &header, vec![vec![1i8, 2, 3, 4]]).expect("write");
        std::fs::write(&path, &bytes).expect("write file");

        let asset = Asset {
            name: "Meaning index",
            purpose: "",
            url: String::new(),
            path,
            bytes: bytes.len() as u64,
            sha256: "",
        };

        match state_of(&asset) {
            AssetState::Unusable(reason) => {
                assert!(reason.contains("some-other-model-int8"), "{reason}");
            }
            other => panic!("a mismatched artefact must be refused, got {other:?}"),
        }
    }
}
