//! Installing an addon: paste a URL, see what it declares, confirm, enable.
//!
//! Each step is a function here so the IPC layer only calls them in order (ADR-0022).
//! Nothing is stored until [`install`], which is only reached after the user has seen
//! [`describe`]'s plain-language summary and said yes (SPEC.md §15 Phase 6: "no raw
//! JSON in the primary path").

use sinephile_metadata_api::{Request, Transport};
use sinephile_persistence::repositories::{AddonRow, SourceRepository};
use sinephile_persistence::{Db, DbError};
use source_protocol::manifest::MAX_MANIFEST_BYTES;
use source_protocol::{
    parse_manifest_url, Catalogue, Manifest, ManifestError, MediaKind, Transport as Wire,
};
use url::Url;

use crate::backends::addon::backend_id;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0}")]
    Manifest(#[from] ManifestError),
    #[error("could not reach {host}: {reason}")]
    Unreachable { host: String, reason: String },
    #[error("{host} answered {status}{hint}")]
    Status {
        host: String,
        status: u16,
        hint: &'static str,
    },
    #[error("{0}")]
    Store(#[from] DbError),
}

/// Fetch and check a manifest from an address the user pasted.
pub async fn fetch_manifest(
    http: &dyn Transport,
    pasted: &str,
) -> Result<(Url, Manifest), InstallError> {
    let url = parse_manifest_url(pasted)?;
    let bytes = fetch(http, &url, MAX_MANIFEST_BYTES).await?;
    let manifest = Manifest::parse(bytes.as_bytes(), &url)?;
    Ok((url, manifest))
}

/// Fetch and check a catalogue from an address the user supplied. **No default URL
/// exists anywhere in the app** (SPEC.md §2.1).
pub async fn fetch_catalogue(
    http: &dyn Transport,
    pasted: &str,
) -> Result<Catalogue, InstallError> {
    let url = parse_manifest_url(pasted)?;
    let bytes = fetch(http, &url, source_protocol::catalogue::MAX_CATALOGUE_BYTES).await?;
    Ok(Catalogue::parse(bytes.as_bytes())?)
}

async fn fetch(http: &dyn Transport, url: &Url, max: usize) -> Result<String, InstallError> {
    let host = url.host_str().unwrap_or("that address").to_string();
    let response = http
        .send(
            Request::get(url.as_str())
                .header("accept", "application/json")
                .limit(max),
        )
        .await
        .map_err(|e| InstallError::Unreachable {
            host: host.clone(),
            reason: e.to_string(),
        })?;
    match response.status {
        200 => Ok(response.body),
        status @ 300..=399 => Err(InstallError::Status {
            host,
            status,
            hint: " (a redirect); paste the address it redirects to instead",
        }),
        404 => Err(InstallError::Status {
            host,
            status: 404,
            hint: ": there is no manifest at that address",
        }),
        status => Err(InstallError::Status {
            host,
            status,
            hint: "",
        }),
    }
}

/// What the confirm screen shows: every claim the manifest makes, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    pub backend_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    /// "Finds films and TV episodes."
    pub finds: String,
    /// "Streams over the web." / "Shares peer to peer, like a torrent."
    pub delivers: Vec<String>,
    /// The one host every query will be sent to.
    pub host: String,
    pub homepage: Option<String>,
    /// True if this replaces an installed addon with the same id.
    pub replaces: bool,
}

pub fn describe(url: &Url, manifest: &Manifest, installed: &[AddonRow]) -> Description {
    let id = backend_id(manifest);
    let kinds: Vec<&str> = manifest
        .media
        .iter()
        .filter_map(|k| match k {
            MediaKind::Film => Some("films"),
            MediaKind::Episode => Some("TV episodes"),
            MediaKind::Unknown => None,
        })
        .collect();
    let delivers = manifest
        .transports
        .iter()
        .filter_map(|t| match t {
            Wire::Http => Some("Streams over the web.".to_string()),
            Wire::P2p => Some("Shares peer to peer, like a torrent.".to_string()),
            Wire::Local => Some("Points at files on your own computer.".to_string()),
            Wire::Unknown => None,
        })
        .collect();
    Description {
        replaces: installed.iter().any(|a| a.backend_id == id),
        backend_id: id,
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        finds: format!("Finds {}.", kinds.join(" and ")),
        delivers,
        host: url.host_str().unwrap_or_default().to_string(),
        homepage: manifest.homepage.clone(),
    }
}

/// Store a manifest the user confirmed. It is re-serialised from the checked struct,
/// never stored as the bytes that arrived, so what is kept is exactly what was checked.
pub async fn install(db: &Db, url: &Url, manifest: &Manifest) -> Result<String, InstallError> {
    let id = backend_id(manifest);
    let json = serde_json::to_string(manifest).expect("a manifest always serialises");
    SourceRepository::new(db)
        .install_addon(&id, url.as_str(), &json)
        .await?;
    Ok(id)
}

/// Every installed manifest, checked again on the way out of the database. A row that
/// no longer passes — edited by hand, or written by an older, laxer build — is left
/// out and reported, not loaded.
pub async fn installed(db: &Db) -> Result<(Vec<(Url, Manifest)>, Vec<String>), DbError> {
    let rows = SourceRepository::new(db).addons().await?;
    let (mut ok, mut refused) = (Vec::new(), Vec::new());
    for row in rows {
        let checked = Url::parse(&row.manifest_url).ok().and_then(|url| {
            Manifest::parse(row.manifest.as_bytes(), &url)
                .ok()
                .map(|m| (url, m))
        });
        match checked {
            Some(pair) => ok.push(pair),
            None => refused.push(row.backend_id),
        }
    }
    Ok((ok, refused))
}
