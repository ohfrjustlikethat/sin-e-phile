//! Wikidata and Wikipedia: where the catalogue gets text about what a film is *about*.
//!
//! Two endpoints, two jobs (ADR-0033):
//!
//! - **Wikidata Query Service** maps an IMDb id to an English Wikipedia article through
//!   property **P345**. An exact join on IMDb's own identifier, not a title-and-year
//!   guess — the same standard the anime matcher is held to.
//! - **The Wikipedia action API** returns the lead extract for up to 20 articles per
//!   request, which is what actually goes into the embedding.
//!
//! # Neither endpoint needs a key, and that is the point
//!
//! ADR-0013 and ADR-0027 built the whole catalogue around being complete with no API
//! key. TMDB would have covered more titles and better, and it would have made the
//! artefact depend on a key somebody had to own. See ADR-0033.
//!
//! # Be a good citizen or be blocked
//!
//! Both services are donation-funded and both ask for a descriptive `User-Agent` with a
//! way to reach the operator; Wikimedia blocks generic ones. The rate limits below are
//! deliberately gentler than anything published, because a one-off catalogue build has
//! no reason to hurry and being throttled costs more than going slowly.

use crate::backoff::{self, Backoff};
use crate::limiter::{Limit, RateLimiter};
use crate::transport::{Request, Response, Transport, TransportError};

pub const WDQS_ENDPOINT: &str = "https://query.wikidata.org/sparql";
pub const WDQS_HOST: &str = "query.wikidata.org";
pub const WIKIPEDIA_ENDPOINT: &str = "https://en.wikipedia.org/w/api.php";
pub const WIKIPEDIA_HOST: &str = "en.wikipedia.org";

/// Identifies this project to Wikimedia, as their policy requires.
///
/// A generic agent is blocked outright, and an anonymous one is impolite: if this job
/// ever misbehaves, whoever notices should be able to find out whose it is.
pub const USER_AGENT: &str =
    "sin-e-phile/0.1 (portfolio project; https://github.com/ohfrjustlikethat/sin-e-phile)";

/// One SPARQL query at a time, with a long pause between.
///
/// WDQS gives an anonymous client roughly a minute of query time and enforces
/// concurrency limits; a mapping chunk measured 11.6 s on its own. One every three
/// seconds finishes the whole mapping in under an hour and never queues.
pub fn wdqs_limit() -> Limit {
    Limit::new(1.0 / 3.0, 1.0)
}

/// The action API is cheap by comparison, but it is still someone else's server.
///
/// 20 articles per request at two requests a second covers the catalogue overnight.
pub fn wikipedia_limit() -> Limit {
    Limit::new(2.0, 4.0)
}

/// How many article titles fit in one extracts request.
///
/// **The API's own cap for `prop=extracts` is 20**, and it does not error when given
/// more — it silently truncates, which would look like missing articles rather than a
/// misuse. Named here so the batching cannot drift past it by accident.
pub const EXTRACTS_PER_REQUEST: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("wikipedia transport: {0}")]
    Transport(#[from] TransportError),
    #[error("{host} returned {status}: {body}")]
    Status {
        host: String,
        status: u16,
        body: String,
    },
    #[error("{host} returned a body that is not the shape documented: {detail}")]
    Shape { host: String, detail: String },
}

/// An IMDb id and the article that describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub imdb_id: String,
    /// The article *title*, not its URL — that is what the extracts API takes.
    pub article: String,
}

/// An article's lead extract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extract {
    /// The article's CURRENT title, as Wikipedia answered.
    pub article: String,
    /// The title we ASKED for, when it differs — a redirect or a normalisation.
    ///
    /// Without this the caller cannot pair an answer with its question. Wikidata's
    /// sitelink and Wikipedia's current title disagree whenever an article has been
    /// moved, which for a catalogue of this age is thousands of films, and every one of
    /// them would look missing rather than renamed.
    pub requested: Option<String>,
    pub text: String,
}

impl Extract {
    /// The title this answers, whichever name was used to ask.
    pub fn answers(&self) -> &str {
        self.requested.as_deref().unwrap_or(&self.article)
    }
}

