//! From a catalogue item to something playing — SPEC.md Phase 8: "Wire search → detail
//! → resolve → play end to end."
//!
//! [`plan`] does everything up to the first frame: build the query from the catalogue,
//! ask every source, rank what comes back, resolve candidates in order until one works,
//! turn a torrent into a local stream, and look up where the viewer left off. The detail
//! page calls it **when it opens**, not when Play is pressed: the sources are asked and a
//! torrent's peers are found while the viewer reads, so Play has nothing left to wait
//! for (Phase 7 E1, carried here).
//!
//! The ranking is deliberately simple. SPEC.md Phase 9 is where sources are scored
//! properly; this only has to pick something sensible today.

use std::sync::Arc;

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError, MediaKind};
use sinephile_sources::{registry, Resolver};
use sinephile_torrent::play::{target, Target};
use sinephile_torrent::{Engine, StreamServer, Wanted};
use source_protocol::{
    ExternalIds, Health, MediaKind as Kind, MediaQuery, SourceCandidate, Transport,
};

#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    #[error("this is not in the catalogue")]
    Unknown,
    #[error("none of your sources has this")]
    NoSources,
    #[error("found {0} sources, but none could be opened: {1}")]
    NothingPlayable(usize, String),
    #[error("{0}")]
    Store(#[from] DbError),
}

/// Everything the player needs, decided before Play is pressed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayPlan {
    pub media_item_id: i64,
    pub title: String,
    /// The address or path the player opens.
    pub open: String,
    /// Where the viewer left off, if they did.
    pub start: Option<f64>,
    /// The source's name for the "playing from …" line.
    pub backend: String,
    /// How many candidates the sources offered.
    pub offered: usize,
}

/// The catalogue item as the query sources receive.
pub async fn query_for(db: &Db, media_item_id: i64) -> Result<Option<MediaQuery>, DbError> {
    let media = MediaRepository::new(db);
    let Some(item) = media.by_id(media_item_id).await? else {
        return Ok(None);
    };
    let kind = match item.kind {
        MediaKind::Episode => Kind::Episode,
        _ => Kind::Film,
    };
    Ok(Some(MediaQuery {
        kind,
        title: item.primary_title,
        year: item.release_year.and_then(|y| u16::try_from(y).ok()),
        ids: ExternalIds {
            imdb: media.external_id(media_item_id, "imdb").await?,
            tmdb: media
                .external_id(media_item_id, "tmdb")
                .await?
                .and_then(|v| v.parse().ok()),
            anilist: media
                .external_id(media_item_id, "anilist")
                .await?
                .and_then(|v| v.parse().ok()),
        },
        season: None,
        episode: None,
    }))
}

/// The interim order, until Phase 9 scores sources: your own file, then a direct
/// stream, then a swarm; within each, higher resolution first, then more seeders.
pub fn rank(mut candidates: Vec<SourceCandidate>) -> Vec<SourceCandidate> {
    let transport = |c: &SourceCandidate| match c.transport {
        Transport::Local => 0,
        Transport::Http => 1,
        Transport::P2p => 2,
        Transport::Unknown => 3,
    };
    let seeders = |c: &SourceCandidate| match c.health {
        Health::Swarm { seeders, .. } => seeders,
        _ => 0,
    };
    candidates.sort_by(|a, b| {
        transport(a)
            .cmp(&transport(b))
            .then(
                b.quality
                    .resolution
                    .rank()
                    .cmp(&a.quality.resolution.rank()),
            )
            .then(seeders(b).cmp(&seeders(a)))
    });
    candidates
}

/// Where to start: the saved position, unless it is in the first 10 seconds (start
/// over) or the last 3 minutes (it was finished).
pub fn resume_from(position: i64, duration: Option<i64>) -> Option<f64> {
    if position < 10 {
        return None;
    }
    if let Some(d) = duration {
        if d > 0 && position > d - 180 {
            return None;
        }
    }
    Some(position as f64)
}

