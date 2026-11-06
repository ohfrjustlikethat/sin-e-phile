//! `DirectHttpBackend`: links the user added themselves (SPEC.md §2.1).
//!
//! A link is a title, an optional year and a URL, typed in or imported from an M3U
//! playlist the user chose. The app ships none. Matching is by normalised title, and
//! by year when both sides have one — the same normalisation the catalogue's exact
//! title tier uses, so "Nosferatu" and "NOSFERATU." are one film here as there.

use source_protocol::{
    async_trait, BackendCapabilities, Health, MediaKind, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError, Transport,
};

use sinephile_catalogue::matching::normalise;

pub const ID: &str = "direct";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLink {
    pub title: String,
    pub year: Option<u16>,
    pub url: String,
}

pub struct DirectHttpBackend {
    links: Vec<(String, DirectLink)>,
}

impl DirectHttpBackend {
    pub fn new(links: Vec<DirectLink>) -> Self {
        Self {
            links: links
                .into_iter()
                .map(|l| (normalise(&l.title), l))
                .collect(),
        }
    }
}

#[async_trait]
impl SourceBackend for DirectHttpBackend {
    fn id(&self) -> &str {
        ID
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film, MediaKind::Episode],
            transports: vec![Transport::Http],
            network: true,
        }
    }

    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        let wanted = normalise(&query.title);
        Ok(self
            .links
            .iter()
            .filter(|(title, link)| {
                *title == wanted
                    && match (query.year, link.year) {
                        (Some(a), Some(b)) => a == b,
                        _ => true,
                    }
            })
            .map(|(_, link)| SourceCandidate {
                key: link.url.clone(),
                title: link.title.clone(),
                year: link.year,
                transport: Transport::Http,
                quality: Default::default(),
                languages: vec![],
                subtitles: vec![],
                codec: Default::default(),
                size_bytes: None,
                duration_seconds: None,
                health: Health::Direct,
                locator: Some(link.url.clone()),
                provenance: Default::default(),
            })
            .collect())
    }

    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        // Only a URL this backend holds: a candidate from elsewhere, or one whose link
        // was removed since, is refused rather than fetched on its say-so.
        self.links
            .iter()
            .find(|(_, l)| l.url == candidate.key)
            .map(|(_, l)| PlayableStream::Http {
                url: l.url.clone(),
                content_type: None,
                size_bytes: None,
            })
            .ok_or_else(|| SourceError::NotFound("that link is no longer in your list".into()))
    }
}

/// Most entries a single import may add. A playlist larger than this is not a list of
/// films someone curated.
pub const MAX_M3U_ENTRIES: usize = 10_000;

/// Read an M3U or extended-M3U playlist into links.
///
/// `#EXTINF:<duration> <attributes>,<display name>` names the URL on the next line; a
/// bare URL with no `#EXTINF` takes its file name as its title. A trailing `(1922)` in
/// a name becomes the year. Only `http` and `https` URLs are kept — a playlist can
/// also name local paths and other schemes, and this backend serves neither.
pub fn parse_m3u(text: &str) -> Vec<DirectLink> {
    let mut links = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            // The display name is after the LAST comma that is outside quotes;
            // attributes such as tvg-name="a, b" may contain commas.
            pending = display_name(info);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let name = pending.take();
        if !(line.starts_with("https://") || line.starts_with("http://")) {
            continue;
        }
        let name = name.unwrap_or_else(|| file_stem(line));
        let (title, year) = split_year(&name);
        if title.is_empty() {
            continue;
        }
        links.push(DirectLink {
            title,
            year,
            url: line.to_string(),
        });
        if links.len() == MAX_M3U_ENTRIES {
            break;
        }
    }
    links
}

fn display_name(info: &str) -> Option<String> {
    let mut in_quotes = false;
    let mut split = None;
    for (i, c) in info.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => split = Some(i),
            _ => {}
        }
    }
    let name = info[split? + 1..].trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn file_stem(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or(path);
    let stem = last.rsplit_once('.').map(|(s, _)| s).unwrap_or(last);
    stem.replace(['_', '.', '-'], " ").trim().to_string()
}

/// "Nosferatu (1922)" → ("Nosferatu", Some(1922)). Only a plausible film year counts.
fn split_year(name: &str) -> (String, Option<u16>) {
    let trimmed = name.trim();
    if let Some(open) = trimmed.rfind('(') {
        let inner = trimmed[open + 1..].trim_end_matches(')').trim();
        if trimmed.ends_with(')') {
            if let Ok(year) = inner.parse::<u16>() {
                if (1878..=2100).contains(&year) {
                    return (trimmed[..open].trim().to_string(), Some(year));
                }
            }
        }
    }
    (trimmed.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYLIST: &str = "#EXTM3U
#EXTINF:-1 tvg-name=\"Silent, restored\" group-title=\"Films\",Nosferatu (1922)
https://films.example/nosferatu.mp4

#EXTINF:5400,The General
https://films.example/general.mp4?token=x
https://films.example/the_cabinet_of_dr_caligari.mkv
#EXTINF:-1,Local only
C:\\Films\\local.mkv
#EXTINF:-1,Sneaky
file:///C:/Windows/win.ini
";

    #[test]
    fn an_extended_playlist_becomes_links() {
        let links = parse_m3u(PLAYLIST);
        let got: Vec<(&str, Option<u16>)> =
            links.iter().map(|l| (l.title.as_str(), l.year)).collect();
        assert_eq!(
            got,
            [
                ("Nosferatu", Some(1922)),
                ("The General", None),
                ("the cabinet of dr caligari", None),
            ]
        );
        assert!(links.iter().all(|l| l.url.starts_with("https://")));
    }

    #[test]
    fn only_a_plausible_trailing_year_is_a_year() {
        assert_eq!(
            split_year("2001: A Space Odyssey"),
            ("2001: A Space Odyssey".into(), None)
        );
        assert_eq!(
            split_year("Film (Director's Cut)"),
            ("Film (Director's Cut)".into(), None)
        );
        assert_eq!(split_year("Old (1200)"), ("Old (1200)".into(), None));
        assert_eq!(split_year("M (1931)"), ("M".into(), Some(1931)));
    }

    #[tokio::test]
    async fn finds_by_normalised_title_and_year_and_resolves_only_its_own_links() {
        let backend = DirectHttpBackend::new(parse_m3u(PLAYLIST));
        let found = backend
            .find(&MediaQuery::film("NOSFERATU", Some(1922)))
            .await
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].health, Health::Direct);
        assert!(
            backend
                .find(&MediaQuery::film("Nosferatu", Some(1979)))
                .await
                .unwrap()
                .is_empty(),
            "a different year is a different film"
        );
        assert_eq!(
            backend
                .find(&MediaQuery::film("The General", Some(1926)))
                .await
                .unwrap()
                .len(),
            1,
            "a link with no year matches any year"
        );

        assert!(matches!(
            backend.resolve(&found[0]).await,
            Ok(PlayableStream::Http { .. })
        ));
        let mut forged = found[0].clone();
        forged.key = "https://elsewhere.example/x".into();
        assert!(matches!(
            backend.resolve(&forged).await,
            Err(SourceError::NotFound(_))
        ));
    }
}
