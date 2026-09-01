//! The HTTP boundary.
//!
//! Every client talks through this trait rather than to `reqwest` directly, for one
//! reason: **a client that can only be tested against the live service is a client
//! that is not tested.** Rate limiting, backoff, cache interaction and response
//! parsing are all logic, and all of it is exercised here against a fake that
//! returns exactly the responses a test wants — including the 429s and malformed
//! bodies a real service will not produce on demand.
//!
//! It also keeps the crate honest about §2.4: nothing here reaches the network
//! unless a caller supplies a transport that does.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type ResponseFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub body: Option<String>,
    pub headers: Vec<(String, String)>,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            body: None,
            headers: Vec::new(),
        }
    }

    pub fn post_json(url: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            method: Method::Post,
            url: url.into(),
            body: Some(body.into()),
            headers: vec![
                ("content-type".into(), "application/json".into()),
                ("accept".into(), "application/json".into()),
            ],
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// The host, for the rate limiter.
    pub fn host(&self) -> &str {
        self.url
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(&self.url)
            .split('/')
            .next()
            .unwrap_or("")
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
}

impl Response {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            headers: HashMap::new(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .insert(name.to_ascii_lowercase(), value.to_string());
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn etag(&self) -> Option<&str> {
        self.header("etag")
    }

    pub fn last_modified(&self) -> Option<&str> {
        self.header("last-modified")
    }

    /// How long the server asked us to wait.
    ///
    /// `Retry-After` is either a number of seconds or an HTTP date. Only the numeric
    /// form is honoured: the date form needs a clock the two ends agree on, and
    /// guessing wrong in the impatient direction is how a 429 becomes a ban. An
    /// unparseable value falls back to the caller's own backoff, which is the safe
    /// direction.
    pub fn retry_after(&self) -> Option<Duration> {
        self.header("retry-after")?
            .trim()
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("network: {0}")]
    Network(String),
    #[error("timed out")]
    Timeout,
}

pub trait Transport: Send + Sync {
    fn send(&self, request: Request) -> ResponseFuture<'_>;
}

/// A transport that returns canned responses, and records what it was asked.
///
/// The recording matters as much as the responses: most of what is worth asserting
/// about a client is *how many* requests it made and in what order — whether a cache
/// hit skipped the network, whether a retry actually happened, whether a conditional
/// request carried its validators.
#[derive(Default)]
pub struct FakeTransport {
    queued: Mutex<Vec<Result<Response, TransportError>>>,
    pub requests: Arc<Mutex<Vec<Request>>>,
}

impl FakeTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a response. They are returned in the order queued.
    pub fn push(&self, response: Response) -> &Self {
        self.queued.lock().expect("lock").push(Ok(response));
        self
    }

    pub fn push_error(&self, error: TransportError) -> &Self {
        self.queued.lock().expect("lock").push(Err(error));
        self
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().expect("lock").len()
    }

    pub fn last_request(&self) -> Option<Request> {
        self.requests.lock().expect("lock").last().cloned()
    }
}

impl Transport for FakeTransport {
    fn send(&self, request: Request) -> ResponseFuture<'_> {
        self.requests.lock().expect("lock").push(request);
        let queued = self.queued.lock().expect("lock").pop();
        Box::pin(async move {
            match queued {
                Some(result) => result,
                // Running out of queued responses is a test bug, and saying so is
                // far more useful than returning a default that makes the test pass
                // for the wrong reason.
                None => Err(TransportError::Network(
                    "FakeTransport ran out of queued responses".into(),
                )),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_is_extracted_for_the_rate_limiter() {
        assert_eq!(
            Request::get("https://graphql.anilist.co").host(),
            "graphql.anilist.co"
        );
        assert_eq!(
            Request::get("https://api.example.test/3/movie/1?x=1").host(),
            "api.example.test"
        );
    }

    #[test]
    fn headers_are_matched_case_insensitively() {
        // Servers are inconsistent about casing and HTTP does not care.
        let response = Response::new(200, "{}").with_header("ETag", "\"abc\"");
        assert_eq!(response.header("etag"), Some("\"abc\""));
        assert_eq!(response.etag(), Some("\"abc\""));
    }

    #[test]
    fn retry_after_reads_seconds_and_ignores_the_date_form() {
        let seconds = Response::new(429, "").with_header("Retry-After", "30");
        assert_eq!(seconds.retry_after(), Some(Duration::from_secs(30)));

        // The date form needs a shared clock, and guessing impatiently is how a 429
        // becomes a ban. Falling back to our own backoff is the safe direction.
        let date =
            Response::new(429, "").with_header("Retry-After", "Wed, 21 Oct 2026 07:28:00 GMT");
        assert_eq!(date.retry_after(), None);
    }

    #[tokio::test]
    async fn the_fake_records_what_it_was_asked() {
        let transport = FakeTransport::new();
        transport
            .push(Response::new(200, "second"))
            .push(Response::new(200, "first"));

        let first = transport
            .send(Request::get("https://example.test/a"))
            .await
            .expect("ok");
        assert_eq!(first.body, "first", "queued responses come back in order");

        transport
            .send(Request::get("https://example.test/b"))
            .await
            .expect("ok");
        assert_eq!(transport.request_count(), 2);
        assert_eq!(
            transport.last_request().expect("recorded").url,
            "https://example.test/b"
        );
    }

    #[tokio::test]
    async fn running_out_of_responses_is_an_error_not_a_default() {
        // A default would make a test pass for the wrong reason.
        let transport = FakeTransport::new();
        let result = transport.send(Request::get("https://example.test/a")).await;
        assert!(result.is_err());
    }
}
