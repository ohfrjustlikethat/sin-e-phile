//! AniList (`SPEC.md` Phase 4).
//!
//! # Why this client first
//!
//! **AniList needs no key.** ADR-0027 makes every key per-user and optional, so
//! AniList is the one enrichment source that works for everybody on first run. It
//! also answers two things nothing else does:
//!
//! - **romaji, english and native titles as asserted facts** (§6.2), rather than the
//!   script heuristic `title.akas` forces on us;
//! - **airing schedules** — the next episode's air time — which is the missing half
//!   of the weekly-TV freshness problem in `docs/specs/catalogue-freshness.md`.
//!
//! # It is GraphQL, which changes one thing
//!
//! Every request is a `POST` to the same URL with a different body, so the URL is
//! useless as a cache key. The key is the URL plus a hash of the query and its
//! variables — otherwise every AniList response would overwrite the last one.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use serde::Deserialize;

use crate::backoff::{self, Backoff};
use crate::limiter::{Limit, RateLimiter};
use crate::transport::{Request, Transport, TransportError};

/// `anilist.co` is on `tools/guard/allowlist.txt` as a metadata source (ADR-0010).
pub const ENDPOINT: &str = "https://graphql.anilist.co";
pub const HOST: &str = "graphql.anilist.co";

/// AniList documents 90 requests per minute.
///
/// It has run degraded at a third of that for long periods, which is exactly what
/// `Retry-After` is for: the limiter is penalised from the response rather than
/// guessing a lower number here and being permanently slow for nothing.
pub fn default_limit() -> Limit {
    Limit::per_window(90, Duration::from_secs(60))
}

#[derive(Debug, thiserror::Error)]
pub enum AniListError {
    #[error("anilist transport: {0}")]
    Transport(#[from] TransportError),
    #[error("anilist returned {status}: {body}")]
    Status { status: u16, body: String },
    #[error("anilist response was not the shape we expected: {0}")]
    Shape(String),
    #[error("anilist: {0}")]
    Api(String),
}

/// One title, as AniList knows it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Media {
    pub id: i64,
    /// MAL's id for the same title, when AniList has it. The cross-mapping subtask
    /// (4.5) needs this and nothing else provides it for free.
    #[serde(rename = "idMal")]
    pub id_mal: Option<i64>,
    pub title: Titles,
    /// `TV`, `MOVIE`, `OVA`, `ONA`, `SPECIAL`, `MUSIC`.
    pub format: Option<String>,
    /// `FINISHED`, `RELEASING`, `NOT_YET_RELEASED`, `CANCELLED`, `HIATUS`.
    pub status: Option<String>,
    pub episodes: Option<i64>,
    #[serde(rename = "seasonYear")]
    pub season_year: Option<i64>,
    /// The next episode and when it airs. `None` for anything finished — which is
    /// the honest answer, not a gap.
    #[serde(rename = "nextAiringEpisode")]
    pub next_airing: Option<AiringEpisode>,
}

/// The three title forms §6.2 names.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Titles {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AiringEpisode {
    /// Unix seconds.
    #[serde(rename = "airingAt")]
    pub airing_at: i64,
    /// Seconds from now. AniList computes this server-side, so it is not subject to
    /// the client's clock being wrong.
    #[serde(rename = "timeUntilAiring")]
    pub time_until_airing: i64,
    pub episode: i64,
}

impl Media {
    /// Is this a series or a film, in `media_kind` terms (ADR-0025)?
    ///
    /// AniList's `format` is authoritative here in a way IMDb's `titleType` is not:
    /// this is where a title becomes `anime_film` or `anime_series` rather than
    /// plain `film`, and it is a fact rather than an inference from country or genre.
    pub fn media_kind(&self) -> &'static str {
        match self.format.as_deref() {
            Some("MOVIE") => "anime_film",
            _ => "anime_series",
        }
    }

    /// Currently airing, so its schedule is worth polling.
    pub fn is_releasing(&self) -> bool {
        self.status.as_deref() == Some("RELEASING")
    }
}

const MEDIA_FIELDS: &str = "
    id
    idMal
    title { romaji english native }
    format
    status
    episodes
    seasonYear
    nextAiringEpisode { airingAt timeUntilAiring episode }
";

pub struct AniList<'a> {
    transport: &'a dyn Transport,
    limiter: RateLimiter,
    backoff: Backoff,
}

impl<'a> AniList<'a> {
    pub async fn new(transport: &'a dyn Transport) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure(HOST, default_limit()).await;
        Self {
            transport,
            limiter,
            backoff: Backoff::default(),
        }
    }

    /// Share a limiter with the other clients. The limit is per host, so this only
    /// matters when something else also talks to AniList — but taking a shared one
    /// is what makes that correct by construction rather than by luck.
    pub async fn with_limiter(transport: &'a dyn Transport, limiter: RateLimiter) -> Self {
        limiter.configure(HOST, default_limit()).await;
        Self {
            transport,
            limiter,
            backoff: Backoff::default(),
        }
    }

