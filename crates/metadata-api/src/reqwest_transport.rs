//! The real HTTP transport.
//!
//! Deliberately the smallest thing that satisfies [`Transport`]: it maps a request
//! onto `reqwest` and a response back, and does nothing else. Rate limiting,
//! retries, caching and parsing all live above it, where they can be tested against
//! [`crate::transport::FakeTransport`].
//!
//! There is nothing here worth a test that would not be a test of `reqwest`.

use std::collections::HashMap;
use std::time::Duration;

use crate::transport::{Method, Request, Response, ResponseFuture, Transport, TransportError};

pub struct HttpTransport {
    client: reqwest::Client,
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpTransport {
    pub fn new() -> Self {
        Self::build(reqwest::redirect::Policy::default())
    }

    /// For addresses a USER supplied — addon manifests and endpoints, catalogues.
    ///
    /// Redirects are not followed. A manifest's endpoints must be on the manifest's own
    /// host (`source_protocol::manifest`), and a redirect is a way round that rule:
    /// the endpoint passes the check, then answers 302 to somewhere else, or to plain
    /// http. A 3xx comes back to the caller as a status like any other, and is refused
    /// there.
    pub fn untrusted() -> Self {
        Self::build(reqwest::redirect::Policy::none())
    }

    fn build(redirects: reqwest::redirect::Policy) -> Self {
        let client = reqwest::Client::builder()
            .redirect(redirects)
            // Identify honestly. A metadata service is entitled to know what is
            // asking, and an anonymous scraper-shaped request is the one that gets
            // blocked.
            .user_agent(concat!(
                "sin-e-phile/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/ohfrjustlikethat/sin-e-phile)"
            ))
            .connect_timeout(Duration::from_secs(15))
            // A whole-request timeout is right here where it was wrong for dataset
            // downloads: an API response is small, so anything slow is stuck.
            .timeout(Duration::from_secs(30))
            .build()
            .expect("a client with no TLS backend cannot be built");
        Self { client }
    }
}

impl Transport for HttpTransport {
    fn send(&self, request: Request) -> ResponseFuture<'_> {
        Box::pin(async move {
            let mut builder = match request.method {
                Method::Get => self.client.get(&request.url),
                Method::Post => self.client.post(&request.url),
                Method::Head => self.client.head(&request.url),
            };
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }

            let response = builder.send().await.map_err(|error| {
                if error.is_timeout() {
                    TransportError::Timeout
                } else {
                    TransportError::Network(error.to_string())
                }
            })?;

            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|v| (name.as_str().to_ascii_lowercase(), v.to_string()))
                })
                .collect::<HashMap<_, _>>();

            // The body is read even for an error status: the message in it is
            // frequently the only thing that says what went wrong.
            let body = match request.max_body_bytes {
                None => response
                    .text()
                    .await
                    .map_err(|error| TransportError::Network(error.to_string()))?,
                Some(max) => read_capped(response, max).await?,
            };

            Ok(Response {
                status,
                body,
                headers,
            })
        })
    }
}

/// Read at most `max` bytes, chunk by chunk, and stop the moment the body exceeds it.
/// `Content-Length` is checked first, but not trusted: a server can omit it or lie, so
/// the running count is what actually enforces the cap.
async fn read_capped(
    mut response: reqwest::Response,
    max: usize,
) -> Result<String, TransportError> {
    if response.content_length().is_some_and(|n| n > max as u64) {
        return Err(TransportError::TooLarge(max));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| TransportError::Network(error.to_string()))?
    {
        if body.len() + chunk.len() > max {
            return Err(TransportError::TooLarge(max));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body)
        .map_err(|_| TransportError::Network("the response is not UTF-8 text".into()))
}
