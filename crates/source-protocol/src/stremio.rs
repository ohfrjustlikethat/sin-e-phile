//! Stremio's add-on protocol, spoken beside the app's own (ADR-0047).
//!
//! A Stremio add-on is a service with a `manifest.json`; the app asks it
//! `GET <base>/stream/<movie|series>/<id>.json`, where the id is IMDb's (`tt0068646`, or
//! `tt0903747:1:2` for a series' season 1, episode 2), and reads back a list of streams:
//! a torrent's info hash (with the file inside it and trackers to find the swarm) or a
//! web address. Nothing an add-on sends is trusted: the manifest and every answer are
//! size-capped and depth-capped like the app's own protocol, each stream is checked by
//! `wire::check_stream` before it can become a candidate, and text is stripped of
//! control and invisible characters. Streams the app cannot play in its own player (a
//! page to open elsewhere, a video site's id, one that needs request headers) are left
//! out rather than offered and failed.
//!
//! The app ships no add-on and names none (SPEC.md §2.1): this is the protocol only.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

use crate::manifest::{
    bounded_json, check_text, is_deceptive, malformed, ManifestError, MAX_DESCRIPTION_CHARS,
    MAX_MANIFEST_BYTES, MAX_NAME_CHARS, MAX_VERSION_CHARS,
};
use crate::types::{
    Health, MediaKind, MediaQuery, PlayableStream, Provenance, Quality, Resolution,
    SourceCandidate, Transport, VideoCodec,
};
use crate::wire::{check_stream, MAX_TITLE_CHARS, MAX_TRACKERS};

/// An answer's size limit: torrent add-ons list a great many streams.
pub const MAX_STREAMS_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
/// Streams kept from one answer, best first as the add-on ordered them.
pub const MAX_STREAMS: usize = 80;
/// Trackers carried in a candidate's key: enough to find the swarm, short enough to keep.
const KEY_TRACKERS: usize = 12;

/// A Stremio add-on's manifest, as far as the app uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StremioManifest {
    /// Its id, lower-cased (`org.example.films`).
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    /// The content types it serves streams for that the app plays: `movie`, `series`.
    pub stream_types: Vec<String>,
    /// The id prefixes it answers for (`tt`); `None`: any.
    pub id_prefixes: Option<Vec<String>>,
    /// Where requests go: the manifest's address without `manifest.json`. Configured
    /// add-ons keep their settings in this path, so it is kept whole.
    pub base: Url,
}

fn field(name: &'static str, problem: impl Into<String>) -> ManifestError {
    ManifestError::Field {
        field: name,
        problem: problem.into(),
    }
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    let list = v?.as_array()?;
    Some(
        list.iter()
            .take(16)
            .filter_map(|s| s.as_str())
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty() && s.len() <= 32)
            .collect(),
    )
}

/// Text from an add-on, made safe to show: control and invisible characters gone, lines
/// joined, capped.
fn display(raw: &str, max: usize) -> String {
    let joined: String = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    joined
        .chars()
        .filter(|&c| !c.is_control() && !is_deceptive(c))
        .take(max)
        .collect()
}

impl StremioManifest {
    /// Parse and check a manifest fetched from `origin`, which must end in
    /// `manifest.json` (Stremio's own rule).
    pub fn parse(bytes: &[u8], origin: &Url) -> Result<Self, ManifestError> {
        let text = bounded_json(bytes, MAX_MANIFEST_BYTES)?;
        let v: Value = serde_json::from_str(text).map_err(malformed)?;
        let obj = v
            .as_object()
            .ok_or_else(|| ManifestError::Malformed("the manifest is not a JSON object".into()))?;
        let text_of = |key: &'static str| obj.get(key).and_then(Value::as_str).unwrap_or("");

        let id = text_of("id").trim().to_ascii_lowercase();
        if !(3..=64).contains(&id.len())
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            return Err(field(
                "id",
                "must be 3 to 64 letters, digits, dots, dashes or underscores",
            ));
        }
        let name = text_of("name").trim().to_string();
        check_text("name", &name, 1, MAX_NAME_CHARS)?;
        let version = text_of("version").trim().to_string();
        check_text("version", &version, 1, MAX_VERSION_CHARS)?;
        let description = display(text_of("description"), MAX_DESCRIPTION_CHARS);