pub struct Wikipedia<'a> {
    transport: &'a dyn Transport,
    limiter: RateLimiter,
    backoff: Backoff,
}

impl<'a> Wikipedia<'a> {
    pub async fn new(transport: &'a dyn Transport) -> Wikipedia<'a> {
        let limiter = RateLimiter::new();
        limiter.configure(WDQS_HOST, wdqs_limit()).await;
        limiter.configure(WIKIPEDIA_HOST, wikipedia_limit()).await;
        Self {
            transport,
            limiter,
            backoff: Backoff::default(),
        }
    }

    /// Every IMDb id beginning with `prefix` that has an English Wikipedia article.
    ///
    /// Chunked by prefix because **WDQS times out at 60 seconds** and the unfiltered
    /// query needs 59 of them: measured 2026-09-07, `COUNT(*)` over the whole join
    /// returned 509,464 in 58.9 s, while the `tt004` prefix returned 7,513 rows in
    /// 11.6 s. The caller subdivides when a chunk is still too large, so the prefix
    /// length adapts to how ids are actually distributed rather than to a guess about
    /// it.
    pub async fn mappings(&self, prefix: &str) -> Result<Vec<Mapping>, WikiError> {
        let query = format!(
            "SELECT ?imdb ?article WHERE {{ \
               ?item wdt:P345 ?imdb . \
               ?article schema:about ?item ; schema:isPartOf <https://en.wikipedia.org/> . \
               FILTER(STRSTARTS(?imdb, \"{prefix}\")) \
             }}"
        );
        let url = format!("{WDQS_ENDPOINT}?query={}", encode(&query));
        let request = Request::get(url)
            .header("accept", "application/sparql-results+json")
            .header("user-agent", USER_AGENT);

        let response = self.execute(WDQS_HOST, request).await?;
        parse_mappings(&response.body)
    }

    /// Lead extracts for up to [`EXTRACTS_PER_REQUEST`] articles.
    ///
    /// `redirects=1` because Wikidata's sitelink and Wikipedia's current title disagree
    /// whenever an article has been moved, and following the redirect is the difference
    /// between an extract and a silent miss.
    pub async fn extracts(&self, articles: &[String]) -> Result<Vec<Extract>, WikiError> {
        if articles.is_empty() {
            return Ok(Vec::new());
        }
        let titles = articles.join("|");
        let url = format!(
            "{WIKIPEDIA_ENDPOINT}?action=query&format=json&formatversion=2\
             &prop=extracts&exintro=1&explaintext=1&exlimit={EXTRACTS_PER_REQUEST}\
             &redirects=1&titles={}",
            encode(&titles)
        );
        let request = Request::get(url)
            .header("accept", "application/json")
            .header("user-agent", USER_AGENT);

        let response = self.execute(WIKIPEDIA_HOST, request).await?;
        parse_extracts(&response.body)
    }

    /// Send, respecting the limiter, retrying what is worth retrying.
    ///
    /// The same shape as the AniList client's, for the same reasons — a 429 penalises
    /// the limiter from the server's own `Retry-After` rather than from our model of
    /// its limits, because the server's number outranks ours.
    async fn execute(&self, host: &str, request: Request) -> Result<Response, WikiError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.limiter.acquire(host).await;

            let response = match self.transport.send(request.clone()).await {
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
                let wait = response
                    .retry_after()
                    .unwrap_or_else(|| self.backoff.delay_for(attempt + 1));
                self.limiter.penalise(host, wait).await;
            }

            if (200..300).contains(&response.status) {
                return Ok(response);
            }

            if !self
                .backoff
                .should_retry(attempt, backoff::classify(response.status))
            {
                return Err(WikiError::Status {
                    host: host.to_string(),
                    status: response.status,
                    body: response.body.chars().take(200).collect(),
                });
            }
            self.wait(attempt).await;
        }
    }

    async fn wait(&self, attempt: u32) {
        let sample = (attempt as f64 * 0.618).fract();
        tokio::time::sleep(self.backoff.jittered(attempt + 1, sample)).await;
    }
}

