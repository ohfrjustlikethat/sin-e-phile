//! `InternetArchiveBackend`: public-domain feature films from the Internet Archive.
//!
//! SPEC.md §2.1 names this as one of the three unambiguously legal reference sources,
//! and `tools/guard/allowlist.txt` carries `archive.org` for it. Two public APIs, both
//! keyless:
//!
//! 1. **Advanced search**, restricted to `mediatype:movies` in the `feature_films`
//!    collection, ranked by downloads. That collection is the Archive's own curated
//!    set of full-length films; the search never leaves it.
//! 2. **Item metadata** for the best few matches, which lists the item's files.
//!
//! A candidate is one VIDEO FILE that is the WHOLE FILM. Recorded responses showed why
//! that needs saying: some items split a film into `1of5` … `5of5`, and no one of those
//! is something to play. So a format counts only when an item has exactly one file in
//! it, and a multi-part item yields nothing rather than a fifth of a film.
//!
//! Quality comes from the Archive's own `height` and `format` labels, not from probing
//! the file: it is what the source says, which is what every candidate's quality is.

use std::sync::Arc;

use serde_json::Value;
use sinephile_catalogue::matching::normalise;
use sinephile_metadata_api::{Request, Transport};
use source_protocol::{
    async_trait, BackendCapabilities, Health, MediaKind, MediaQuery, PlayableStream, Provenance,
    Quality, Resolution, SourceBackend, SourceCandidate, SourceError, Transport as Wire,
    VideoCodec,
};
use tokio::task::JoinSet;
use url::Url;

pub const ID: &str = "internet-archive";

const SEARCH: &str = "https://archive.org/advancedsearch.php";
const METADATA: &str = "https://archive.org/metadata/";
const DOWNLOAD: &str = "https://archive.org/download/";
const DETAILS: &str = "https://archive.org/details/";

/// Items whose metadata is fetched per query: one search plus this many lookups.
const ITEMS_PER_QUERY: usize = 4;
const FILES_PER_ITEM: usize = 3;
const MAX_SEARCH_BYTES: usize = 1024 * 1024;
/// An item with thousands of files has a large listing; this is generous for a film.
const MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;

/// The Archive's `format` labels that are video, and what each says about the codec.
/// A label is a description, not a probe: "512Kb MPEG4" has been both MPEG-4 Part 2
/// and H.264 over the Archive's history, so it maps to the older and safer guess.
const VIDEO_FORMATS: &[(&str, VideoCodec)] = &[
    ("h.264", VideoCodec::H264),
    ("h.264 IA", VideoCodec::H264),
    ("h.264 HD", VideoCodec::H264),
    ("MPEG4", VideoCodec::Mpeg4),
    ("512Kb MPEG4", VideoCodec::Mpeg4),
    ("HiRes MPEG4", VideoCodec::Mpeg4),
    ("DivX", VideoCodec::Mpeg4),
    ("Xvid", VideoCodec::Mpeg4),
    ("MPEG2", VideoCodec::Unknown),
    ("MPEG1", VideoCodec::Unknown),
    ("Ogg Video", VideoCodec::Unknown),
    ("Matroska", VideoCodec::Unknown),
    ("QuickTime", VideoCodec::Unknown),
];

pub struct InternetArchiveBackend {
    http: Arc<dyn Transport>,
}

impl InternetArchiveBackend {
    pub fn new(http: Arc<dyn Transport>) -> Self {
        Self { http }
    }
}