        let hints = obj.get("behaviorHints").and_then(Value::as_object);
        if hints
            .and_then(|h| h.get("configurationRequired"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Err(field(
                "behaviorHints",
                "says the add-on must be set up first: open its own page, set it up, and paste \
                 the address it gives you then",
            ));
        }

        let types = strings(obj.get("types")).unwrap_or_default();
        let prefixes = strings(obj.get("idPrefixes"));
        let resources = obj
            .get("resources")
            .and_then(Value::as_array)
            .ok_or_else(|| field("resources", "is missing"))?;
        let stream = resources.iter().take(16).find_map(|r| match r {
            Value::String(s) if s == "stream" => Some((types.clone(), prefixes.clone())),
            Value::Object(o) if o.get("name").and_then(Value::as_str) == Some("stream") => Some((
                strings(o.get("types")).unwrap_or_else(|| types.clone()),
                strings(o.get("idPrefixes")).or_else(|| prefixes.clone()),
            )),
            _ => None,
        });
        let Some((stream_types, id_prefixes)) = stream else {
            return Err(field(
                "resources",
                "offers no streams: it may list or describe titles, but this app needs streams",
            ));
        };
        let stream_types: Vec<String> = stream_types
            .into_iter()
            .filter(|t| t == "movie" || t == "series")
            .collect();
        if stream_types.is_empty() {
            return Err(field(
                "types",
                "names neither films (movie) nor series, the only kinds this app plays",
            ));
        }
        if let Some(p) = &id_prefixes {
            if !p.iter().any(|p| imdb_prefix(p)) {
                return Err(field(
                    "idPrefixes",
                    "asks by ids this app does not have; it knows IMDb's (tt…)",
                ));
            }
        }

        let path = origin.path();
        let Some(dir) = path.strip_suffix("manifest.json") else {
            return Err(ManifestError::Url {
                url: origin.as_str().chars().take(80).collect(),
                problem: "does not end in manifest.json, as a Stremio add-on's address does".into(),
            });
        };
        let mut base = origin.clone();
        base.set_query(None);
        base.set_fragment(None);
        base.set_path(dir);

        Ok(Self {
            id,
            name,
            version,
            description,
            stream_types,
            id_prefixes,
            base,
        })
    }

    /// What is stored once the user confirms: a manifest of the same shape, holding only
    /// what was checked, so that `parse` reads it back the same.
    pub fn to_json(&self) -> String {
        let mut stream = Map::new();
        stream.insert("name".into(), "stream".into());
        stream.insert("types".into(), self.stream_types.clone().into());
        if let Some(p) = &self.id_prefixes {
            stream.insert("idPrefixes".into(), p.clone().into());
        }
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "version": self.version,
            "description": self.description,
            "types": self.stream_types,
            "resources": [Value::Object(stream)],
        })
        .to_string()
    }

    /// The kinds of item it can be asked for.
    pub fn media(&self) -> Vec<MediaKind> {
        let mut out = Vec::new();
        if self.stream_types.iter().any(|t| t == "movie") {
            out.push(MediaKind::Film);
        }
        if self.stream_types.iter().any(|t| t == "series") {
            out.push(MediaKind::Episode);
        }
        out
    }

    /// The address to ask for `q`'s streams, or `None` when this add-on cannot be asked
    /// for it: no IMDb id, a kind it does not serve, an episode without its numbers.
    pub fn stream_url(&self, q: &MediaQuery) -> Option<Url> {
        let imdb = q.ids.imdb.as_deref()?;
        if !(imdb.starts_with("tt")
            && imdb.len() <= 12
            && imdb[2..].chars().all(|c| c.is_ascii_digit()))
        {
            return None;
        }
        let (kind, id) = match q.kind {
            MediaKind::Film => ("movie", imdb.to_string()),
            MediaKind::Episode => ("series", format!("{imdb}:{}:{}", q.season?, q.episode?)),
            MediaKind::Unknown => return None,
        };
        if !self.stream_types.iter().any(|t| t == kind) {
            return None;
        }
        self.base.join(&format!("stream/{kind}/{id}.json")).ok()
    }
}

