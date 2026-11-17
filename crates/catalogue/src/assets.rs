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
//!
//! # The installed copy (ADR-0053)
//!
//! The installer carries all three files in `meaning/` beside the executable, laid out as
//! a data folder is, so an installed copy has search by meaning with nothing to download.
//! Each file is read from there when it is there, and from the data folder otherwise (a
//! copy built with cargo, or one that downloaded them before). A downloaded copy that an
//! installed one supersedes is removed ([`remove_superseded`]).

use std::path::{Path, PathBuf};

use sinephile_embedding::{Artefact, MODEL};

use crate::download::{Downloader, Progress};
use crate::job::JobError;

/// Where the published artefact lives.
///
/// A versioned GitHub Release asset (ADR-0014): not a server this project operates, the
/// same category as the model download and the IMDb datasets, and nothing that can fail
/// in a way that degrades a running installation.
///
/// **Bump this with every artefact format or document-layout change.** v3 (format 3,
/// layout v3) can't be read as v2 and v2 can't be read by this code, so pointing at the
/// old release would download 330 MB only to refuse it at load.
pub const ARTEFACT_RELEASE: &str = "embeddings-v3";

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
    /// The whole file's sha256, for assets that cannot vouch for themselves.
    ///
    /// **`None` for the artefact, deliberately.** It carries its own checksum over its
    /// header and vectors, which [`Artefact::read`] verifies — a stronger check than a
    /// pinned constant, because it also catches truncation and cannot go stale when the
    /// artefact is rebuilt.
    ///
    /// The first version pinned one anyway, and pinned the WRONG NUMBER: the value
    /// `ingest embed` prints is the artefact's internal checksum, computed over header
    /// and vectors, while `verify_sha256` hashes the whole file including the trailing
    /// checksum itself. They are different by construction. A real download of the real
    /// published file was refused by that mistake, which is the good outcome — but the
    /// lesson is that two things called "the sha256" were not the same thing.
    pub sha256: Option<&'static str>,
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

/// The language model's file, in `models/`: one name for the download and every loader.
pub const MODEL_FILE: &str = "bge-small-en-v1.5-int8.onnx";
/// The tokenizer's file, in `models/`. The download once saved it as
/// `bge-small-en-v1.5-int8-tokenizer.json` while the app and the tools loaded this name, so
/// no install that downloaded search by meaning could load it (Phase 16, found on a copy of
/// the author's install); only a machine whose file was placed by hand worked.
pub const TOKENIZER_FILE: &str = "bge-small-en-v1.5-tokenizer.json";

/// Where the language model is, for a data directory.
pub fn model_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("models").join(MODEL_FILE)
}

/// Where the tokenizer is, for a data directory.
pub fn tokenizer_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("models").join(TOKENIZER_FILE)
}

/// Where an installed copy carries search by meaning: `meaning/` beside the executable.
pub const INSTALLED_DIR: &str = "meaning";

/// The installed copy's folder, whether or not it is there.
pub fn installed_dir() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join(INSTALLED_DIR))
}

/// The three files search by meaning reads, wherever each one is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeaningFiles {
    pub artefact: PathBuf,
    pub model: PathBuf,
    pub tokenizer: PathBuf,
}

impl MeaningFiles {
    /// For this executable: its installed copy first, then `data_dir`.
    pub fn find(data_dir: &Path) -> Self {
        Self::find_in(data_dir, installed_dir().as_deref())
    }

    /// Each file from `installed` when it is there, else from `data_dir` (where a download
    /// puts it, so a missing file's path is the one to fetch it to).
    pub fn find_in(data_dir: &Path, installed: Option<&Path>) -> Self {
        let pick = |at: fn(&Path) -> PathBuf| {
            installed
                .map(at)
                .filter(|p| p.is_file())
                .unwrap_or_else(|| at(data_dir))
        };
        Self {
            artefact: pick(sinephile_embedding::artefact_path),
            model: pick(model_path),
            tokenizer: pick(tokenizer_path),
        }
    }
}

