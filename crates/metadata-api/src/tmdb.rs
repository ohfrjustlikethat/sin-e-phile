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

#[derive(Deserialize)]
struct Found {
    #[serde(default)]
    movie_results: Vec<Posted>,
    #[serde(default)]
    tv_results: Vec<Posted>,
}

#[derive(Deserialize)]
struct Posted {
    poster_path: Option<String>,
    #[serde(default)]
    backdrop_path: Option<String>,
}

/// TMDB's image server, at a width that suits a 176–236 px card at 125% (Phase 16).
pub const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w342";

/// A still across the top of a title's page (round 9, ADR-0052): wide enough for the page at 150%.
pub const BACKDROP_BASE: &str = "https://image.tmdb.org/t/p/w1280";

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

    /// The poster of a title, by its IMDb id, as an address on TMDB's image server (Phase 16,
    /// artwork with the profile's own key). `None` when TMDB has the title but no poster.
    pub async fn poster_url(&self, imdb_id: &str) -> Result<Option<String>, TmdbError> {
        self.image_url(imdb_id, |p| p.poster_path, POSTER_BASE)
            .await
    }

    /// A title's backdrop, a wide still, by its IMDb id (round 9, ADR-0052). `None` when TMDB
    /// has the title but no still.
    pub async fn backdrop_url(&self, imdb_id: &str) -> Result<Option<String>, TmdbError> {
        self.image_url(imdb_id, |p| p.backdrop_path, BACKDROP_BASE)
            .await
    }

    /// One of a title's images, found by IMDb id: only an IMDb id goes into the request, and
    /// only a plain path on TMDB's image server comes out.
    async fn image_url(
        &self,
        imdb_id: &str,
        pick: fn(Posted) -> Option<String>,
        base: &str,
    ) -> Result<Option<String>, TmdbError> {
        if !imdb_id.starts_with("tt") || !imdb_id[2..].chars().all(|c| c.is_ascii_digit()) {
            return Ok(None);
        }
        let body = self
            .get(&format!("/find/{imdb_id}?external_source=imdb_id"))
            .await?;
        let f: Found = serde_json::from_str(&body)
            .map_err(|e| TmdbError::Unavailable(format!("find: {e}")))?;
        Ok(f.movie_results
            .into_iter()
            .chain(f.tv_results)
            .find_map(pick)
            .filter(|p| p.starts_with('/') && !p.contains(".."))
            .map(|p| format!("{base}{p}")))
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

    #[tokio::test]
    async fn a_poster_is_found_by_imdb_id_and_nothing_else_is_asked() {
        let fake = Arc::new(FakeTransport::new());
        fake.push(Response::new(
            200,
            r#"{"movie_results":[],"tv_results":[{"poster_path":"/abc.jpg"}]}"#,
        ));
        let got = client(&fake).poster_url("tt0098936").await.unwrap();
        assert_eq!(
            got.as_deref(),
            Some("https://image.tmdb.org/t/p/w342/abc.jpg")
        );
        let asked = fake.requests.lock().unwrap()[0].url.clone();
        assert!(asked.starts_with(
            "https://api.themoviedb.org/3/find/tt0098936?external_source=imdb_id&api_key="
        ));
        // Not an IMDb id: nothing is asked at all.
        assert_eq!(client(&fake).poster_url("../config").await.unwrap(), None);
        assert_eq!(fake.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_backdrop_is_found_by_imdb_id_at_the_wide_size() {
        let fake = Arc::new(FakeTransport::new());
        // The fake answers last-pushed first.
        fake.push(Response::new(
            200,
            r#"{"movie_results":[{"poster_path":"/p.jpg","backdrop_path":"/../x"}],"tv_results":[]}"#,
        ));
        fake.push(Response::new(
            200,
            r#"{"movie_results":[{"poster_path":"/p.jpg","backdrop_path":"/wide.jpg"}],"tv_results":[]}"#,
        ));
        let got = client(&fake).backdrop_url("tt0105665").await.unwrap();
        assert_eq!(
            got.as_deref(),
            Some("https://image.tmdb.org/t/p/w1280/wide.jpg")
        );
        // A path that leaves the image server's folder is not followed.
        assert_eq!(client(&fake).backdrop_url("tt0105665").await.unwrap(), None);
    }
}