/// Whether a prefix can match IMDb's ids ("tt", "t", or "tt" itself narrowed).
fn imdb_prefix(p: &str) -> bool {
    "tt".starts_with(p) || p.starts_with("tt")
}

/// What a candidate's key holds: the stream itself, since Stremio has no second step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "s")]
enum Key {
    #[serde(rename = "t")]
    Torrent {
        h: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        f: Option<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        t: Vec<String>,
    },
    #[serde(rename = "u")]
    Web {
        u: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<u64>,
    },
}

/// Read an add-on's answer to `stream_url`: the candidates, best first as the add-on
/// ordered them, and how many streams were dropped (unplayable here, or malformed).
pub fn parse_streams(
    bytes: &[u8],
    backend: &str,
    q: &MediaQuery,
) -> Result<(Vec<SourceCandidate>, usize), ManifestError> {
    let text = bounded_json(bytes, MAX_STREAMS_RESPONSE_BYTES)?;
    let v: Value = serde_json::from_str(text).map_err(malformed)?;
    let Some(list) = v.get("streams").and_then(Value::as_array) else {
        return Err(ManifestError::Malformed(
            "the answer has no `streams` list".into(),
        ));
    };
    let mut out = Vec::new();
    let mut dropped = 0;
    for s in list {
        if out.len() == MAX_STREAMS {
            dropped += 1;
            continue;
        }
        match candidate(s, backend, q) {
            Some(c) => out.push(c),
            None => dropped += 1,
        }
    }
    Ok((out, dropped))
}

/// The stream a candidate made by `parse_streams` plays, checked again.
pub fn stream_of(candidate: &SourceCandidate) -> Result<PlayableStream, ManifestError> {
    let key: Key = serde_json::from_str(&candidate.key).map_err(malformed)?;
    let stream = match key {
        Key::Torrent { h, f, t } => PlayableStream::Torrent {
            info_hash: h,
            file_index: f,
            trackers: t,
        },
        Key::Web { u, b } => PlayableStream::Http {
            url: u,
            content_type: None,
            size_bytes: b,
        },
    };
    check_stream(&stream)?;
    Ok(stream)
}

fn candidate(s: &Value, backend: &str, q: &MediaQuery) -> Option<SourceCandidate> {
    let o = s.as_object()?;
    let text_of = |key: &str| o.get(key).and_then(Value::as_str).unwrap_or("");
    let hints = o.get("behaviorHints").and_then(Value::as_object);
    // A stream that needs request headers cannot be opened by an address alone.
    if hints.is_some_and(|h| h.contains_key("proxyHeaders")) {
        return None;
    }
    let filename = hints
        .and_then(|h| h.get("filename"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let video_size = hints
        .and_then(|h| h.get("videoSize"))
        .and_then(Value::as_u64);

    let (key, transport, stream) = if let Some(hash) = o.get("infoHash").and_then(Value::as_str) {
        let h = hash.trim().to_ascii_lowercase();
        let f = o
            .get("fileIdx")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok());
        let t: Vec<String> = o
            .get("sources")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .filter_map(|s| s.strip_prefix("tracker:"))
                    .filter(|t| t.len() <= 512)
                    .map(str::to_string)
                    .take(MAX_TRACKERS)
                    .collect()
            })
            .unwrap_or_default();
        let stream = PlayableStream::Torrent {
            info_hash: h.clone(),
            file_index: f,
            trackers: t.clone(),
        };
        let key = Key::Torrent {
            h,
            f,
            t: t.into_iter().take(KEY_TRACKERS).collect(),
        };
        (key, Transport::P2p, stream)
    } else {
        // Without an address either, it is a video site's id or a page to open elsewhere:
        // not playable in this player.
        let url = o.get("url").and_then(Value::as_str)?;
        let stream = PlayableStream::Http {
            url: url.to_string(),
            content_type: None,
            size_bytes: video_size,
        };
        let key = Key::Web {
            u: url.to_string(),
            b: video_size,
        };
        (key, Transport::Http, stream)
    };
    check_stream(&stream).ok()?;
    let key = serde_json::to_string(&key).ok()?;
    if key.len() > 8 * 1024 {
        return None;
    }

    // Add-ons put quality, size and seeders in the stream's own text.
    let words = [
        text_of("name"),
        text_of("title"),
        text_of("description"),
        filename,
    ]
    .join("\n");
    let read = read_text(&words);
    let shown = [
        text_of("title"),
        text_of("description"),
        filename,
        text_of("name"),
    ]
    .into_iter()
    .find(|t| !t.trim().is_empty())
    .unwrap_or(&q.title);
    Some(SourceCandidate {
        key,
        title: display(shown, MAX_TITLE_CHARS),
        year: q.year,
        transport,
        quality: Quality {
            resolution: read.resolution,
            hdr: read.hdr,
        },
        languages: Vec::new(),
        subtitles: Vec::new(),
        codec: read.codec,
        size_bytes: video_size.or(read.size_bytes),
        duration_seconds: None,
        health: match transport {
            Transport::P2p => read.seeders.map_or(Health::Unknown, |n| Health::Swarm {
                seeders: n,
                peers: n,
            }),
            _ => Health::Direct,
        },
        locator: None,
        provenance: Provenance {
            backend: backend.to_string(),
            ..Provenance::default()
        },
    })
}