#[async_trait]
impl SourceBackend for InternetArchiveBackend {
    fn id(&self) -> &str {
        ID
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Wire::Http],
            network: true,
        }
    }

    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        let words = normalise(&query.title);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let body = get(&*self.http, search_url(&words), MAX_SEARCH_BYTES).await?;
        let docs = parse_search(&body)?;
        let wanted: Vec<Doc> = docs
            .into_iter()
            .filter(|d| {
                title_matches(&words, &d.title) && year_agrees(query.year, d.year, &d.title)
            })
            .take(ITEMS_PER_QUERY)
            .collect();

        // The lookups run together; a failed one costs that item, not the answer.
        let mut lookups = JoinSet::new();
        for (rank, doc) in wanted.into_iter().enumerate() {
            let http = self.http.clone();
            lookups.spawn(async move {
                let body = get(&*http, metadata_url(&doc.identifier), MAX_METADATA_BYTES).await;
                (rank, doc, body)
            });
        }
        let mut items = Vec::new();
        while let Some(joined) = lookups.join_next().await {
            if let Ok((rank, doc, Ok(body))) = joined {
                if let Ok(files) = parse_files(&body) {
                    items.push((rank, doc, files));
                }
            }
        }
        // Back into the search's order — most downloaded first — not arrival order.
        items.sort_by_key(|(rank, _, _)| *rank);
        Ok(items
            .into_iter()
            .flat_map(|(_, doc, files)| candidates(&doc, &files))
            .collect())
    }

    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        // Rebuilt from the key alone, which is checked: nothing else in a candidate is
        // trusted to say where the bytes are.
        let (identifier, name) = split_key(&candidate.key)
            .ok_or_else(|| SourceError::NotFound("not an Internet Archive file".into()))?;
        Ok(PlayableStream::Http {
            url: download_url(identifier, name),
            content_type: content_type(name).map(String::from),
            size_bytes: candidate.size_bytes,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Doc {
    identifier: String,
    title: String,
    year: Option<u16>,
    licence: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct File {
    name: String,
    format: String,
    size: Option<u64>,
    height: Option<u32>,
}

async fn get(http: &dyn Transport, url: String, max: usize) -> Result<String, SourceError> {
    let response = http
        .send(
            Request::get(url)
                .header("accept", "application/json")
                .limit(max),
        )
        .await
        .map_err(|e| SourceError::Unavailable(format!("the Internet Archive: {e}")))?;
    if response.status != 200 {
        return Err(SourceError::Unavailable(format!(
            "the Internet Archive answered {}",
            response.status
        )));
    }
    Ok(response.body)
}

fn search_url(words: &str) -> String {
    // `normalise` leaves only letters, digits and single spaces, so no Lucene syntax
    // from the user's title can reach the query.
    let terms = words.split(' ').collect::<Vec<_>>().join(" AND ");
    let q = format!("title:({terms}) AND mediatype:(movies) AND collection:(feature_films)");
    Url::parse_with_params(
        SEARCH,
        &[
            ("q", q.as_str()),
            ("fl[]", "identifier"),
            ("fl[]", "title"),
            ("fl[]", "year"),
            ("fl[]", "licenseurl"),
            ("sort[]", "downloads desc"),
            ("rows", "8"),
            ("page", "1"),
            ("output", "json"),
        ],
    )
    .expect("a constant base URL parses")
    .to_string()
}

fn metadata_url(identifier: &str) -> String {
    let mut url = Url::parse(METADATA).expect("a constant base URL parses");
    url.path_segments_mut()
        .expect("https has a path")
        .pop_if_empty()
        .push(identifier);
    url.to_string()
}

fn download_url(identifier: &str, name: &str) -> String {
    let mut url = Url::parse(DOWNLOAD).expect("a constant base URL parses");
    {
        let mut segments = url.path_segments_mut().expect("https has a path");
        segments.pop_if_empty().push(identifier);
        // A file name may contain directories; each part is encoded on its own.
        segments.extend(name.split('/'));
    }
    url.to_string()
}

/// `identifier/name`, both checked. Identifiers are the Archive's own: ASCII letters,
/// digits, `.`, `_` and `-`. A name may have `/` for subdirectories but never a `..`
/// segment, a backslash or an empty part.
fn split_key(key: &str) -> Option<(&str, &str)> {
    let (identifier, name) = key.split_once('/')?;
    // First character alphanumeric, as the Archive's identifiers are — which is also
    // what stops `..` being an identifier.
    let id_ok = identifier.starts_with(|c: char| c.is_ascii_alphanumeric())
        && identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let name_ok = !name.contains('\\')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && part != ".");
    (id_ok && name_ok).then_some((identifier, name))
}

/// The Archive returns a field as a string, a number, or a list of either, depending
/// on the item. Every reader below goes through this.
fn first_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(a) => first_text(a.first()),
        _ => None,
    }
}

fn year_of(v: Option<&Value>) -> Option<u16> {
    let text = first_text(v)?;
    let year: u16 = text.get(..4)?.parse().ok()?;
    (1878..=2100).contains(&year).then_some(year)
}

fn parse_search(body: &str) -> Result<Vec<Doc>, SourceError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| SourceError::InvalidResponse(format!("search: {e}")))?;
    let docs = v
        .pointer("/response/docs")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::InvalidResponse("search: no response.docs".into()))?;
    Ok(docs
        .iter()
        .filter_map(|d| {
            Some(Doc {
                identifier: first_text(d.get("identifier"))?,
                title: first_text(d.get("title"))?,
                year: year_of(d.get("year")),
                licence: first_text(d.get("licenseurl")),
            })
        })
        .filter(|d| split_key(&format!("{}/x", d.identifier)).is_some())
        .collect())
}

fn parse_files(body: &str) -> Result<Vec<File>, SourceError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| SourceError::InvalidResponse(format!("metadata: {e}")))?;
    let files = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::InvalidResponse("metadata: no files".into()))?;
    Ok(files
        .iter()
        .filter_map(|f| {
            Some(File {
                name: first_text(f.get("name"))?,
                format: first_text(f.get("format"))?,
                size: first_text(f.get("size")).and_then(|s| s.parse().ok()),
                height: first_text(f.get("height")).and_then(|s| s.parse().ok()),
            })
        })
        .collect())
}