/// Percent-encode everything a query string cannot carry literally.
///
/// Written out rather than taking a dependency: SPARQL is full of `{}`, `<>`, `"` and
/// `#`, and a URL crate would be a whole dependency for one function. `#` in particular
/// would silently truncate the query at the fragment.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The article title from a Wikipedia URL, with underscores turned back into spaces.
fn article_from_url(url: &str) -> Option<String> {
    let slug = url.rsplit_once("/wiki/")?.1;
    let decoded = percent_decode(slug);
    Some(decoded.replace('_', " "))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// SPARQL JSON results into mappings.
///
/// Hand-parsed rather than deserialised into a typed tree: the shape is three levels of
/// wrapper around two strings, and a serde model of it would be more code than this.
fn parse_mappings(body: &str) -> Result<Vec<Mapping>, WikiError> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| WikiError::Shape {
        host: WDQS_HOST.into(),
        detail: e.to_string(),
    })?;
    let bindings = value["results"]["bindings"]
        .as_array()
        .ok_or_else(|| WikiError::Shape {
            host: WDQS_HOST.into(),
            detail: "results.bindings is not an array".into(),
        })?;

    let mut out = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let (Some(imdb), Some(url)) = (
            binding["imdb"]["value"].as_str(),
            binding["article"]["value"].as_str(),
        ) else {
            continue;
        };
        // A row missing either half is skipped rather than failing the chunk: one
        // malformed Wikidata item should not cost the other 7,512.
        if let Some(article) = article_from_url(url) {
            out.push(Mapping {
                imdb_id: imdb.to_string(),
                article,
            });
        }
    }
    Ok(out)
}

