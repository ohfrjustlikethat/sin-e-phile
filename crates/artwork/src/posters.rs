//! Posters, fetched once and kept (Phase 16, the author's brief item 10; ADR-0048).
//!
//! Two places a poster can come from, both by the title's IMDb id and both chosen by the
//! person: TMDB's image server once the profile's own key is added (ADR-0027), and, opted
//! into, the poster images Stremio's own catalogue uses, which need no key. The caller
//! decides which addresses to try; this module only fetches what it is given, and never
//! anything that is not an image.
//!
//! Each poster is fetched once, re-encoded to WebP (`ArtworkCache::put`) and read from the
//! disk after that. A title with no poster anywhere is remembered for the session, so a
//! rail does not ask again every time it is drawn. At most four fetches run at once.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::cache::ArtworkCache;

/// Stremio's poster images, by IMDb id (ADR-0048).
pub const KEYLESS_BASE: &str = "https://images.metahub.space/poster/medium";

/// The same service's wide stills, by IMDb id (round 9, ADR-0052).
pub const KEYLESS_BACKDROP_BASE: &str = "https://images.metahub.space/background/medium";

/// Which of a title's images: the card's poster, or the still across the top of its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    Poster,
    Backdrop,
}

/// A poster is a few hundred kilobytes; anything far larger is not one.
pub const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;

const AT_ONCE: usize = 4;

/// Whether `id` is an IMDb id (`tt` and digits), the only thing put in a poster address.
pub fn is_imdb_id(id: &str) -> bool {
    id.len() > 2 && id.starts_with("tt") && id[2..].chars().all(|c| c.is_ascii_digit())
}

/// The keyless address of a title's poster or backdrop, by IMDb id.
pub fn keyless_url(art: Art, imdb_id: &str) -> Option<String> {
    let base = match art {
        Art::Poster => KEYLESS_BASE,
        Art::Backdrop => KEYLESS_BACKDROP_BASE,
    };
    is_imdb_id(imdb_id).then(|| format!("{base}/{imdb_id}/img"))
}

/// The addresses to try for a title's image, best first: TMDB's (when the profile has a
/// key, already looked up), then the keyless one (when the person switched it on).
pub fn sources(art: Art, tmdb: Option<String>, keyless: bool, imdb_id: &str) -> Vec<String> {
    let mut out: Vec<String> = tmdb.into_iter().collect();
    if keyless {
        out.extend(keyless_url(art, imdb_id));
    }
    out
}

/// A poster as the page shows it: a `data:` URL, which its content policy allows where a
/// remote address is not (ADR-0048).
pub fn data_url(webp: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/webp;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(webp)
    )
}

pub struct Posters {
    cache: Arc<ArtworkCache>,
    client: reqwest::Client,
    gate: tokio::sync::Semaphore,
    misses: Mutex<HashSet<String>>,
}

impl Posters {
    pub fn new(cache: ArtworkCache) -> Self {
        Self {
            cache: Arc::new(cache),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(20))
                .build()
                .expect("an HTTP client with no special options always builds"),
            gate: tokio::sync::Semaphore::new(AT_ONCE),
            misses: Mutex::default(),
        }
    }

    /// The first of `urls` that gives a poster, as WebP bytes: from the disk if it was
    /// fetched before, else fetched now and kept. `None` when none of them has one.
    pub async fn first(&self, urls: &[String]) -> Option<Vec<u8>> {
        for url in urls {
            if let Some(path) = self.cache.get(url) {
                if let Ok(bytes) = std::fs::read(path) {
                    return Some(bytes);
                }
            }
        }
        let _turn = self.gate.acquire().await.ok()?;
        for url in urls {
            if self.misses.lock().expect("misses").contains(url) {
                continue;
            }
            match self.fetch(url).await {
                Some(bytes) => {
                    let (cache, at) = (self.cache.clone(), url.clone());
                    let stored = tokio::task::spawn_blocking(move || cache.put(&at, &bytes))
                        .await
                        .ok()
                        .and_then(Result::ok);
                    if let Some(stored) = stored {
                        if let Ok(webp) = std::fs::read(stored.path) {
                            return Some(webp);
                        }
                    }
                    self.misses.lock().expect("misses").insert(url.clone());
                }
                None => {
                    self.misses.lock().expect("misses").insert(url.clone());
                }
            }
        }
        None
    }

    /// An image's bytes, or `None`: not there, not an image, or too large.
    async fn fetch(&self, url: &str) -> Option<Vec<u8>> {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return None;
        }
        let mut response = self.client.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let is_image = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t.starts_with("image/"));
        if !is_image
            || response
                .content_length()
                .is_some_and(|n| n as usize > MAX_IMAGE_BYTES)
        {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_IMAGE_BYTES {
                return None;
            }
        }
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmdb_first_then_the_keyless_one_and_neither_unless_chosen() {
        let tmdb = Some("https://image.tmdb.org/t/p/w342/a.jpg".to_string());
        assert_eq!(
            sources(Art::Poster, tmdb.clone(), true, "tt0013442"),
            [
                "https://image.tmdb.org/t/p/w342/a.jpg",
                "https://images.metahub.space/poster/medium/tt0013442/img"
            ]
        );
        assert_eq!(sources(Art::Poster, tmdb, false, "tt0013442").len(), 1);
        assert!(
            sources(Art::Backdrop, None, false, "tt0013442").is_empty(),
            "nothing asked unless chosen"
        );
        assert_eq!(data_url(b"RIFF"), "data:image/webp;base64,UklGRg==");
    }

    #[test]
    fn only_an_imdb_id_goes_into_an_address() {
        assert_eq!(
            keyless_url(Art::Poster, "tt0013442").as_deref(),
            Some("https://images.metahub.space/poster/medium/tt0013442/img")
        );
        assert_eq!(
            keyless_url(Art::Backdrop, "tt0013442").as_deref(),
            Some("https://images.metahub.space/background/medium/tt0013442/img")
        );
        assert_eq!(keyless_url(Art::Poster, "tt"), None);
        assert_eq!(keyless_url(Art::Backdrop, "../../etc"), None);
        assert_eq!(keyless_url(Art::Poster, "tt12/../x"), None);
    }
}