/// What a stream's text says about it.
#[derive(Debug, Default, PartialEq)]
struct Read {
    resolution: Resolution,
    hdr: bool,
    codec: VideoCodec,
    size_bytes: Option<u64>,
    seeders: Option<u32>,
}

fn read_text(text: &str) -> Read {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    let resolution = if has("2160p") || has("4k") || has("uhd") {
        Resolution::Uhd
    } else if has("1080p") || has("1080i") {
        Resolution::FullHd
    } else if has("720p") {
        Resolution::Hd
    } else if has("576p") || has("480p") || has("360p") || has("sd") {
        Resolution::Sd
    } else {
        Resolution::Unknown
    };
    let hdr =
        has("hdr") || has("hdr10") || has("dv") || has("dovi") || lower.contains("dolby vision");
    let codec = if has("x265") || has("hevc") || lower.contains("h.265") || has("h265") {
        VideoCodec::H265
    } else if has("x264") || has("avc") || lower.contains("h.264") || has("h264") {
        VideoCodec::H264
    } else if has("av1") {
        VideoCodec::Av1
    } else {
        VideoCodec::Unknown
    };
    Read {
        resolution,
        hdr,
        codec,
        size_bytes: size_in(&lower),
        seeders: seeders_in(&lower),
    }
}

/// "1.4 GB", "700 MB", "2.1 GiB": the first size the text gives.
fn size_in(lower: &str) -> Option<u64> {
    let b = lower.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            let number: f64 = lower[start..i].parse().ok()?;
            let rest = lower[i..].trim_start();
            let unit = if rest.starts_with("gb") || rest.starts_with("gib") {
                Some(1024.0 * 1024.0 * 1024.0)
            } else if rest.starts_with("mb") || rest.starts_with("mib") {
                Some(1024.0 * 1024.0)
            } else {
                None
            };
            if let Some(u) = unit {
                return Some((number * u) as u64);
            }
        }
        i += 1;
    }
    None
}