/// Action-API JSON into extracts.
///
/// `formatversion=2` makes `pages` an array rather than an object keyed by page id,
/// which is both easier to read and stable — the page-id keys change between wikis.
fn parse_extracts(body: &str) -> Result<Vec<Extract>, WikiError> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| WikiError::Shape {
        host: WIKIPEDIA_HOST.into(),
        detail: e.to_string(),
    })?;
    let pages = value["query"]["pages"]
        .as_array()
        .ok_or_else(|| WikiError::Shape {
            host: WIKIPEDIA_HOST.into(),
            detail: "query.pages is not an array (formatversion=2 expected)".into(),
        })?;

    // `to` -> `from`, so an answer can be traced back to the question. The API reports
    // two kinds of rewriting and BOTH have to be reversed: `normalized` (underscores,
    // capitalisation) happens first, then `redirects` (the article was moved).
    let mut asked_as: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let empty: Vec<serde_json::Value> = Vec::new();
    for kind in ["normalized", "redirects"] {
        for entry in value["query"][kind].as_array().unwrap_or(&empty) {
            if let (Some(from), Some(to)) = (entry["from"].as_str(), entry["to"].as_str()) {
                // Chase through an earlier rewrite so a title that was normalised AND
                // then redirected still reports the name we actually sent.
                let origin = asked_as.get(from).copied().unwrap_or(from);
                asked_as.insert(to, origin);
            }
        }
    }

    let mut out = Vec::with_capacity(pages.len());
    for page in pages {
        // A title we asked for that does not exist comes back flagged `missing`, with
        // no extract. That is an answer, not an error.
        if page["missing"].as_bool().unwrap_or(false) {
            continue;
        }
        let (Some(title), Some(text)) = (page["title"].as_str(), page["extract"].as_str()) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        out.push(Extract {
            article: title.to_string(),
            requested: asked_as.get(title).map(|s| s.to_string()),
            text: text.to_string(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sparql_result_becomes_a_mapping_with_the_article_title() {
        // The real shape, trimmed: the article arrives as a URL with underscores and
        // percent-encoding, and the extracts API wants a plain title.
        let body = r#"{"results":{"bindings":[
            {"imdb":{"value":"tt0047478"},"article":{"value":"https://en.wikipedia.org/wiki/Seven_Samurai"}},
            {"imdb":{"value":"tt0087544"},"article":{"value":"https://en.wikipedia.org/wiki/Nausica%C3%A4_of_the_Valley_of_the_Wind"}}
        ]}}"#;
        let mappings = parse_mappings(body).expect("parse");
        assert_eq!(
            mappings,
            vec![
                Mapping {
                    imdb_id: "tt0047478".into(),
                    article: "Seven Samurai".into()
                },
                Mapping {
                    imdb_id: "tt0087544".into(),
                    article: "Nausicaä of the Valley of the Wind".into()
                },
            ]
        );
    }

    #[test]
    fn a_row_missing_half_of_itself_is_skipped_not_fatal() {
        let body = r#"{"results":{"bindings":[
            {"imdb":{"value":"tt1"}},
            {"imdb":{"value":"tt2"},"article":{"value":"https://en.wikipedia.org/wiki/Ikiru"}}
        ]}}"#;
        let mappings = parse_mappings(body).expect("parse");
        assert_eq!(mappings.len(), 1, "one bad row must not cost the chunk");
        assert_eq!(mappings[0].article, "Ikiru");
    }

    #[test]
    fn a_body_that_is_not_the_documented_shape_is_an_error_not_an_empty_result() {
        // Silently returning nothing here would look exactly like "this prefix has no
        // articles", and would quietly skip whole chunks of the catalogue.
        assert!(matches!(
            parse_mappings("{\"results\":{}}"),
            Err(WikiError::Shape { .. })
        ));
        assert!(matches!(
            parse_mappings("not json"),
            Err(WikiError::Shape { .. })
        ));
    }

    #[test]
    fn extracts_are_parsed_and_missing_articles_are_simply_absent() {
        let body = r#"{"query":{"pages":[
            {"title":"Seven Samurai","extract":"Seven Samurai is a 1954 Japanese epic samurai film."},
            {"title":"Nonexistent Film","missing":true},
            {"title":"Blank","extract":"   "}
        ]}}"#;
        let extracts = parse_extracts(body).expect("parse");
        assert_eq!(extracts.len(), 1, "missing and blank are both absent");
        assert_eq!(extracts[0].article, "Seven Samurai");
        assert_eq!(
            extracts[0].answers(),
            "Seven Samurai",
            "no rewriting, no change"
        );
        assert!(extracts[0].text.starts_with("Seven Samurai is a 1954"));
    }

    #[test]
    fn a_moved_article_still_answers_the_question_that_was_asked() {
        // Wikidata's sitelink and Wikipedia's current title disagree whenever an article
        // has been moved. If the answer could not be traced back to the question, every
        // renamed film would be recorded as having no article at all.
        let body = r#"{"query":{
            "redirects":[{"from":"Solaris (1972 film)","to":"Solaris (1972 Soviet film)"}],
            "pages":[{"title":"Solaris (1972 Soviet film)","extract":"Solaris is a 1972 Soviet film."}]
        }}"#;
        let extracts = parse_extracts(body).expect("parse");
        assert_eq!(extracts[0].article, "Solaris (1972 Soviet film)");
        assert_eq!(extracts[0].answers(), "Solaris (1972 film)");
    }

    #[test]
    fn a_title_normalised_and_then_redirected_reports_the_name_we_sent() {
        // Both rewrites, chained, which is the case a single lookup table gets wrong.
        let body = r#"{"query":{
            "normalized":[{"from":"seven_samurai","to":"Seven samurai"}],
            "redirects":[{"from":"Seven samurai","to":"Seven Samurai"}],
            "pages":[{"title":"Seven Samurai","extract":"A 1954 film."}]
        }}"#;
        let extracts = parse_extracts(body).expect("parse");
        assert_eq!(extracts[0].answers(), "seven_samurai");
    }

    #[test]
    fn sparql_syntax_survives_being_put_in_a_url() {
        // `#` would truncate the query at the fragment and `{}` are not legal in a URL;
        // both appear in every SPARQL query this client sends.
        let encoded = encode("SELECT ?x WHERE { ?x wdt:P345 \"tt1\" } #comment");
        assert!(!encoded.contains('#'), "{encoded}");
        assert!(!encoded.contains('{'), "{encoded}");
        assert!(encoded.contains("%23"), "the hash is encoded, not dropped");
    }
}
