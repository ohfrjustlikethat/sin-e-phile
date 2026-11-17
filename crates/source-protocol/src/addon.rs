//! An installed add-on's manifest: the app's own protocol, or Stremio's (ADR-0047).
//! Which one is decided by the manifest's shape: Stremio's lists `resources` and has no
//! `protocol`.

use serde_json::Value;
use url::Url;

use crate::manifest::{bounded_json, malformed, Manifest, ManifestError, MAX_MANIFEST_BYTES};
use crate::stremio::StremioManifest;
use crate::types::{MediaKind, Transport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddonManifest {
    Native(Manifest),
    Stremio(StremioManifest),
}

impl AddonManifest {
    /// Parse and check a manifest fetched from `origin`, whichever protocol it speaks.
    pub fn parse(bytes: &[u8], origin: &Url) -> Result<Self, ManifestError> {
        let text = bounded_json(bytes, MAX_MANIFEST_BYTES)?;
        let v: Value = serde_json::from_str(text).map_err(malformed)?;
        let stremio = v.get("resources").is_some() && v.get("protocol").is_none();
        if stremio {
            StremioManifest::parse(bytes, origin).map(Self::Stremio)
        } else {
            Manifest::parse(bytes, origin).map(Self::Native)
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Native(m) => &m.id,
            Self::Stremio(m) => &m.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Native(m) => &m.name,
            Self::Stremio(m) => &m.name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Native(m) => &m.version,
            Self::Stremio(m) => &m.version,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Native(m) => &m.description,
            Self::Stremio(m) => &m.description,
        }
    }

    /// Its own settings page, if it has one: a Stremio add-on that says it is configurable.
    pub fn configure_url(&self) -> Option<Url> {
        match self {
            Self::Native(_) => None,
            Self::Stremio(m) => m.configure_url(),
        }
    }

    /// Which protocol it speaks, in words.
    pub fn protocol(&self) -> &'static str {
        match self {
            Self::Native(_) => "sin·e·phile add-on",
            Self::Stremio(_) => "Stremio add-on",
        }
    }

    pub fn homepage(&self) -> Option<&str> {
        match self {
            Self::Native(m) => m.homepage.as_deref(),
            Self::Stremio(_) => None,
        }
    }

    pub fn media(&self) -> Vec<MediaKind> {
        match self {
            Self::Native(m) => m.media.clone(),
            Self::Stremio(m) => m.media(),
        }
    }

    /// How its streams travel. A Stremio add-on does not say in advance: either, title
    /// by title.
    pub fn transports(&self) -> Vec<Transport> {
        match self {
            Self::Native(m) => m.transports.clone(),
            Self::Stremio(_) => vec![Transport::P2p, Transport::Http],
        }
    }

    /// What is stored once the user confirms: re-serialised from what was checked.
    pub fn to_json(&self) -> String {
        match self {
            Self::Native(m) => serde_json::to_string(m).expect("a manifest always serialises"),
            Self::Stremio(m) => m.to_json(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shape_decides_the_protocol() {
        let origin = Url::parse("https://addon.example/manifest.json").unwrap();
        let stremio = br#"{"id":"org.example.x","version":"1.0.0","name":"X","resources":["stream"],"types":["movie"]}"#;
        assert!(matches!(
            AddonManifest::parse(stremio, &origin),
            Ok(AddonManifest::Stremio(_))
        ));
        let native = br#"{"protocol":1,"id":"com.example.films","name":"Example","version":"1.0.0",
            "media":["film"],"transports":["http"],
            "endpoints":{"find":"https://addon.example/find","resolve":"https://addon.example/resolve"}}"#;
        assert!(matches!(
            AddonManifest::parse(native, &origin),
            Ok(AddonManifest::Native(_))
        ));
        // A native manifest that also lists resources is still refused as native would be.
        let mixed = br#"{"protocol":1,"resources":["stream"],"id":"com.example.films"}"#;
        assert!(AddonManifest::parse(mixed, &origin).is_err());
    }
}
