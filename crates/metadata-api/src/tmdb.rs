//! TMDB, with the viewer's own key (ADR-0013, ADR-0027: optional, per profile, none ships).
//!
//! Phase 16 uses it for one thing the offline catalogue cannot know: what is in cinemas and
//! on the air **now** (IMDb's datasets carry years, not release dates). Each title comes back
//! as TMDB's id, so its IMDb id is asked for separately, and the catalogue matches on that.
//! Everything that needs TMDB degrades to the offline catalogue without it.

use std::sync::Arc;

use serde::Deserialize;

use crate::transport::{Request, Transport};

const BASE: &str = "https://api.themoviedb.org/3";
/// A page of results is a few kilobytes; nothing here is large.
const MAX_BODY: usize = 1 << 20;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TmdbError {
    #[error("TMDB did not accept that key")]
    BadKey,
    #[error("TMDB could not be reached: {0}")]
    Unavailable(String),
}

/// What is showing now, by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Now {
    /// Films in cinemas.
    Films,
    /// Series airing this week.
    Series,
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    results: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    id: i64,
}

#[derive(Deserialize)]
struct External {
    imdb_id: Option<String>,
}

pub struct Tmdb {
    transport: Arc<dyn Transport>,
    key: String,
}

impl Tmdb {
    pub fn new(transport: Arc<dyn Transport>, key: impl Into<String>) -> Self {
        Self {
            transport,
            key: key.into(),
        }
    }

    async fn get(&self, path: &str) -> Result<String, TmdbError> {
        let sep = if path.contains('?') { '&' } else { '?' };
        let url = format!("{BASE}{path}{sep}api_key={}", self.key);
        let r = self
            .transport
            .send(Request::get(url).limit(MAX_BODY))
            .await
            .map_err(|e| TmdbError::Unavailable(e.to_string()))?;
        match r.status {
            200 => Ok(r.body),
            401 => Err(TmdbError::BadKey),
            s => Err(TmdbError::Unavailable(format!("TMDB answered {s}"))),
        }
    }

    /// Whether the key is accepted: one small request.
    pub async fn check(&self) -> Result<(), TmdbError> {
        self.get("/configuration").await.map(|_| ())
    }

    /// TMDB's ids of what is showing now: the first `pages` pages (twenty a page).
    pub async fn now(&self, what: Now, pages: u32) -> Result<Vec<i64>, TmdbError> {
        let path = match what {
            Now::Films => "/movie/now_playing",
            Now::Series => "/tv/on_the_air",
        };
        let mut ids = Vec::new();
        for page in 1..=pages.max(1) {
            let body = self.get(&format!("{path}?page={page}")).await?;
            let p: Page = serde_json::from_str(&body)
                .map_err(|e| TmdbError::Unavailable(format!("now: {e}")))?;
            ids.extend(p.results.into_iter().map(|i| i.id));
        }
        Ok(ids)
    }

    /// The IMDb id TMDB holds for one of its titles, if it holds one.
    pub async fn imdb_id(&self, what: Now, tmdb_id: i64) -> Result<Option<String>, TmdbError> {
        let kind = match what {
            Now::Films => "movie",
            Now::Series => "tv",
        };
        let body = self.get(&format!("/{kind}/{tmdb_id}/external_ids")).await?;
        let e: External = serde_json::from_str(&body)
            .map_err(|e| TmdbError::Unavailable(format!("external ids: {e}")))?;
        Ok(e.imdb_id.filter(|i| i.starts_with("tt")))
    }

    /// What is showing now, as IMDb ids, in TMDB's order (most popular first).
    pub async fn now_imdb(&self, what: Now, pages: u32) -> Result<Vec<String>, TmdbError> {
        let mut out = Vec::new();
        for id in self.now(what, pages).await? {
            if let Some(imdb) = self.imdb_id(what, id).await? {
                out.push(imdb);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{FakeTransport, Response};

    // Obviously not a key: valid in shape, so scanners and readers are not misled.
    const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn client(fake: &Arc<FakeTransport>) -> Tmdb {
        Tmdb::new(fake.clone() as Arc<dyn Transport>, KEY)
    }

    #[tokio::test]
    async fn a_key_is_checked_and_a_refused_one_says_so() {
        let fake = Arc::new(FakeTransport::new());
        fake.push(Response::new(200, "{}"));
        assert_eq!(client(&fake).check().await, Ok(()));
        fake.push(Response::new(401, "{}"));
        assert_eq!(client(&fake).check().await, Err(TmdbError::BadKey));
    }

    #[tokio::test]
    async fn what_is_showing_now_comes_back_as_imdb_ids_in_order() {
        let fake = Arc::new(FakeTransport::new());
        // The fake answers last pushed first: the external ids, then the page.
        fake.push(Response::new(200, r#"{"imdb_id":"tt0000002"}"#));
        fake.push(Response::new(200, r#"{"imdb_id":null}"#));
        fake.push(Response::new(200, r#"{"imdb_id":"tt0000001"}"#));
        fake.push(Response::new(
            200,
            r#"{"results":[{"id":11},{"id":12},{"id":13}]}"#,
        ));
        let got = client(&fake).now_imdb(Now::Films, 1).await.unwrap();
        assert_eq!(
            got,
            ["tt0000001", "tt0000002"],
            "a title without an IMDb id is left out"
        );
        let asked: Vec<String> = fake
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.url.clone())
            .collect();
        assert!(
            asked[0].starts_with("https://api.themoviedb.org/3/movie/now_playing?page=1&api_key=")
        );
        assert!(asked[1].contains("/movie/11/external_ids?api_key="));
    }
}
