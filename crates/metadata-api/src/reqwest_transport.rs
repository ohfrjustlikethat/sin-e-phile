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
        let client = reqwest::Client::builder()
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
            let body = response
                .text()
                .await
                .map_err(|error| TransportError::Network(error.to_string()))?;

            Ok(Response {
                status,
                body,
                headers,
            })
        })
    }
}
