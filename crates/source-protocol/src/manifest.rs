//! The addon manifest: a source described as DATA (ADR-0006).
//!
//! Declarative is the whole point. Parsing untrusted data is a bounded problem — every
//! limit below is a number — and executing untrusted code is not. A manifest therefore
//! says only what an addon is called, what it offers, and where its two endpoints are.
//! It cannot carry code, templates, selectors or anything else that would turn the app
//! into the thing that fetches and interprets someone's website.
//!
//! **Everything here assumes the input is hostile.** The checks run in order of cost:
//! size before UTF-8, UTF-8 before nesting depth, depth before the JSON parser, and the
//! parser before any field rule. So no input reaches the expensive steps having failed
//! a cheap one, and every rejection says, in plain words, which rule it broke.

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::types::{MediaKind, Transport};
use crate::PROTOCOL_VERSION;

/// Manifests are a few hundred bytes. 64 KiB is generous and still bounded.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// A valid manifest nests three deep (object → endpoints → string). Eight is slack;
/// a hundred thousand `[` is an attack on a recursive parser.
pub const MAX_DEPTH: usize = 8;
pub const MAX_NAME_CHARS: usize = 64;
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_VERSION_CHARS: usize = 32;
pub const MAX_URL_CHARS: usize = 2048;
pub const MAX_LIST_ENTRIES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Which version of this protocol the addon speaks. Only `1` exists.
    pub protocol: u32,
    /// Stable identity, e.g. `com.example.films`. Lowercase, 3–64 characters.
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub media: Vec<MediaKind>,
    pub transports: Vec<Transport>,
    pub endpoints: Endpoints,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoints {
    pub find: String,
    pub resolve: String,
}

/// Why a manifest was refused, worded for the person who pasted the URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("the manifest is {0} bytes; the limit is {MAX_MANIFEST_BYTES}")]
    TooLarge(usize),
    #[error("the manifest is not UTF-8 text")]
    NotText,
    #[error("the manifest nests deeper than {MAX_DEPTH} levels")]
    TooDeep,
    #[error("the manifest is not valid: {0}")]
    Malformed(String),
    #[error(
        "the addon speaks protocol version {0}; this app understands version {PROTOCOL_VERSION}"
    )]
    UnsupportedProtocol(u32),
    #[error("`{field}` {problem}")]
    Field {
        field: &'static str,
        problem: String,
    },
    #[error("the address `{url}` {problem}")]
    Url { url: String, problem: String },
}

fn field(field: &'static str, problem: impl Into<String>) -> ManifestError {
    ManifestError::Field {
        field,
        problem: problem.into(),
    }
}

impl Manifest {
    /// Parse and check a manifest fetched from `origin`. Its endpoints must be on
    /// `origin`'s host: a manifest on one site may not send the app to another.
    pub fn parse(bytes: &[u8], origin: &Url) -> Result<Self, ManifestError> {
        let text = bounded_json(bytes, MAX_MANIFEST_BYTES)?;
        let m: Manifest = serde_json::from_str(text).map_err(malformed)?;
        m.check(origin)?;
        Ok(m)
    }

    fn check(&self, origin: &Url) -> Result<(), ManifestError> {
        if self.protocol != PROTOCOL_VERSION {
            return Err(ManifestError::UnsupportedProtocol(self.protocol));
        }
        check_id(&self.id)?;
        check_text("name", &self.name, 1, MAX_NAME_CHARS)?;
        check_text("version", &self.version, 1, MAX_VERSION_CHARS)?;
        check_text("description", &self.description, 0, MAX_DESCRIPTION_CHARS)?;
        check_list("media", &self.media, MediaKind::Unknown)?;
        check_list("transports", &self.transports, Transport::Unknown)?;
        for (name, raw) in [
            ("endpoints.find", &self.endpoints.find),
            ("endpoints.resolve", &self.endpoints.resolve),
        ] {
            let url = check_url(raw)?;
            if url.host() != origin.host()
                || url.port_or_known_default() != origin.port_or_known_default()
            {
                return Err(field(
                    name,
                    format!(
                        "points at {}, but the manifest came from {}; an addon's endpoints must be on its own host",
                        host_label(&url),
                        host_label(origin)
                    ),
                ));
            }
        }
        if let Some(home) = &self.homepage {
            check_url(home)?;
        }
        Ok(())
    }
}

/// The parser's own message, which says where the problem is — capped, because it can
/// quote the input ("unknown field `…`") and the input is the attacker's.
pub(crate) fn malformed(e: serde_json::Error) -> ManifestError {
    ManifestError::Malformed(e.to_string().chars().take(200).collect())
}

/// Check the address a user pasted before anything is fetched from it.
pub fn parse_manifest_url(raw: &str) -> Result<Url, ManifestError> {
    check_url(raw.trim())
}

