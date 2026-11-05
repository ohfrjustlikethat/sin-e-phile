//! The values that cross the `SourceBackend` boundary.
//!
//! Every type here is also a **wire format**: an addon is a remote service, so these
//! structs are what it receives and returns as JSON (`docs/specs/addon-protocol.md`).
//! That is why they are plain data with serde names fixed in `snake_case`, and why
//! every enum has an `Unknown` arm: a newer addon sending a value this build has never
//! heard of must degrade to "unknown", never fail the whole response.

use serde::{Deserialize, Serialize};

/// What is being looked for. Built by the app from a catalogue row; a backend never
/// sees the app's internal ids, only identifiers the outside world shares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaQuery {
    pub kind: MediaKind,
    /// The primary title, as the catalogue holds it.
    pub title: String,
    pub year: Option<u16>,
    #[serde(default)]
    pub ids: ExternalIds,
    /// Present only for an episode.
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
}

impl MediaQuery {
    pub fn film(title: impl Into<String>, year: Option<u16>) -> Self {
        Self {
            kind: MediaKind::Film,
            title: title.into(),
            year,
            ids: ExternalIds::default(),
            season: None,
            episode: None,
        }
    }
}

/// Identifiers shared with the outside world (SPEC.md §6.2). All optional: the app
/// sends what the catalogue knows, and a backend uses what it understands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalIds {
    /// `tt` followed by digits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imdb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anilist: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Film,
    Episode,
    #[serde(other)]
    Unknown,
}

/// How the bytes of a candidate would travel. The resolver does not care; Phase 9's
/// scorer and the player do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// A file on this machine.
    Local,
    /// An HTTP(S) URL that serves the file.
    Http,
    /// A peer-to-peer swarm, played through the torrent engine (Phase 7).
    P2p,
    #[serde(other)]
    Unknown,
}

/// One way to watch the thing asked for. A local file, a swarm and a direct link are
/// all candidates, which is what lets one scoring function rank them (SPEC.md §6.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceCandidate {
    /// Opaque to everyone but the backend that made it: what `resolve` is given back.
    pub key: String,
    /// What the source calls it — which is not always what was asked for.
    pub title: String,
    #[serde(default)]
    pub year: Option<u16>,
    pub transport: Transport,
    #[serde(default)]
    pub quality: Quality,
    /// Audio languages, BCP 47 (`en`, `ja`, `pt-BR`). Empty means not stated.
    #[serde(default)]
    pub languages: Vec<String>,
    /// Subtitle languages that come with it, BCP 47.
    #[serde(default)]
    pub subtitles: Vec<String>,
    #[serde(default)]
    pub codec: VideoCodec,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub health: Health,
    /// The same bytes reached two ways are one candidate. When a backend can name
    /// what the bytes ARE — a URL, a path, an info hash — it says so here, and the
    /// resolver merges on it. `None` means "never merge me".
    #[serde(default)]
    pub locator: Option<String>,
    /// Filled in by the resolver, never trusted from a backend.
    #[serde(default)]
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quality {
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default)]
    pub hdr: bool,
}

/// Coarse on purpose: four buckets are what a viewer chooses between.
//
// No `Ord`: `Unknown` must be the last variant for `#[serde(other)]`, so declaration
// order is not quality order. `rank` is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Up to 576 lines.
    Sd,
    /// 720p.
    Hd,
    /// 1080p.
    FullHd,
    /// 2160p.
    Uhd,
    #[default]
    #[serde(other)]
    Unknown,
}

