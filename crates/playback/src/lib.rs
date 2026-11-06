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
//! The order candidates are tried in is `sinephile_selection`'s: every candidate is
//! scored against what this machine plays and what the viewer chose (SPEC.md Phase 9).

use std::sync::Arc;

use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError, MediaKind};
pub use sinephile_selection::{Context, Machine, Score};
use sinephile_sources::{registry, Resolver};
use sinephile_torrent::play::{target, Target};
use sinephile_torrent::{Engine, StreamServer, Wanted};
use source_protocol::{ExternalIds, MediaKind as Kind, MediaQuery};

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
    /// Why this copy: its score, term by term.
    pub score: Score,
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

/// What this machine plays well, from what `tiers.rs` permits and found. AV1 decode and
/// an HDR display are not probed yet, so both are assumed absent: the cautious guess
/// costs a little ranking, never a stutter.
pub fn machine(high_res_playback: bool, hardware_decode: bool) -> Machine {
    Machine {
        max_resolution: if high_res_playback {
            source_protocol::Resolution::Uhd
        } else {
            source_protocol::Resolution::FullHd
        },
        hevc: hardware_decode,
        av1: false,
        hdr_display: false,
    }
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

/// [`plan_with`] on a Tier 0 machine with nothing chosen.
pub async fn plan(
    db: &Db,
    resolver: &Resolver,
    torrent: Option<(&Arc<Engine>, &StreamServer)>,
    media_item_id: i64,
) -> Result<PlayPlan, PlayError> {
    plan_with(db, resolver, torrent, media_item_id, Context::default()).await
}

/// See the module note. `torrent` is the engine and its server, for a candidate that is
/// a swarm; `None` skips swarms (the engine could not start). `ctx` says what the
/// machine plays and what the viewer chose; the catalogue's runtime is filled in here.
pub async fn plan_with(
    db: &Db,
    resolver: &Resolver,
    torrent: Option<(&Arc<Engine>, &StreamServer)>,
    media_item_id: i64,
    mut ctx: Context,
) -> Result<PlayPlan, PlayError> {
    let query = query_for(db, media_item_id)
        .await?
        .ok_or(PlayError::Unknown)?;
    ctx.runtime_minutes = MediaRepository::new(db)
        .by_id(media_item_id)
        .await?
        .and_then(|item| item.runtime_minutes)
        .and_then(|m| u32::try_from(m).ok());
    let found = resolver.find_all(&query).await;
    registry::record(db, &found.outcomes).await?;
    let ranked = sinephile_selection::rank(found.candidates, &ctx);
    if ranked.is_empty() {
        return Err(PlayError::NoSources);
    }
    let offered = ranked.len();
    let mut reasons = Vec::new();
    for (candidate, score) in ranked {
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
            score,
        });
    }
    Err(PlayError::NothingPlayable(offered, reasons.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_without_high_res_playback_tops_out_at_1080p() {
        use source_protocol::Resolution;
        assert_eq!(machine(false, true).max_resolution, Resolution::FullHd);
        assert_eq!(machine(true, true).max_resolution, Resolution::Uhd);
        assert!(!machine(true, false).hevc);
    }

    #[test]
    fn resume_skips_the_first_seconds_and_the_credits() {
        assert_eq!(resume_from(5, Some(6000)), None, "barely started");
        assert_eq!(resume_from(1800, Some(6000)), Some(1800.0));
        assert_eq!(resume_from(5900, Some(6000)), None, "finished");
        assert_eq!(resume_from(1800, None), Some(1800.0));
    }
}