/// Size, then UTF-8, then nesting depth: everything that can be refused before a
/// parser sees the input.
pub(crate) fn bounded_json(bytes: &[u8], max: usize) -> Result<&str, ManifestError> {
    if bytes.len() > max {
        return Err(ManifestError::TooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::NotText)?;
    if nesting_depth(text) > MAX_DEPTH {
        return Err(ManifestError::TooDeep);
    }
    Ok(text)
}

/// The deepest `{`/`[` nesting outside string literals. One pass, no allocation, and
/// no recursion — which is the property that matters, because it runs BEFORE a
/// recursive parser and exists to protect it.
pub(crate) fn nesting_depth(text: &str) -> usize {
    let (mut depth, mut max) = (0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for b in text.bytes() {
        if in_string {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                max = max.max(depth);
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

fn check_id(id: &str) -> Result<(), ManifestError> {
    let ok_len = (3..=64).contains(&id.len());
    let ok_start = id.starts_with(|c: char| c.is_ascii_lowercase());
    let ok_chars = id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'));
    if ok_len && ok_start && ok_chars {
        Ok(())
    } else {
        Err(field(
            "id",
            "must be 3 to 64 characters of lowercase letters, digits, `.`, `-` or `_`, starting with a letter",
        ))
    }
}

/// Text that will be SHOWN to the user. Beyond length, it may not carry control
/// characters or the bidirectional overrides that make "Films" render as something
/// else — the trick that lets a name impersonate another addon.
pub(crate) fn check_text(
    name: &'static str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), ManifestError> {
    let chars = value.chars().count();
    if chars < min || value.trim().chars().count() < min {
        return Err(field(name, "is empty"));
    }
    if chars > max {
        return Err(field(
            name,
            format!("is {chars} characters; the limit is {max}"),
        ));
    }
    if let Some(c) = value.chars().find(|&c| is_deceptive(c)) {
        return Err(field(
            name,
            format!(
                "contains the invisible or control character U+{:04X}",
                c as u32
            ),
        ));
    }
    Ok(())
}

fn is_deceptive(c: char) -> bool {
    (c.is_control() && c != '\n')
        || matches!(c as u32,
            0x200B..=0x200F      // zero-width space/joiners, LRM, RLM
            | 0x202A..=0x202E    // bidi embeddings and overrides
            | 0x2066..=0x2069    // bidi isolates
            | 0xFEFF) // zero-width no-break space
}

fn check_list<T: PartialEq>(
    name: &'static str,
    list: &[T],
    unknown: T,
) -> Result<(), ManifestError> {
    if list.len() > MAX_LIST_ENTRIES {
        return Err(field(
            name,
            format!(
                "has {} entries; the limit is {MAX_LIST_ENTRIES}",
                list.len()
            ),
        ));
    }
    if !list.iter().any(|v| *v != unknown) {
        return Err(field(name, "names nothing this app understands"));
    }
    Ok(())
}

/// HTTPS, or plain HTTP to this machine only; no credentials in the address.
pub(crate) fn check_url(raw: &str) -> Result<Url, ManifestError> {
    let bad = |problem: &str| ManifestError::Url {
        url: raw.chars().take(80).collect(),
        problem: problem.into(),
    };
    if raw.len() > MAX_URL_CHARS {
        return Err(bad("is too long"));
    }
    let url = Url::parse(raw).map_err(|_| bad("is not a valid web address"))?;
    if url.host().is_none() {
        return Err(bad("has no host"));
    }
    match url.scheme() {
        "https" => {}
        "http" if is_loopback(&url) => {}
        "http" => {
            return Err(bad(
                "is not secure: only https is allowed, except on this computer",
            ))
        }
        _ => return Err(bad("must be an https address")),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(bad("contains a user name or password"));
    }
    Ok(url)
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

fn host_label(url: &Url) -> String {
    match (url.host_str(), url.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        _ => "nowhere".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn origin() -> Url {
        Url::parse("https://addon.example/manifest.json").unwrap()
    }

    pub(crate) const GOOD: &str = r#"{
        "protocol": 1,
        "id": "com.example.films",
        "name": "Example Films",
        "version": "1.0.0",
        "description": "Public-domain features.",
        "media": ["film"],
        "transports": ["http"],
        "endpoints": {
            "find": "https://addon.example/find",
            "resolve": "https://addon.example/resolve"
        }
    }"#;

    #[test]
    fn a_well_formed_manifest_parses() {
        let m = Manifest::parse(GOOD.as_bytes(), &origin()).unwrap();
        assert_eq!(m.name, "Example Films");
        assert_eq!(m.media, vec![MediaKind::Film]);
    }

    #[test]
    fn depth_ignores_brackets_inside_strings() {
        assert_eq!(nesting_depth(r#"{"a":"[[[[{{{{\"[[["}"#), 1);
        assert_eq!(nesting_depth("[[[]]]"), 3);
        assert_eq!(nesting_depth("]]]][["), 2);
    }

    #[test]
    fn plain_http_is_allowed_only_on_this_machine() {
        assert!(check_url("http://localhost:7000/m.json").is_ok());
        assert!(check_url("http://127.0.0.1/m.json").is_ok());
        assert!(check_url("http://[::1]/m.json").is_ok());
        assert!(check_url("http://example.com/m.json").is_err());
        assert!(check_url("file:///C:/Windows/win.ini").is_err());
    }
}
