//! `AddonBackend`: a source the USER installed, described by its manifest and served
//! by someone else's service speaking the protocol (`source_protocol::wire`).
//!
//! The app never runs an addon's code; it sends a query and parses the answer. Every
//! request goes through the untrusted transport — no redirects, capped bodies — and
//! every answer through the wire validator, so the worst an addon can do is answer
//! badly. The resolver's timeout bounds how long it can take to do so.

use std::sync::Arc;

use sinephile_metadata_api::{Request, Transport};
use source_protocol::wire::{
    parse_find, parse_resolve, ResolveRequest, MAX_FIND_RESPONSE_BYTES, MAX_RESOLVE_RESPONSE_BYTES,
};
use source_protocol::{
    async_trait, BackendCapabilities, Manifest, MediaQuery, PlayableStream, SourceBackend,
    SourceCandidate, SourceError,
};

/// Addons share the registry with built-ins, so their ids are namespaced: an addon
/// cannot install itself as `internet-archive` and take over its settings.
pub fn backend_id(manifest: &Manifest) -> String {
    format!("addon:{}", manifest.id)
}

pub struct AddonBackend {
    id: String,
    manifest: Manifest,
    http: Arc<dyn Transport>,
}

impl AddonBackend {
    /// `http` must be the UNTRUSTED transport (`HttpTransport::untrusted`).
    pub fn new(manifest: Manifest, http: Arc<dyn Transport>) -> Self {
        Self {
            id: backend_id(&manifest),
            manifest,
            http,
        }
    }

    async fn post(&self, url: &str, body: String, max: usize) -> Result<String, SourceError> {
        let response = self
            .http
            .send(Request::post_json(url, body).limit(max))
            .await
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        match response.status {
            200 => Ok(response.body),
            300..=399 => Err(SourceError::InvalidResponse(
                "the addon tried to redirect, which is not allowed".into(),
            )),
            s => Err(SourceError::Unavailable(format!("the addon answered {s}"))),
        }
    }
}

#[async_trait]
impl SourceBackend for AddonBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: self.manifest.media.clone(),
            transports: self.manifest.transports.clone(),
            network: true,
        }
    }

    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        let body = serde_json::to_string(query).expect("a query always serialises");
        let text = self
            .post(&self.manifest.endpoints.find, body, MAX_FIND_RESPONSE_BYTES)
            .await?;
        let (candidates, dropped) =
            parse_find(text.as_bytes()).map_err(|e| SourceError::InvalidResponse(e.to_string()))?;
        if dropped > 0 {
            tracing::info!(addon = %self.id, dropped, "ignored malformed candidates");
        }
        Ok(candidates)
    }

    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        let body = serde_json::to_string(&ResolveRequest {
            key: candidate.key.clone(),
        })
        .expect("a key always serialises");
        let text = self
            .post(
                &self.manifest.endpoints.resolve,
                body,
                MAX_RESOLVE_RESPONSE_BYTES,
            )
            .await?;
        parse_resolve(text.as_bytes()).map_err(|e| SourceError::InvalidResponse(e.to_string()))
    }
}