impl Resolution {
    /// Higher is better; `Unknown` is lowest.
    pub fn rank(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Sd => 1,
            Self::Hd => 2,
            Self::FullHd => 3,
            Self::Uhd => 4,
        }
    }

    /// From a frame height in pixels.
    pub fn from_height(h: u32) -> Self {
        match h {
            0 => Self::Unknown,
            1..=576 => Self::Sd,
            577..=720 => Self::Hd,
            721..=1080 => Self::FullHd,
            _ => Self::Uhd,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    H264,
    H265,
    Av1,
    Vp9,
    /// MPEG-4 Part 2 — what much of the Internet Archive's older material is.
    Mpeg4,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Will it actually play? What the evidence is depends on the transport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Health {
    /// Served by a host or a disk: available unless that one place is down.
    Direct,
    /// A swarm, as last counted.
    Swarm { seeders: u32, peers: u32 },
    #[default]
    #[serde(other)]
    Unknown,
}

/// Where a candidate came from, for the "why this source" line and for trust.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The backend's `id()`. Set by the resolver from the backend that answered, so a
    /// backend cannot claim to be another one.
    #[serde(default)]
    pub backend: String,
    /// Other backends that returned the same `locator`, after merging.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_from: Vec<String>,
    /// A human-readable page about the item, if the source has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// The licence the source states, e.g. `public domain`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
}

/// What `resolve` hands to playback: something the player or the torrent engine can
/// open with no further lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayableStream {
    Http {
        url: String,
        #[serde(default)]
        content_type: Option<String>,
        #[serde(default)]
        size_bytes: Option<u64>,
    },
    File {
        path: String,
    },
    /// Handed to the torrent engine (Phase 7), which turns it into a local HTTP stream.
    Torrent {
        /// 40 hex characters (BitTorrent v1 info hash).
        info_hash: String,
        /// Which file inside the torrent, when there is more than one.
        #[serde(default)]
        file_index: Option<u32>,
    },
}

/// What a backend says about itself, before it is asked anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilities {
    /// The kinds of item it can find. A query of any other kind is not sent to it.
    pub media: Vec<MediaKind>,
    /// The transports its candidates use.
    pub transports: Vec<Transport>,
    /// Whether it touches the network at all. An offline app skips those that do.
    pub network: bool,
}

impl BackendCapabilities {
    pub fn handles(&self, kind: MediaKind) -> bool {
        self.media.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_enum_value_degrades_rather_than_failing_the_response() {
        // A newer addon may send values this build has never heard of.
        let c: SourceCandidate = serde_json::from_str(
            r#"{"key":"k","title":"T","transport":"carrier_pigeon",
                "quality":{"resolution":"8k"},"codec":"h266","health":{"kind":"vibes"}}"#,
        )
        .unwrap();
        assert_eq!(c.transport, Transport::Unknown);
        assert_eq!(c.quality.resolution, Resolution::Unknown);
        assert_eq!(c.codec, VideoCodec::Unknown);
        assert_eq!(c.health, Health::Unknown);
    }

    #[test]
    fn a_minimal_candidate_needs_only_key_title_and_transport() {
        let c: SourceCandidate =
            serde_json::from_str(r#"{"key":"k","title":"T","transport":"http"}"#).unwrap();
        assert_eq!(c.locator, None);
        assert!(c.languages.is_empty());
        assert_eq!(c.provenance, Provenance::default());
    }

    #[test]
    fn resolution_buckets_by_frame_height() {
        assert_eq!(Resolution::from_height(480), Resolution::Sd);
        assert_eq!(Resolution::from_height(576), Resolution::Sd);
        assert_eq!(Resolution::from_height(720), Resolution::Hd);
        assert_eq!(Resolution::from_height(1080), Resolution::FullHd);
        assert_eq!(Resolution::from_height(2160), Resolution::Uhd);
        assert!(Resolution::Uhd.rank() > Resolution::Sd.rank());
        assert_eq!(Resolution::Unknown.rank(), 0);
    }

    #[test]
    fn streams_are_tagged_by_kind_on_the_wire() {
        let s = PlayableStream::Torrent {
            info_hash: "0".repeat(40),
            file_index: Some(2),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.starts_with(r#"{"kind":"torrent""#), "{json}");
        assert_eq!(serde_json::from_str::<PlayableStream>(&json).unwrap(), s);
    }
}
