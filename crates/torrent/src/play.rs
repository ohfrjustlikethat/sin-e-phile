//! From "play this torrent" to an address the player can open.
//!
//! The resolver hands playback a `PlayableStream::Torrent` (SPEC.md §6.1). This is the
//! step after: make sure the engine has the torrent, pick the file, and publish it on the
//! local streaming server. The player then treats a swarm exactly like a file on disk.

use std::sync::Arc;

use crate::engine::{AddOptions, Engine, Source, TorrentError};
use crate::select::{TorrentFile, Wanted};
use crate::server::StreamServer;

/// What the player needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub id: String,
    pub url: String,
    pub file: TorrentFile,
}

/// Add (or find) the torrent, choose the file, and publish it.
///
/// `file_index`, when the source named one, wins over the selection rules: the source
/// knows which file it meant. Otherwise `select::choose` picks the feature or episode.
pub async fn prepare(
    engine: &Arc<Engine>,
    server: &StreamServer,
    source: Source,
    file_index: Option<usize>,
    wanted: Wanted,
    opts: AddOptions,
) -> Result<Prepared, TorrentError> {
    let id = engine.add(source, opts).await?;
    let file = match file_index {
        Some(i) => engine
            .files(&id)?
            .into_iter()
            .find(|f| f.index == i)
            .ok_or_else(|| TorrentError::BadInput(format!("the torrent has no file {i}")))?,
        None => engine.main_file(&id, wanted)?,
    };
    let route = format!("{}-{}", &id[..16], file.index);
    let url = server.publish(&route, engine.stream_source(&id, &file)?);
    Ok(Prepared { id, url, file })
}

/// What the player opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Url(String),
    File(std::path::PathBuf),
}

/// Turn whatever the resolver produced into something the player can open: an HTTP
/// stream passes through, a file is a path, and a torrent goes through the engine and
/// the local server. A torrent the engine already holds is not added again — so a film
/// already downloaded plays from disk without a single network request.
pub async fn target(
    engine: &Arc<Engine>,
    server: &StreamServer,
    stream: &source_protocol::PlayableStream,
    wanted: Wanted,
) -> Result<Target, TorrentError> {
    use source_protocol::PlayableStream as P;
    match stream {
        P::Http { url, .. } => Ok(Target::Url(url.clone())),
        P::File { path } => Ok(Target::File(path.into())),
        P::Torrent {
            info_hash,
            file_index,
            trackers,
        } => {
            let id = info_hash.to_ascii_lowercase();
            let index = file_index.map(|i| i as usize);
            if engine.has(&id) {
                let file = match index {
                    Some(i) => engine.files(&id)?.into_iter().find(|f| f.index == i),
                    None => Some(engine.main_file(&id, wanted)?),
                }
                .ok_or(TorrentError::NoVideo)?;
                let route = format!("{}-{}", &id[..16], file.index);
                return Ok(Target::Url(
                    server.publish(&route, engine.stream_source(&id, &file)?),
                ));
            }
            let source = Source::InfoHash {
                hash: id.clone(),
                trackers: trackers.clone(),
            };
            let p = prepare(engine, server, source, index, wanted, AddOptions::default()).await?;
            Ok(Target::Url(p.url))
        }
    }
}