/// "👤 123", or "seeders: 123" / "seeds 123": how many share it, as the add-on says.
fn seeders_in(lower: &str) -> Option<u32> {
    for marker in ["👤", "seeders", "seeds", "seeders:", "seeds:"] {
        if let Some(at) = lower.find(marker) {
            let digits: String = lower[at + marker.len()..]
                .trim_start_matches([' ', ':'])
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(n) = digits.parse() {
                return Some(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExternalIds;

    fn origin() -> Url {
        Url::parse("https://addon.example/abc%3D%3D/manifest.json").unwrap()
    }

    fn manifest(json: &str) -> Result<StremioManifest, ManifestError> {
        StremioManifest::parse(json.as_bytes(), &origin())
    }

    const BASIC: &str = r#"{"id":"org.example.Films","version":"1.2.0","name":"Example",
        "description":"Line one\nline two","resources":["catalog",{"name":"stream","types":["movie","series"],
        "idPrefixes":["tt"]}],"types":["movie","series","channel"],"catalogs":[],
        "behaviorHints":{"configurable":true}}"#;

    #[test]
    fn a_stremio_manifest_is_read_with_its_base_kept_whole() {
        let m = manifest(BASIC).unwrap();
        assert_eq!(m.id, "org.example.films");
        assert_eq!(m.stream_types, vec!["movie", "series"]);
        assert_eq!(m.description, "Line one · line two");
        // A configured add-on keeps its settings in the path: the base keeps them.
        assert_eq!(m.base.as_str(), "https://addon.example/abc%3D%3D/");
        assert_eq!(m.media(), vec![MediaKind::Film, MediaKind::Episode]);
        // Stored and read back, it is the same.
        let again = StremioManifest::parse(m.to_json().as_bytes(), &origin()).unwrap();
        assert_eq!(again, m);
    }

    #[test]
    fn a_manifest_this_app_cannot_use_says_why() {
        let no_stream = r#"{"id":"org.example.meta","version":"1","name":"Meta","resources":["catalog","meta"],"types":["movie"]}"#;
        assert!(manifest(no_stream)
            .unwrap_err()
            .to_string()
            .contains("offers no streams"));
        let set_up = r#"{"id":"org.example.cfg","version":"1","name":"Cfg","resources":["stream"],"types":["movie"],"behaviorHints":{"configurationRequired":true}}"#;
        assert!(manifest(set_up)
            .unwrap_err()
            .to_string()
            .contains("set up first"));
        let kitsu = r#"{"id":"org.example.anime","version":"1","name":"A","resources":["stream"],"types":["series"],"idPrefixes":["kitsu"]}"#;
        assert!(manifest(kitsu).unwrap_err().to_string().contains("IMDb"));
        let tv = r#"{"id":"org.example.tv","version":"1","name":"TV","resources":["stream"],"types":["tv","channel"]}"#;
        assert!(manifest(tv).is_err());
        let not_manifest = Url::parse("https://addon.example/addon.json").unwrap();
        assert!(StremioManifest::parse(BASIC.as_bytes(), &not_manifest).is_err());
    }

    fn query(kind: MediaKind) -> MediaQuery {
        MediaQuery {
            kind,
            title: "Example Show".into(),
            year: Some(2008),
            ids: ExternalIds {
                imdb: Some("tt0903747".into()),
                ..ExternalIds::default()
            },
            season: (kind == MediaKind::Episode).then_some(1),
            episode: (kind == MediaKind::Episode).then_some(2),
        }
    }

    #[test]
    fn a_film_and_an_episode_are_asked_for_by_imdb_id() {
        let m = manifest(BASIC).unwrap();
        assert_eq!(
            m.stream_url(&query(MediaKind::Film)).unwrap().as_str(),
            "https://addon.example/abc%3D%3D/stream/movie/tt0903747.json"
        );
        assert_eq!(
            m.stream_url(&query(MediaKind::Episode)).unwrap().as_str(),
            "https://addon.example/abc%3D%3D/stream/series/tt0903747:1:2.json"
        );
        let mut no_id = query(MediaKind::Film);
        no_id.ids.imdb = None;
        assert!(m.stream_url(&no_id).is_none(), "no IMDb id, nothing to ask");
        let mut odd = query(MediaKind::Film);
        odd.ids.imdb = Some("tt12/../x".into());
        assert!(
            m.stream_url(&odd).is_none(),
            "only tt and digits reach the path"
        );
    }

    #[test]
    fn a_stremio_install_link_stands_for_its_https_address() {
        let url = crate::parse_manifest_url("stremio://addon.example/cfg/manifest.json").unwrap();
        assert_eq!(url.as_str(), "https://addon.example/cfg/manifest.json");
    }

    /// A made-up info hash, built at run time: the posture guard refuses a literal one.
    fn hash() -> String {
        "0123456789abcdef".repeat(3)[..40].to_string()
    }

    #[test]
    fn streams_become_candidates_with_what_their_text_says() {
        let answer = format!(
            r#"{{"streams":[
              {{"name":"Example\n2160p","title":"Film.2008.2160p.UHD.BluRay.x265.HDR-GRP\n👤 412 💾 18.4 GB","infoHash":"{}","fileIdx":3,
                "sources":["tracker:udp://tracker.example:1337/announce","dht:{}"]}},
              {{"name":"Example 720p","url":"https://cdn.example/film.mp4","behaviorHints":{{"videoSize":734003200,"filename":"Film.2008.720p.WEB.x264.mp4"}}}},
              {{"name":"Trailer","ytId":"abc"}},
              {{"name":"Elsewhere","externalUrl":"https://site.example/watch"}},
              {{"name":"Needs headers","url":"https://cdn.example/x.mkv","behaviorHints":{{"proxyHeaders":{{"request":{{"Referer":"https://site.example"}}}}}}}},
              {{"name":"Bad","infoHash":"nothex"}},
              {{"name":"Local","url":"file:///C:/x.mkv"}}
            ]}}"#,
            hash().to_uppercase(),
            hash()
        );
        let (c, dropped) = parse_streams(
            answer.as_bytes(),
            "addon:org.example.films",
            &query(MediaKind::Film),
        )
        .unwrap();
        assert_eq!((c.len(), dropped), (2, 5));
        let t = &c[0];
        assert_eq!(t.transport, Transport::P2p);
        assert_eq!(t.quality.resolution, Resolution::Uhd);
        assert!(t.quality.hdr);
        assert_eq!(t.codec, VideoCodec::H265);
        assert_eq!(
            t.health,
            Health::Swarm {
                seeders: 412,
                peers: 412
            }
        );
        assert_eq!(t.size_bytes, Some((18.4 * 1024.0 * 1024.0 * 1024.0) as u64));
        assert_eq!(t.provenance.backend, "addon:org.example.films");
        assert_eq!(
            stream_of(t).unwrap(),
            PlayableStream::Torrent {
                info_hash: hash(),
                file_index: Some(3),
                trackers: vec!["udp://tracker.example:1337/announce".into()],
            }
        );
        let w = &c[1];
        assert_eq!(
            (w.transport, w.quality.resolution, w.codec),
            (Transport::Http, Resolution::Hd, VideoCodec::H264)
        );
        assert_eq!(
            (w.health, w.size_bytes),
            (Health::Direct, Some(734_003_200))
        );
        assert!(matches!(stream_of(w).unwrap(), PlayableStream::Http { .. }));
    }

    #[test]
    fn a_hostile_answer_is_bounded() {
        let many: Vec<String> = (0..200)
            .map(|i| format!(r#"{{"url":"https://cdn.example/{i}.mp4"}}"#))
            .collect();
        let answer = format!(r#"{{"streams":[{}]}}"#, many.join(","));
        let (c, dropped) =
            parse_streams(answer.as_bytes(), "addon:x", &query(MediaKind::Film)).unwrap();
        assert_eq!((c.len(), dropped), (MAX_STREAMS, 200 - MAX_STREAMS));
        let deep = format!("{}{}", "[".repeat(40), "]".repeat(40));
        assert!(parse_streams(deep.as_bytes(), "addon:x", &query(MediaKind::Film)).is_err());
        let sneaky =
            r#"{"streams":[{"title":"Fine\u202eexe.mp4","url":"https://cdn.example/a.mp4"}]}"#;
        let (c, _) = parse_streams(sneaky.as_bytes(), "addon:x", &query(MediaKind::Film)).unwrap();
        assert!(
            !c[0].title.contains('\u{202e}'),
            "no invisible reordering in shown text"
        );
    }
}
