//! An addon catalogue: a list of manifests someone recommends.
//!
//! The app fetches one only from a URL the USER supplies. **No default URL ships**
//! (SPEC.md §2.1), so this module is a format and a validator, never an address.
//! Listing an addon here installs nothing: each entry is a manifest URL that goes
//! through exactly the same checks as one pasted by hand.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::manifest::{
    bounded_json, check_text, check_url, malformed, ManifestError, MAX_DESCRIPTION_CHARS,
    MAX_NAME_CHARS,
};
use crate::PROTOCOL_VERSION;

pub const MAX_CATALOGUE_BYTES: usize = 256 * 1024;
pub const MAX_CATALOGUE_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub protocol: u32,
    pub name: String,
    pub addons: Vec<CatalogueEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub manifest: String,
}

impl Catalogue {
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        let text = bounded_json(bytes, MAX_CATALOGUE_BYTES)?;
        let c: Catalogue = serde_json::from_str(text).map_err(malformed)?;
        if c.protocol != PROTOCOL_VERSION {
            return Err(ManifestError::UnsupportedProtocol(c.protocol));
        }
        check_text("name", &c.name, 1, MAX_NAME_CHARS)?;
        if c.addons.len() > MAX_CATALOGUE_ENTRIES {
            return Err(ManifestError::Field {
                field: "addons",
                problem: format!(
                    "has {} entries; the limit is {MAX_CATALOGUE_ENTRIES}",
                    c.addons.len()
                ),
            });
        }
        for e in &c.addons {
            check_text("addons[].name", &e.name, 1, MAX_NAME_CHARS)?;
            check_text(
                "addons[].description",
                &e.description,
                0,
                MAX_DESCRIPTION_CHARS,
            )?;
            check_url(&e.manifest)?;
        }
        Ok(c)
    }

    /// Entry `i`'s manifest address, already checked by `parse`.
    pub fn manifest_url(&self, i: usize) -> Option<Url> {
        self.addons
            .get(i)
            .and_then(|e| Url::parse(&e.manifest).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_lists_manifests_on_any_host() {
        let c = Catalogue::parse(
            br#"{"protocol":1,"name":"Picks","addons":[
                {"name":"One","manifest":"https://one.example/m.json"},
                {"name":"Two","description":"d","manifest":"https://two.example/m.json"}]}"#,
        )
        .unwrap();
        assert_eq!(c.addons.len(), 2);
        assert_eq!(c.manifest_url(1).unwrap().host_str(), Some("two.example"));
    }

    #[test]
    fn an_entry_with_an_insecure_manifest_rejects_the_catalogue() {
        let err = Catalogue::parse(
            br#"{"protocol":1,"name":"P","addons":[{"name":"One","manifest":"http://one.example/m.json"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::Url { .. }), "{err}");
    }
}
