//! `LocalFileBackend`: films and episodes already on this machine, from the library the
//! Phase 12 scanner builds.
//!
//! A query is turned back into the catalogue item it names (its IMDb id when the query
//! has one, else its title, year and kind; an episode through its series), and the
//! library's files matched to that item are offered. So a local file appears next to
//! the streaming options without special-casing, which is SPEC.md §6.1's whole point.

use std::path::Path;

use sinephile_persistence::repositories::{
    EpisodeRepository, LibraryRepository, LocalFile, MediaRepository, SearchRepository,
};
use sinephile_persistence::{Db, DbError, IdSource};
use source_protocol::{
    async_trait, BackendCapabilities, MediaKind, MediaQuery, PlayableStream, Quality, Resolution,
    SourceBackend, SourceCandidate, SourceError, Transport, VideoCodec,
};

pub const ID: &str = "local";

/// With no database (property tests, a database still opening) it finds nothing.
#[derive(Default)]
pub struct LocalFileBackend {
    db: Option<Db>,
}

impl LocalFileBackend {
    pub fn new(db: Option<Db>) -> Self {
        Self { db }
    }
}

/// The catalogue item a query names, if the catalogue has it.
async fn item_for(db: &Db, q: &MediaQuery) -> Result<Option<i64>, DbError> {
    let media = MediaRepository::new(db);
    let episode = q.kind == MediaKind::Episode;
    let mut id = match &q.ids.imdb {
        Some(tt) => media
            .by_external_id(IdSource::Imdb, tt)
            .await?
            .map(|m| m.id),
        None => None,
    };
    if id.is_none() {
        let kinds: &[&str] = if episode {
            &["series", "anime_series"]
        } else {
            &["film", "anime_film"]
        };
        id = SearchRepository::new(db)
            .exact_title(&q.title, 50)
            .await?
            .into_iter()
            .find(|h| {
                kinds.contains(&h.kind.as_str())
                    && (q.year.is_none() || h.year == q.year.map(i64::from))
            })
            .map(|h| h.media_item_id);
    }
    let Some(id) = id else { return Ok(None) };
    if let (true, Some(season), Some(number)) = (episode, q.season, q.episode) {
        return Ok(EpisodeRepository::new(db)
            .resolve_seasonal(id, IdSource::Imdb, i64::from(season), i64::from(number))
            .await?
            .map(|e| e.episode_id));
    }
    Ok(Some(id))
}

fn candidate(q: &MediaQuery, f: &LocalFile) -> SourceCandidate {
    let resolution = match f.height {
        Some(h) if h >= 1600 => Resolution::Uhd,
        Some(h) if h >= 900 => Resolution::FullHd,
        Some(h) if h >= 650 => Resolution::Hd,
        Some(_) => Resolution::Sd,
        None => Resolution::Unknown,
    };
    let codec = match f.video_codec.as_deref() {
        Some("h264") => VideoCodec::H264,
        Some("hevc") => VideoCodec::H265,
        Some("av1") => VideoCodec::Av1,
        Some("vp9") => VideoCodec::Vp9,
        Some("mpeg4") => VideoCodec::Mpeg4,
        _ => VideoCodec::Unknown,
    };
    SourceCandidate {
        key: f.path.clone(),
        title: q.title.clone(),
        year: q.year,
        transport: Transport::Local,
        quality: Quality {
            resolution,
            hdr: false,
        },
        languages: f.audio_languages.clone(),
        subtitles: Vec::new(),
        codec,
        size_bytes: u64::try_from(f.size_bytes).ok(),
        duration_seconds: f.duration_seconds.and_then(|d| u32::try_from(d).ok()),
        health: Default::default(),
        locator: Some(format!("file:{}", f.path)),
        provenance: Default::default(),
    }
}

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

    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        let Some(db) = &self.db else {
            return Ok(Vec::new());
        };
        let fail = |e: DbError| SourceError::Unavailable(e.to_string());
        let Some(item) = item_for(db, query).await.map_err(fail)? else {
            return Ok(Vec::new());
        };
        let files = LibraryRepository::new(db)
            .files_for_item(item)
            .await
            .map_err(fail)?;
        Ok(files
            .iter()
            // A file that has gone since the last scan is not offered.
            .filter(|f| Path::new(&f.path).is_file())
            .map(|f| candidate(query, f))
            .collect())
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
        let ok = LocalFileBackend::default()
            .resolve(&at(me.to_str().unwrap()))
            .await;
        assert!(matches!(ok, Ok(PlayableStream::File { .. })));
        let gone = LocalFileBackend::default()
            .resolve(&at("C:\\nowhere\\at\\all.mkv"))
            .await;
        assert!(matches!(gone, Err(SourceError::NotFound(_))));
        let relative = LocalFileBackend::default().resolve(&at("Cargo.toml")).await;
        assert!(
            matches!(relative, Err(SourceError::NotFound(_))),
            "relative paths are refused"
        );
    }
}