/// Delete the data folder's copies of files the installed copy carries (ADR-0053): they
/// are never read again, and one may be the corrupt download that broke the author's
/// search by meaning (D90). Returns the bytes freed.
pub fn remove_superseded(data_dir: &Path, installed: Option<&Path>) -> u64 {
    let Some(installed) = installed else {
        return 0;
    };
    let mut freed = 0;
    for at in [
        sinephile_embedding::artefact_path as fn(&Path) -> PathBuf,
        model_path,
        tokenizer_path,
    ] {
        let (mine, downloaded) = (at(installed), at(data_dir));
        if mine.is_file() && downloaded.is_file() && mine != downloaded {
            let bytes = std::fs::metadata(&downloaded).map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(&downloaded).is_ok() {
                freed += bytes;
            }
        }
    }
    freed
}

/// Everything the semantic half needs: the installed copy's files, or the ones to download.
pub fn optional_assets(data_dir: &Path) -> Vec<Asset> {
    optional_assets_in(data_dir, installed_dir().as_deref())
}

/// [`optional_assets`], with the installed copy's folder given.
pub fn optional_assets_in(data_dir: &Path, installed: Option<&Path>) -> Vec<Asset> {
    let files = MeaningFiles::find_in(data_dir, installed);
    vec![
        Asset {
            name: "Meaning index",
            purpose: "Lets you search by describing a film rather than naming it.",
            url: format!(
                "https://github.com/ohfrjustlikethat/sin-e-phile/releases/download/{ARTEFACT_RELEASE}/embeddings-{MODEL}.bin"
            ),
            path: files.artefact,
            bytes: 329_537_568,
            // See the field's note: the artefact verifies itself, header and vectors.
            sha256: None,
        },
        Asset {
            name: "Language model",
            purpose: "Turns what you type into something the meaning index can match.",
            url: "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/onnx/model_quantized.onnx".into(),
            path: files.model,
            bytes: 33_961_249,
            sha256: Some("6c9c6101a956d62dfb5e7190c538226c0c5bb9cb27b651234b6df063ee7dbfe4"),
        },
        Asset {
            name: "Tokenizer",
            purpose: "Splits your words the same way the index was built.",
            url: "https://huggingface.co/Xenova/bge-small-en-v1.5/resolve/main/tokenizer.json".into(),
            path: files.tokenizer,
            bytes: 711_661,
            sha256: Some("d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66"),
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

/// Is this asset usable, judged by reading all of it: the artefact's own checksum and
/// identity, the others' pinned sha256. About a second for the artefact, so for the moment
/// before the files are used, never for a screen that only asks what to offer.
///
/// [`state_of`] reads the artefact's header alone, and a file with a sound header and a
/// corrupt middle passed it: the author's download read as complete, and every build
/// failed on it (D90).
pub fn verified_state(asset: &Asset) -> AssetState {
    if !asset.path.is_file() {
        return AssetState::Absent;
    }
    match verify(asset) {
        Ok(()) => AssetState::Present,
        Err(e) => AssetState::Unusable(e.to_string()),
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
    if !matches!(verified_state(asset), AssetState::Absent) {
        let _ = std::fs::remove_file(&asset.path);
    }

    let downloader = Downloader::new();
    downloader
        .fetch(&asset.url, &asset.path, &mut on_progress)
        .await?;

    // A download that fails its checks is never kept: kept, it read as downloaded and
    // every build failed on it, with nothing to say so (D90).
    verify(asset).inspect_err(|_| {
        let _ = std::fs::remove_file(&asset.path);
    })
}

/// Transport integrity, then the artefact's own checks.
pub fn verify(asset: &Asset) -> Result<(), JobError> {
    if let Some(sha256) = asset.sha256 {
        crate::embed::verify_sha256(&asset.path, sha256)?;
    }

    // The artefact gets the two checks a pinned constant cannot make: that it is
    // internally consistent — its own checksum, over its own bytes, which also catches
    // a truncated download — and that it belongs to THIS build.
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
            watermark: 0,
            id_digest: [0; 32],
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
            sha256: None,
        };

        match state_of(&asset) {
            AssetState::Unusable(reason) => {
                assert!(reason.contains("some-other-model-int8"), "{reason}");
            }
            other => panic!("a mismatched artefact must be refused, got {other:?}"),
        }
    }
}