/// Every word of the query appears in the item's title. Archive titles are noisy —
/// "Nosferatu, eine Symphonie des Grauens (A Symphony of Horror)", "Nosferatu_DVD_quality"
/// — so equality would miss the most-downloaded copy of the film.
fn title_matches(words: &str, title: &str) -> bool {
    let have = normalise(title);
    let have: Vec<&str> = have.split(' ').collect();
    words.split(' ').all(|w| have.contains(&w))
}

/// Whether an item is the film of the year asked for.
///
/// Both years known: they must agree. An item with no year matches only if the year is
/// unknown or the item's title names it ("Nosferatu (1922) Colorized"). Letting a
/// year-less item through matched the 1922 Nosferatu to the 2024 one, so Play on the
/// 2024 film would have shown the 1922 film: a confidently wrong film is worse than a
/// missed copy, and well-catalogued films have copies that do carry a year.
fn year_agrees(wanted: Option<u16>, have: Option<u16>, title: &str) -> bool {
    match (wanted, have) {
        (Some(a), Some(b)) => a == b,
        (Some(a), None) => title.contains(&a.to_string()),
        (None, _) => true,
    }
}

fn candidates(doc: &Doc, files: &[File]) -> Vec<SourceCandidate> {
    let mut whole: Vec<(&File, VideoCodec)> = VIDEO_FORMATS
        .iter()
        .filter_map(|(format, codec)| {
            let mut of_format = files.iter().filter(|f| f.format == *format);
            match (of_format.next(), of_format.next()) {
                // Exactly one file in this format: the whole film.
                (Some(f), None) => Some((f, *codec)),
                // None, or several parts.
                _ => None,
            }
        })
        .collect();
    whole.sort_by_key(|(f, _)| std::cmp::Reverse(f.height.unwrap_or(0)));
    whole
        .into_iter()
        .take(FILES_PER_ITEM)
        .map(|(f, codec)| SourceCandidate {
            key: format!("{}/{}", doc.identifier, f.name),
            title: doc.title.clone(),
            year: doc.year,
            transport: Wire::Http,
            quality: Quality {
                resolution: f.height.map(Resolution::from_height).unwrap_or_default(),
                hdr: false,
            },
            languages: vec![],
            subtitles: vec![],
            codec,
            size_bytes: f.size,
            health: Health::Direct,
            locator: Some(download_url(&doc.identifier, &f.name)),
            provenance: Provenance {
                page: Some(format!("{DETAILS}{}", doc.identifier)),
                licence: doc.licence.clone(),
                ..Default::default()
            },
        })
        .collect()
}

fn content_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "ogv" => "video/ogg",
        "avi" => "video/x-msvideo",
        "mpg" | "mpeg" => "video/mpeg",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_users_title_cannot_inject_search_syntax() {
        let url = search_url(&normalise("Nosferatu) OR collection:(anything"));
        let decoded = Url::parse(&url).unwrap();
        let q = decoded.query_pairs().find(|(k, _)| k == "q").unwrap().1;
        assert_eq!(
            q,
            "title:(nosferatu AND or AND collection AND anything) AND mediatype:(movies) AND collection:(feature_films)"
        );
    }

    #[test]
    fn keys_are_checked_before_they_become_urls() {
        assert_eq!(
            split_key("Nosferatu_DVD_quality/a b.mp4"),
            Some(("Nosferatu_DVD_quality", "a b.mp4"))
        );
        assert_eq!(split_key("id/dir/file.mp4"), Some(("id", "dir/file.mp4")));
        for bad in [
            "id/../../etc",
            "id/a\\b",
            "id//x",
            "../x",
            "id",
            "i d/x",
            "id/",
        ] {
            assert_eq!(split_key(bad), None, "{bad}");
        }
        assert_eq!(
            download_url("id", "dir/a b#c.mp4"),
            "https://archive.org/download/id/dir/a%20b%23c.mp4"
        );
    }

    #[test]
    fn fields_may_be_strings_numbers_or_lists() {
        assert_eq!(year_of(Some(&serde_json::json!(1922))), Some(1922));
        assert_eq!(year_of(Some(&serde_json::json!("1922-03-04"))), Some(1922));
        assert_eq!(year_of(Some(&serde_json::json!(["1922"]))), Some(1922));
        assert_eq!(year_of(Some(&serde_json::json!("unknown"))), None);
    }

    #[test]
    fn a_year_less_copy_is_not_another_years_film() {
        // The 2024 Nosferatu must not be offered the Archive's year-less 1922 copies.
        assert!(!year_agrees(Some(2024), None, "Nosferatu"));
        assert!(!year_agrees(Some(2024), Some(1922), "Nosferatu (1922)"));
        assert!(year_agrees(Some(1922), None, "Nosferatu (1922) Colorized"));
        assert!(year_agrees(Some(1922), Some(1922), "Nosferatu"));
        assert!(year_agrees(None, None, "Nosferatu"));
    }
}
