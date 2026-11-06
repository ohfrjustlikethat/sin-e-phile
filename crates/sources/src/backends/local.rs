//! `LocalFileBackend`: films already on this machine — a stub until Phase 12.
//!
//! Phase 12 builds the library scanner and the filename parser; until then there is
//! no index of local files to search, so `find` answers with nothing. It is registered
//! now so that the registry, the order and the UI treat "your own copy" as a source
//! like any other from the first day, which is SPEC.md §6.1's whole point: a local
//! file appears next to the streaming options without special-casing.

use std::path::Path;

use source_protocol::{
    async_trait, BackendCapabilities, MediaKind, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError, Transport,
};

pub const ID: &str = "local";

#[derive(Default)]
pub struct LocalFileBackend;

#[async_trait]
impl SourceBackend for LocalFileBackend {
    fn id(&self) -> &str {
        ID
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film, MediaKind::Episode],
            transports: vec![Transport::Local],
            network: false,
        }
    }

    async fn find(&self, _query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        // Phase 12: look the query up in the scanned library.
        Ok(Vec::new())
    }

    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        // The key is the absolute path. Checked at resolve time, because a file found
        // an hour ago may have been moved or deleted since.
        let path = Path::new(&candidate.key);
        if path.is_absolute() && path.is_file() {
            Ok(PlayableStream::File {
                path: candidate.key.clone(),
            })
        } else {
            Err(SourceError::NotFound(format!(
                "{} is not there any more",
                candidate.key
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(key: &str) -> SourceCandidate {
        SourceCandidate {
            key: key.into(),
            title: "T".into(),
            year: None,
            transport: Transport::Local,
            quality: Default::default(),
            languages: vec![],
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            duration_seconds: None,
            health: Default::default(),
            locator: None,
            provenance: Default::default(),
        }
    }

    #[tokio::test]
    async fn resolves_a_file_that_exists_and_refuses_one_that_does_not() {
        let me = std::env::current_exe().unwrap();
        let ok = LocalFileBackend.resolve(&at(me.to_str().unwrap())).await;
        assert!(matches!(ok, Ok(PlayableStream::File { .. })));
        let gone = LocalFileBackend
            .resolve(&at("C:\\nowhere\\at\\all.mkv"))
            .await;
        assert!(matches!(gone, Err(SourceError::NotFound(_))));
        let relative = LocalFileBackend.resolve(&at("Cargo.toml")).await;
        assert!(
            matches!(relative, Err(SourceError::NotFound(_))),
            "relative paths are refused"
        );
    }
}
