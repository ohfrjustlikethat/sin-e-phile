//! Resumable HTTP downloads.
//!
//! The IMDb datasets are hundreds of megabytes each. A download that must complete
//! in one attempt is one that fails on a laptop that sleeps, a hotel connection, or
//! a server that closes an idle socket — and then starts again from zero.
//!
//! # How resumption works
//!
//! Bytes land in `<name>.part`. On restart, its length is the offset to ask for with
//! an HTTP `Range` header. The server answers `206 Partial Content` and sends the
//! rest. The file is renamed to its final name only once the download is complete,
//! so **a file without a `.part` suffix is always whole** — nothing downstream has
//! to wonder whether it got a truncated dataset.
//!
//! Not every server honours `Range`. One that ignores it answers `200 OK` with the
//! whole body, which would append a second copy onto the first if it were treated
//! as a continuation. That case is detected by the status code and starts over
//! rather than producing a corrupt file, which is the failure that would otherwise
//! surface much later as an unparseable line halfway through ingestion.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;

use crate::job::JobError;

/// How much has arrived, for progress reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub downloaded: u64,
    /// From `Content-Length`, plus whatever was already on disk. `None` when the
    /// server does not say — which is normal for a chunked response.
    pub total: Option<u64>,
    pub resumed_from: u64,
}

impl Progress {
    pub fn percent(&self) -> Option<f64> {
        self.total
            .filter(|t| *t > 0)
            .map(|t| self.downloaded as f64 * 100.0 / t as f64)
    }
}

/// What a completed download did. Reported rather than logged so the caller can
/// record honest numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Downloaded {
    pub path: PathBuf,
    pub bytes: u64,
    pub resumed_from: u64,
    pub elapsed: Duration,
    /// False when the file was already present and complete.
    pub fetched: bool,
}

impl Downloaded {
    pub fn bytes_per_second(&self) -> f64 {
        let seconds = self.elapsed.as_secs_f64();
        if seconds <= 0.0 {
            return 0.0;
        }
        (self.bytes - self.resumed_from) as f64 / seconds
    }
}

pub struct Downloader {
    client: reqwest::Client,
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

impl Downloader {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            // Identify honestly. A dataset host is entitled to know what is asking,
            // and an anonymous scraper-shaped request is the one that gets blocked.
            .user_agent(concat!("sin-e-phile/", env!("CARGO_PKG_VERSION")))
            // Connect timeout only. A read timeout would kill a slow-but-progressing
            // multi-hundred-megabyte transfer, which is the normal case here.
            .connect_timeout(Duration::from_secs(30))
            // Datasets are already gzip; asking for transport compression on top
            // wastes CPU at both ends for nothing.
            .no_gzip()
            .build()
            .expect("a client with no TLS backend cannot be built");
        Self { client }
    }

    /// Download `url` to `destination`, resuming if a `.part` file is there.
    ///
    /// `on_progress` is called as bytes arrive; it should be cheap.
    pub async fn fetch(
        &self,
        url: &str,
        destination: &Path,
        mut on_progress: impl FnMut(Progress),
    ) -> Result<Downloaded, JobError> {
        let started = Instant::now();

        if destination.is_file() {
            let bytes = std::fs::metadata(destination)?.len();
            return Ok(Downloaded {
                path: destination.to_path_buf(),
                bytes,
                resumed_from: bytes,
                elapsed: started.elapsed(),
                fetched: false,
            });
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let partial = destination.with_extension("part");
        let mut have = if partial.is_file() {
            std::fs::metadata(&partial)?.len()
        } else {
            0
        };

        let mut request = self.client.get(url);
        if have > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| JobError::step("download", format!("{url}: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(JobError::step(
                "download",
                format!("{url}: server said {status}"),
            ));
        }

        // A server that ignored the Range header sends 200 with the WHOLE body.
        // Appending that to what is already on disk produces a file that is
        // plausible in size and corrupt in the middle, which surfaces much later as
        // an unparseable line. Start over instead.
        let resuming = have > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if have > 0 && !resuming {
            tracing::warn!(
                "{url}: the server ignored our Range request, so the partial download \
                 is discarded and this starts from the beginning"
            );
            have = 0;
        }

        let total = response
            .content_length()
            .map(|remaining| remaining + if resuming { have } else { 0 });

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(!resuming)
            .open(&partial)?;
        if resuming {
            file.seek(SeekFrom::End(0))?;
        }

        let resumed_from = have;
        let mut downloaded = have;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| JobError::step("download", format!("{url}: {e}")))?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            on_progress(Progress {
                downloaded,
                total,
                resumed_from,
            });
        }

        // Flush and close before the rename, so the final file is never a handle
        // someone else is still writing to.
        file.flush()?;
        drop(file);
        std::fs::rename(&partial, destination)?;

        Ok(Downloaded {
            path: destination.to_path_buf(),
            bytes: downloaded,
            resumed_from,
            elapsed: started.elapsed(),
            fetched: true,
        })
    }
}

/// Verify a gzip file decompresses cleanly end to end.
///
/// A truncated or corrupt download frequently parses for a long while before
/// failing, so the cheap check happens up front rather than three hundred thousand
/// rows into a load. Returns the decompressed size, which is what the size budget in
/// exit criterion E1 is actually about.
pub fn verify_gzip(path: &Path) -> Result<u64, JobError> {
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut decoder = flate2::read::MultiGzDecoder::new(std::io::BufReader::new(file));
    let mut buffer = vec![0u8; 1 << 16];
    let mut total = 0u64;

    loop {
        match decoder.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => total += n as u64,
            Err(e) => {
                return Err(JobError::step(
                    "verify",
                    format!("{} is not a complete gzip file: {e}", path.display()),
                ))
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_is_none_without_a_total() {
        let progress = Progress {
            downloaded: 10,
            total: None,
            resumed_from: 0,
        };
        assert!(progress.percent().is_none(), "an unknown total is not 0%");
    }

    #[test]
    fn rate_excludes_bytes_that_were_already_on_disk() {
        // Otherwise a resumed download reports a spectacular and fictional speed.
        let done = Downloaded {
            path: PathBuf::from("x"),
            bytes: 1_000,
            resumed_from: 900,
            elapsed: Duration::from_secs(1),
            fetched: true,
        };
        assert_eq!(done.bytes_per_second(), 100.0);
    }

    #[test]
    fn a_truncated_gzip_is_rejected() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("truncated.gz");

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(b"tconst\ttitle\ntt0000001\tSomething\n")
            .expect("write");
        let complete = encoder.finish().expect("finish");

        std::fs::write(&path, &complete[..complete.len() - 4]).expect("write truncated");
        assert!(
            verify_gzip(&path).is_err(),
            "a truncated download must be caught before it is parsed"
        );

        std::fs::write(&path, &complete).expect("write whole");
        assert!(verify_gzip(&path).is_ok());
    }
}
