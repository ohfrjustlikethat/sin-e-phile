//! `StremioBackend`: an add-on the USER installed that speaks Stremio's protocol
//! (ADR-0047). It is asked by IMDb id, `GET <base>/stream/<movie|series>/<id>.json`, and
//! its streams arrive whole, so there is no second step: a candidate's key is the stream.
//!
//! Like every add-on it goes through the untrusted transport (no redirects, capped
//! bodies) and its answers through a validator (`source_protocol::stremio`), so the
//! worst it can do is answer badly; the resolver's timeout bounds how long it takes.

use std::sync::Arc;

use sinephile_metadata_api::{Request, Transport};
use source_protocol::stremio::{parse_streams, stream_of, MAX_STREAMS_RESPONSE_BYTES};
use source_protocol::{
    async_trait, BackendCapabilities, MediaQuery, PlayableStream, SourceBackend, SourceCandidate,
    SourceError, StremioManifest, Transport as Wire,
};

pub struct StremioBackend {
    id: String,
    manifest: StremioManifest,
    http: Arc<dyn Transport>,
}

impl StremioBackend {
    /// `http` must be the UNTRUSTED transport (`HttpTransport::untrusted`).
    pub fn new(manifest: StremioManifest, http: Arc<dyn Transport>) -> Self {
        Self {
            id: format!("addon:{}", manifest.id),
            manifest,
            http,
        }
    }
}

#[async_trait]
impl SourceBackend for StremioBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: self.manifest.media(),
            transports: vec![Wire::P2p, Wire::Http],
            network: true,
        }
    }

    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        // No IMDb id, or a kind it does not serve: nothing to ask, and not a failure.
        let Some(url) = self.manifest.stream_url(query) else {
            return Ok(Vec::new());
        };
        let response = self
            .http
            .send(
                Request::get(url.as_str())
                    .header("accept", "application/json")
                    .limit(MAX_STREAMS_RESPONSE_BYTES),
            )
            .await
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        match response.status {
            200 => {}
            // Stremio add-ons answer 404 for a title they know nothing about.
            404 => return Ok(Vec::new()),
            300..=399 => {
                return Err(SourceError::InvalidResponse(
                    "the add-on tried to redirect, which is not allowed".into(),
                ))
            }
            s => return Err(SourceError::Unavailable(format!("the add-on answered {s}"))),
        }
        let (candidates, dropped) = parse_streams(response.body.as_bytes(), &self.id, query)
            .map_err(|e| SourceError::InvalidResponse(e.to_string()))?;
        if dropped > 0 {
            tracing::info!(addon = %self.id, dropped, "left out streams this player cannot open");
        }
        Ok(candidates)
    }

    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        stream_of(candidate).map_err(|e| SourceError::InvalidResponse(e.to_string()))
    }
}
