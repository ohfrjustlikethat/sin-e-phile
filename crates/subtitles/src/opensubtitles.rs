//! OpenSubtitles (SPEC.md §14): subtitles matched to a file by its hash, with the user's
//! own API key. The free tier allows a handful of downloads a day, which is why this is
//! step 3 of the chain and why [`crate::fetch`] budgets every download.
//!
//! Two calls: `GET /subtitles?moviehash=…&languages=…` lists what exists (free), and
//! `POST /download` turns a file id into a short-lived link (one download from the
//! quota; the answer says how many are left).

use std::sync::Arc;

use serde::Deserialize;
use sinephile_metadata_api::{Request, Transport};

use crate::fetch::{Download, FetchError, Offer, Provider};

const BASE: &str = "https://api.opensubtitles.com/api/v1";
/// The service asks every app to name itself.
const USER_AGENT: &str = "sin-e-phile v0.1";
/// A search answer is a page of JSON; a subtitle file is text. Neither is large.
const MAX_SEARCH: usize = 1 << 20;
const MAX_FILE: usize = 2 << 20;
/// The free tier's daily downloads (SPEC.md §14: "single-digit downloads/day").
pub const FREE_DAILY: i64 = 5;

pub struct OpenSubtitles {
    transport: Arc<dyn Transport>,
    key: String,
    daily: i64,
}

impl OpenSubtitles {
    /// `key` is the profile's own (CredentialRepository::opensubtitles_key); none ships.
    pub fn new(transport: Arc<dyn Transport>, key: String) -> Self {
        Self {
            transport,
            key,
            daily: FREE_DAILY,
        }
    }

    fn authorised(&self, r: Request) -> Request {
        r.header("Api-Key", &self.key)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
    }

    async fn send(&self, r: Request) -> Result<String, FetchError> {
        let response = self
            .transport
            .send(self.authorised(r))
            .await
            .map_err(|e| FetchError::Unavailable(e.to_string()))?;
        match response.status {
            200 => Ok(response.body),
            401 | 403 => Err(FetchError::BadKey),
            // 406 is the service's "download quota reached".
            406 | 429 => Err(FetchError::QuotaReached),
            s => Err(FetchError::Unavailable(format!(
                "OpenSubtitles answered {s}"
            ))),
        }
    }
}

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default)]
    data: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    attributes: Attributes,
}

#[derive(Deserialize)]
struct Attributes {
    language: Option<String>,
    #[serde(default)]
    download_count: i64,
    #[serde(default)]
    moviehash_match: bool,
    #[serde(default)]
    files: Vec<FileRef>,
}

#[derive(Deserialize)]
struct FileRef {
    file_id: i64,
}

#[derive(Deserialize)]
struct DownloadAnswer {
    link: String,
    remaining: Option<i64>,
}

impl Provider for OpenSubtitles {
    fn id(&self) -> &str {
        "opensubtitles"
    }

    fn daily_budget(&self) -> i64 {
        self.daily
    }

    async fn search(
        &self,
        media_hash: &str,
        languages: &[String],
    ) -> Result<Vec<Offer>, FetchError> {
        // The service asks for parameters in alphabetical order, lower case, so its
        // cache can answer; languages sorted for the same reason.
        let mut langs: Vec<String> = languages.iter().map(|l| l.to_ascii_lowercase()).collect();
        langs.sort();
        langs.dedup();
        let url = format!(
            "{BASE}/subtitles?languages={}&moviehash={}",
            langs.join(","),
            media_hash.to_ascii_lowercase()
        );
        let body = self.send(Request::get(url).limit(MAX_SEARCH)).await?;
        let page: SearchPage = serde_json::from_str(&body)
            .map_err(|e| FetchError::Unavailable(format!("search: {e}")))?;
        Ok(page
            .data
            .into_iter()
            .filter_map(|item| {
                let a = item.attributes;
                Some(Offer {
                    file_id: a.files.first()?.file_id.to_string(),
                    language: a.language?.to_ascii_lowercase(),
                    hash_match: a.moviehash_match,
                    downloads: a.download_count,
                })
            })
            .collect())
    }

    async fn download(&self, file_id: &str) -> Result<Download, FetchError> {
        let id: i64 = file_id
            .parse()
            .map_err(|_| FetchError::Unavailable(format!("not a file id: {file_id}")))?;
        let body = self
            .send(
                Request::post_json(format!("{BASE}/download"), format!("{{\"file_id\":{id}}}"))
                    .limit(MAX_SEARCH),
            )
            .await?;
        let answer: DownloadAnswer = serde_json::from_str(&body)
            .map_err(|e| FetchError::Unavailable(format!("download: {e}")))?;
        // The link is to the file itself; it needs no key.
        let file = self
            .transport
            .send(Request::get(answer.link).limit(MAX_FILE))
            .await
            .map_err(|e| FetchError::Unavailable(e.to_string()))?;
        if file.status != 200 {
            return Err(FetchError::Unavailable(format!(
                "the file answered {}",
                file.status
            )));
        }
        Ok(Download {
            text: file.body,
            remaining: answer.remaining,
        })
    }
}