/// See the module note. `torrent` is the engine and its server, for a candidate that is
/// a swarm; `None` skips swarms (the engine could not start).
pub async fn plan(
    db: &Db,
    resolver: &Resolver,
    torrent: Option<(&Arc<Engine>, &StreamServer)>,
    media_item_id: i64,
) -> Result<PlayPlan, PlayError> {
    let query = query_for(db, media_item_id)
        .await?
        .ok_or(PlayError::Unknown)?;
    let found = resolver.find_all(&query).await;
    registry::record(db, &found.outcomes).await?;
    let ranked = rank(found.candidates);
    if ranked.is_empty() {
        return Err(PlayError::NoSources);
    }
    let offered = ranked.len();
    let mut reasons = Vec::new();
    for candidate in ranked {
        let stream = match resolver.resolve(&candidate).await {
            Ok(s) => s,
            Err(e) => {
                reasons.push(format!("{}: {e}", candidate.provenance.backend));
                continue;
            }
        };
        let opened = match (&stream, torrent) {
            (source_protocol::PlayableStream::Torrent { .. }, None) => {
                reasons.push("a torrent, but the torrent engine is not running".into());
                continue;
            }
            (_, Some((engine, server))) => target(engine, server, &stream, Wanted::default()).await,
            (_, None) => match &stream {
                source_protocol::PlayableStream::Http { url, .. } => Ok(Target::Url(url.clone())),
                source_protocol::PlayableStream::File { path } => Ok(Target::File(path.into())),
                source_protocol::PlayableStream::Torrent { .. } => unreachable!("handled above"),
            },
        };
        let open = match opened {
            Ok(Target::Url(u)) => u,
            Ok(Target::File(p)) => p.to_string_lossy().into_owned(),
            Err(e) => {
                reasons.push(format!("{}: {e}", candidate.provenance.backend));
                continue;
            }
        };
        let profile = ProfileRepository::new(db).ensure_default().await?;
        let start = ProfileRepository::new(db)
            .position(profile, media_item_id)
            .await?
            .and_then(|p| resume_from(p.position_seconds, p.duration_seconds));
        return Ok(PlayPlan {
            media_item_id,
            title: query.title,
            open,
            start,
            backend: candidate.provenance.backend,
            offered,
        });
    }
    Err(PlayError::NothingPlayable(offered, reasons.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_protocol::{Provenance, Quality, Resolution};

    fn cand(t: Transport, r: Resolution, seeders: u32) -> SourceCandidate {
        SourceCandidate {
            key: format!("{t:?}-{r:?}-{seeders}"),
            title: "T".into(),
            year: None,
            transport: t,
            quality: Quality {
                resolution: r,
                hdr: false,
            },
            languages: vec![],
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            health: if t == Transport::P2p {
                Health::Swarm { seeders, peers: 0 }
            } else {
                Health::Direct
            },
            locator: None,
            provenance: Provenance::default(),
        }
    }

    #[test]
    fn your_own_file_first_then_streams_then_swarms() {
        let ranked = rank(vec![
            cand(Transport::P2p, Resolution::Uhd, 500),
            cand(Transport::Http, Resolution::Sd, 0),
            cand(Transport::P2p, Resolution::Uhd, 900),
            cand(Transport::Local, Resolution::Hd, 0),
            cand(Transport::Http, Resolution::FullHd, 0),
        ]);
        let keys: Vec<&str> = ranked.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "Local-Hd-0",
                "Http-FullHd-0",
                "Http-Sd-0",
                "P2p-Uhd-900",
                "P2p-Uhd-500"
            ]
        );
    }

    #[test]
    fn resume_skips_the_first_seconds_and_the_credits() {
        assert_eq!(resume_from(5, Some(6000)), None, "barely started");
        assert_eq!(resume_from(1800, Some(6000)), Some(1800.0));
        assert_eq!(resume_from(5900, Some(6000)), None, "finished");
        assert_eq!(resume_from(1800, None), Some(1800.0));
    }
}
