//! What an addon's two endpoints receive and return — the protocol's wire format.
//!
//! - `POST <endpoints.find>` with a [`MediaQuery`] as the JSON body answers
//!   `{"candidates": [SourceCandidate, …]}`.
//! - `POST <endpoints.resolve>` with `{"key": "<a key it returned>"}` answers
//!   `{"stream": PlayableStream}`.
//!
//! An addon is a remote service the user chose to trust with a QUERY, not with the
//! app. So its answers are parsed as hostile, exactly as its manifest is: bounded in
//! size, every string bounded in length, display text free of control and bidi
//! characters, and links checked. A single bad candidate is dropped, not allowed to
//! cost the good ones; a stream that would make the app open a LOCAL FILE is refused
//! outright, because a remote service has no business naming a path on this machine.

use serde::{Deserialize, Serialize};

use crate::manifest::{bounded_json, check_text, check_url, malformed, ManifestError};
use crate::types::{PlayableStream, SourceCandidate};

pub const MAX_FIND_RESPONSE_BYTES: usize = 1024 * 1024;
pub const MAX_RESOLVE_RESPONSE_BYTES: usize = 16 * 1024;
pub const MAX_CANDIDATES: usize = 200;
pub const MAX_KEY_CHARS: usize = 2048;
pub const MAX_TITLE_CHARS: usize = 300;
pub const MAX_LANGUAGES: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindResponse {
    pub candidates: Vec<SourceCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub stream: PlayableStream,
}

/// Parse a find response. Returns the candidates that passed, and how many did not —
/// the count is for the health line ("3 results ignored as malformed"), not an error.
pub fn parse_find(bytes: &[u8]) -> Result<(Vec<SourceCandidate>, usize), ManifestError> {
    let text = bounded_json(bytes, MAX_FIND_RESPONSE_BYTES)?;
    let r: FindResponse = serde_json::from_str(text).map_err(malformed)?;
    let total = r.candidates.len();
    let kept: Vec<SourceCandidate> = r
        .candidates
        .into_iter()
        .take(MAX_CANDIDATES)
        .filter_map(clean)
        .collect();
    let dropped = total - kept.len();
    Ok((kept, dropped))
}

pub fn parse_resolve(bytes: &[u8]) -> Result<PlayableStream, ManifestError> {
    let text = bounded_json(bytes, MAX_RESOLVE_RESPONSE_BYTES)?;
    let r: ResolveResponse = serde_json::from_str(text).map_err(malformed)?;
    check_stream(&r.stream)?;
    Ok(r.stream)
}

/// A remote stream may be HTTP(S) or a torrent; never a file on this machine.
pub fn check_stream(stream: &PlayableStream) -> Result<(), ManifestError> {
    let refuse = |problem: &str| ManifestError::Field {
        field: "stream",
        problem: problem.into(),
    };
    match stream {
        PlayableStream::Http { url, .. } => {
            let parsed = url::Url::parse(url).map_err(|_| refuse("is not a valid web address"))?;
            if !matches!(parsed.scheme(), "https" | "http") || parsed.host().is_none() {
                return Err(refuse("must be an http or https address"));
            }
            if !parsed.username().is_empty() || parsed.password().is_some() {
                return Err(refuse("contains a user name or password"));
            }
            Ok(())
        }
        PlayableStream::Torrent { info_hash, .. } => {
            if info_hash.len() == 40 && info_hash.chars().all(|c| c.is_ascii_hexdigit()) {
                Ok(())
            } else {
                Err(refuse("names a torrent with a malformed info hash"))
            }
        }
        PlayableStream::File { .. } => Err(refuse(
            "names a file on this computer, which an addon may not do",
        )),
    }
}

/// Keep a candidate only if every field is within bounds; repair what can be repaired
/// harmlessly (an unsafe page link is dropped, not the whole candidate).
fn clean(mut c: SourceCandidate) -> Option<SourceCandidate> {
    if c.key.is_empty() || c.key.chars().count() > MAX_KEY_CHARS {
        return None;
    }
    check_text("title", &c.title, 1, MAX_TITLE_CHARS).ok()?;
    if c.languages.len() > MAX_LANGUAGES || c.subtitles.len() > MAX_LANGUAGES {
        return None;
    }
    if !c
        .languages
        .iter()
        .chain(&c.subtitles)
        .all(|l| is_language_tag(l))
    {
        return None;
    }
    if let Some(locator) = &c.locator {
        if locator.chars().count() > MAX_KEY_CHARS {
            return None;
        }
    }
    // The resolver stamps these; nothing an addon says about them survives.
    c.provenance.backend.clear();
    c.provenance.also_from.clear();
    if let Some(page) = &c.provenance.page {
        if check_url(page).is_err() {
            c.provenance.page = None;
        }
    }
    if let Some(licence) = &c.provenance.licence {
        if check_text("licence", licence, 1, 100).is_err() {
            c.provenance.licence = None;
        }
    }
    Some(c)
}

/// BCP 47 in shape — letters, digits and hyphens, 2–35 characters. Not validated
/// against the registry: shape is what keeps it safe to display and compare.
fn is_language_tag(tag: &str) -> bool {
    (2..=35).contains(&tag.len())
        && tag.starts_with(|c: char| c.is_ascii_alphabetic())
        && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(json: &str) -> (Vec<SourceCandidate>, usize) {
        parse_find(json.as_bytes()).unwrap()
    }

    #[test]
    fn one_bad_candidate_is_dropped_and_the_good_ones_kept() {
        let (kept, dropped) = find(
            r#"{"candidates":[
                {"key":"a","title":"Good","transport":"http","languages":["en","pt-BR"]},
                {"key":"b","title":"Bad \u202e name","transport":"http"},
                {"key":"","title":"No key","transport":"http"},
                {"key":"c","title":"Bad lang","transport":"http","languages":["<script>"]}
            ]}"#,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].key, "a");
        assert_eq!(dropped, 3);
    }

    #[test]
    fn an_addon_cannot_claim_provenance_or_link_somewhere_unsafe() {
        let (kept, _) = find(
            r#"{"candidates":[{"key":"a","title":"T","transport":"http",
                "provenance":{"backend":"internet-archive","also_from":["local"],
                              "page":"javascript:alert(1)","licence":"public domain"}}]}"#,
        );
        let p = &kept[0].provenance;
        assert!(p.backend.is_empty() && p.also_from.is_empty());
        assert_eq!(p.page, None);
        assert_eq!(p.licence.as_deref(), Some("public domain"));
    }

    #[test]
    fn a_remote_stream_may_never_name_a_local_file() {
        let err = parse_resolve(br#"{"stream":{"kind":"file","path":"C:\\Windows\\win.ini"}}"#)
            .unwrap_err();
        assert!(err.to_string().contains("file on this computer"), "{err}");
        assert!(parse_resolve(br#"{"stream":{"kind":"http","url":"file:///C:/x"}}"#).is_err());
        assert!(parse_resolve(br#"{"stream":{"kind":"torrent","info_hash":"zz"}}"#).is_err());
        assert!(parse_resolve(
            br#"{"stream":{"kind":"http","url":"https://cdn.example/film.mp4"}}"#
        )
        .is_ok());
    }

    #[test]
    fn more_than_the_cap_is_truncated_not_refused() {
        let one = r#"{"key":"k","title":"T","transport":"http"}"#;
        let body = format!(r#"{{"candidates":[{}]}}"#, vec![one; 300].join(","));
        let (kept, dropped) = find(&body);
        assert_eq!((kept.len(), dropped), (MAX_CANDIDATES, 100));
    }
}
