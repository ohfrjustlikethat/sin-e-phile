//! Search by meaning's files: the installed copy first, and a corrupt download never kept
//! (Phase 16, round 9: D90, ADR-0053).
//!
//! The author's meaning file downloaded with a sound header and 5 MB of the wrong bytes in
//! its middle. The screen read only the header, so it said everything was downloaded;
//! "Build the index" read all of it and failed on the checksum, every time, and the file
//! stayed. These pin each link of that chain.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;

use sinephile_catalogue::assets::{
    self, download, optional_assets_in, remove_superseded, state_of, verified_state, Asset,
    AssetState, MeaningFiles,
};
use sinephile_embedding::{artefact, Header, Quantisation, MODEL};

/// A small artefact this build accepts: the right model and document layout.
fn artefact_bytes() -> Vec<u8> {
    let header = Header {
        model: MODEL.into(),
        dimension: 8,
        quantisation: Quantisation::Int8,
        document_builder_version: sinephile_embedding::document::VERSION,
        snapshot_date: "2026-11-17".into(),
        text_source: "test".into(),
        count: 64,
        watermark: 0,
        id_digest: [3; 32],
    };
    let vectors: Vec<Vec<i8>> = (0..64)
        .map(|i| (0..8).map(|j| (i * 8 + j) as i8).collect())
        .collect();
    let mut bytes = Vec::new();
    artefact::write(&mut bytes, &header, vectors).expect("write");
    bytes
}

/// The same artefact with its middle overwritten, as the author's was.
fn corrupt_in_the_middle() -> Vec<u8> {
    let mut bytes = artefact_bytes();
    let middle = bytes.len() / 2;
    for b in &mut bytes[middle..middle + 40] {
        *b = b.wrapping_add(91);
    }
    bytes
}

fn the_artefact(path: &Path) -> Asset {
    Asset {
        name: "Meaning index",
        purpose: "",
        url: String::new(),
        path: path.to_path_buf(),
        bytes: 0,
        sha256: None,
    }
}

#[test]
fn a_corrupt_middle_passes_the_header_check_and_fails_the_full_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = sinephile_embedding::artefact_path(dir.path());
    std::fs::write(&path, corrupt_in_the_middle()).expect("write");
    let asset = the_artefact(&path);

    // Why the screen said it was downloaded: the header is sound.
    assert_eq!(state_of(&asset), AssetState::Present);
    // What the build now asks before using it.
    assert!(
        matches!(verified_state(&asset), AssetState::Unusable(_)),
        "{:?}",
        verified_state(&asset)
    );

    std::fs::write(&path, artefact_bytes()).expect("write");
    assert_eq!(verified_state(&asset), AssetState::Present);
}

#[test]
fn the_installed_copy_is_read_before_the_data_folder() {
    let (data, installed) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    std::fs::write(
        sinephile_embedding::artefact_path(data.path()),
        b"downloaded",
    )
    .unwrap();
    std::fs::create_dir_all(data.path().join("models")).unwrap();
    std::fs::write(assets::model_path(data.path()), b"downloaded").unwrap();
    std::fs::write(
        sinephile_embedding::artefact_path(installed.path()),
        b"installed",
    )
    .unwrap();

    let files = MeaningFiles::find_in(data.path(), Some(installed.path()));
    assert_eq!(
        files.artefact,
        sinephile_embedding::artefact_path(installed.path())
    );
    // The installed copy lacks these: the data folder's, present or where to fetch to.
    assert_eq!(files.model, assets::model_path(data.path()));
    assert_eq!(files.tokenizer, assets::tokenizer_path(data.path()));

    // The assets the screen offers follow the same rule.
    let offered = optional_assets_in(data.path(), Some(installed.path()));
    assert_eq!(offered[0].path, files.artefact);
    // With no installed copy, everything is the data folder's.
    let alone = MeaningFiles::find_in(data.path(), None);
    assert_eq!(
        alone.artefact,
        sinephile_embedding::artefact_path(data.path())
    );
}

#[test]
fn a_download_the_installed_copy_supersedes_is_removed_and_nothing_else() {
    let (data, installed) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let downloaded = sinephile_embedding::artefact_path(data.path());
    std::fs::write(&downloaded, corrupt_in_the_middle()).unwrap();
    std::fs::create_dir_all(data.path().join("models")).unwrap();
    std::fs::write(assets::model_path(data.path()), b"model").unwrap();

    assert_eq!(
        remove_superseded(data.path(), None),
        0,
        "no installed copy: nothing goes"
    );
    assert!(downloaded.is_file());

    std::fs::write(
        sinephile_embedding::artefact_path(installed.path()),
        artefact_bytes(),
    )
    .unwrap();
    let freed = remove_superseded(data.path(), Some(installed.path()));
    assert_eq!(freed, corrupt_in_the_middle().len() as u64);
    assert!(!downloaded.exists(), "the superseded download is gone");
    assert!(
        assets::model_path(data.path()).is_file(),
        "a file the installed copy lacks stays"
    );
}

#[tokio::test]
async fn a_download_that_fails_its_check_is_not_kept() {
    // Served whole and in one piece, and corrupt at the source: the checksum is all that
    // can catch it, and then the file must not stay to pass for a download.
    let body = corrupt_in_the_middle();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/meaning.bin", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });

    let dir = tempfile::tempdir().expect("tempdir");
    let mut asset = the_artefact(&sinephile_embedding::artefact_path(dir.path()));
    asset.url = url;
    let result = download(&asset, |_| {}).await;

    assert!(result.is_err(), "a corrupt download is an error");
    assert!(!asset.path.exists(), "and it is not kept");
}