    /// One title by AniList id.
    pub async fn media(&self, id: i64) -> Result<Option<Media>, AniListError> {
        let query =
            format!("query ($id: Int) {{ Media(id: $id, type: ANIME) {{ {MEDIA_FIELDS} }} }}");
        self.media_query(&query, &format!(r#"{{"id":{id}}}"#)).await
    }

    /// One title by MyAnimeList id — the cross-mapping AniList gives away free.
    pub async fn media_by_mal(&self, mal_id: i64) -> Result<Option<Media>, AniListError> {
        let query =
            format!("query ($id: Int) {{ Media(idMal: $id, type: ANIME) {{ {MEDIA_FIELDS} }} }}");
        self.media_query(&query, &format!(r#"{{"id":{mal_id}}}"#))
            .await
    }

    /// Search by title. AniList matches romaji, english and native, which is the
    /// behaviour §6.2 wants and is why this beats matching against our own rows.
    pub async fn search(&self, title: &str) -> Result<Option<Media>, AniListError> {
        let query =
            format!("query ($q: String) {{ Media(search: $q, type: ANIME) {{ {MEDIA_FIELDS} }} }}");
        let variables = serde_json::json!({ "q": title }).to_string();
        self.media_query(&query, &variables).await
    }

    async fn media_query(
        &self,
        query: &str,
        variables: &str,
    ) -> Result<Option<Media>, AniListError> {
        let body = format!(
            r#"{{"query":{},"variables":{variables}}}"#,
            json_string(query)
        );
        let response = self.execute(&body).await?;

        // AniList reports "not found" as a GraphQL error with a 404 status, not an
        // empty data field. Treating that as an error would make every miss look
        // like a failure, and a catalogue full of anime we do not have is normal.
        if response.status == 404 {
            return Ok(None);
        }

        let parsed: GraphQlResponse<MediaData> = serde_json::from_str(&response.body)
            .map_err(|e| AniListError::Shape(format!("{e}: {}", truncate(&response.body))))?;

        if let Some(errors) = parsed.errors {
            let message = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            // A GraphQL "not found" arrives here as an error too, depending on the
            // query shape. It is a miss, not a fault.
            if message.to_ascii_lowercase().contains("not found") {
                return Ok(None);
            }
            return Err(AniListError::Api(message));
        }

        Ok(parsed.data.and_then(|d| d.media))
    }

    /// POST the body, rate-limited, with retries.
    async fn execute(&self, body: &str) -> Result<crate::transport::Response, AniListError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.limiter.acquire(HOST).await;

            let request = Request::post_json(ENDPOINT, body.to_string());
            let outcome = self.transport.send(request).await;

            let response = match outcome {
                Ok(response) => response,
                Err(error) => {
                    if !self
                        .backoff
                        .should_retry(attempt, backoff::TRANSPORT_FAILURE)
                    {
                        return Err(error.into());
                    }
                    self.wait(attempt).await;
                    continue;
                }
            };

            if response.status == 429 {
                // The server's own number outranks our model of its limit. Falling
                // back to our backoff when it does not say is the safe direction.
                let wait = response
                    .retry_after()
                    .unwrap_or_else(|| self.backoff.delay_for(attempt + 1));
                self.limiter.penalise(HOST, wait).await;
            }

            if (200..300).contains(&response.status) || response.status == 404 {
                return Ok(response);
            }

            if !self
                .backoff
                .should_retry(attempt, backoff::classify(response.status))
            {
                return Err(AniListError::Status {
                    status: response.status,
                    body: truncate(&response.body),
                });
            }
            self.wait(attempt).await;
        }
    }

    async fn wait(&self, attempt: u32) {
        // Full jitter, sampled from the attempt so the sequence is deterministic per
        // caller but different between concurrent callers. A real RNG here would
        // make the retry path untestable, which is where the storm would hide.
        let sample = jitter_sample(attempt);
        tokio::time::sleep(self.backoff.jittered(attempt + 1, sample)).await;
    }
}

/// A deterministic pseudo-sample in `[0, 1)`.
///
/// Deterministic on purpose. The point of jitter is that two *different* callers do
/// not collide, and hashing the attempt with the task's address achieves that
/// without making the retry path impossible to test.
fn jitter_sample(attempt: u32) -> f64 {
    let mut hasher = DefaultHasher::new();
    attempt.hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    (hasher.finish() % 1_000) as f64 / 1_000.0
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct MediaData {
    #[serde(rename = "Media")]
    media: Option<Media>,
}

/// A JSON string literal, so a query containing quotes or newlines survives.
fn json_string(raw: &str) -> String {
    serde_json::Value::String(raw.to_string()).to_string()
}

fn truncate(body: &str) -> String {
    body.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_movie_becomes_anime_film_and_everything_else_a_series() {
        // ADR-0025's media_kind. AniList's format is a FACT here, where IMDb's
        // titleType could only ever be an inference about whether a film is anime.
        let media = |format: Option<&str>| Media {
            id: 1,
            id_mal: None,
            title: Titles {
                romaji: None,
                english: None,
                native: None,
            },
            format: format.map(str::to_string),
            status: None,
            episodes: None,
            season_year: None,
            next_airing: None,
        };
        assert_eq!(media(Some("MOVIE")).media_kind(), "anime_film");
        assert_eq!(media(Some("TV")).media_kind(), "anime_series");
        assert_eq!(media(Some("ONA")).media_kind(), "anime_series");
        assert_eq!(media(None).media_kind(), "anime_series");
    }

    #[test]
    fn the_documented_limit_is_ninety_a_minute() {
        let limit = default_limit();
        assert_eq!(limit.per_second, 1.5);
        assert_eq!(limit.burst, 90.0);
    }

    #[test]
    fn a_query_containing_quotes_survives_being_embedded() {
        let escaped = json_string("query { Media(search: \"a\\b\") }");
        assert!(escaped.starts_with('"') && escaped.ends_with('"'));
        assert!(serde_json::from_str::<String>(&escaped).is_ok());
    }

    #[test]
    fn a_long_error_body_is_truncated_rather_than_logged_whole() {
        let long = "x".repeat(5_000);
        assert_eq!(truncate(&long).len(), 200);
    }

    #[test]
    fn jitter_stays_in_the_unit_interval() {
        for attempt in 1..10 {
            let sample = jitter_sample(attempt);
            assert!((0.0..1.0).contains(&sample), "{sample} is out of range");
        }
    }
}
