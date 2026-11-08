//! Spending a provider's quota well (SPEC.md §14: "Treat the quota as a scarce resource
//! with a budget and a cache").
//!
//! [`fetch`] tries, in order: a subtitle already downloaded for this file and language
//! (free); then a reservation from today's budget (`SubtitleRepository::reserve`, one
//! conditional UPDATE, so the budget is never exceeded, even by two at once); then a
//! search, the best offer, and one download. A reservation that buys nothing — no offer,
//! an error — is given back. What the provider says is left replaces our own count when
//! it is lower, since its quota is per account and may be shared.

use std::future::Future;
use std::path::Path;

use sinephile_persistence::repositories::{CachedSubtitle, SubtitleRepository};
use sinephile_persistence::{Db, DbError};

use crate::{Found, Origin};

/// A subtitle a provider has for this file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub file_id: String,
    pub language: String,
    /// Timed for these exact bytes, not just this film.
    pub hash_match: bool,
    pub downloads: i64,
    /// The release it was timed for, for choosing by eye (Phase 16).
    pub release: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub text: String,
    /// What the provider says is left today.
    pub remaining: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("the API key was refused")]
    BadKey,
    #[error("today's downloads are used up")]
    QuotaReached,
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Store(#[from] DbError),
    #[error("could not save the subtitle: {0}")]
    Io(#[from] std::io::Error),
}

/// A subtitle provider. OpenSubtitles is the one that ships.
pub trait Provider {
    fn id(&self) -> &str;
    /// Downloads allowed per UTC day.
    fn daily_budget(&self) -> i64;
    fn search(
        &self,
        media_hash: &str,
        languages: &[String],
    ) -> impl Future<Output = Result<Vec<Offer>, FetchError>>;
    fn download(&self, file_id: &str) -> impl Future<Output = Result<Download, FetchError>>;
}

/// The best offer: the most preferred language, then one timed for this exact file,
/// then the most downloaded.
pub fn best<'a>(offers: &'a [Offer], languages: &[String]) -> Option<&'a Offer> {
    offers
        .iter()
        .filter_map(|o| {
            languages
                .iter()
                .position(|l| l.eq_ignore_ascii_case(&o.language))
                .map(|p| (p, o))
        })
        .min_by_key(|(p, o)| (*p, !o.hash_match, -o.downloads))
        .map(|(_, o)| o)
}

/// The whole chain for one film: what is already here (`local`: embedded tracks, files
/// beside it or in its torrent) if any of it will do, and only otherwise a provider.
/// Phase 10 E3: an embedded track in a preferred language is used without a single
/// network request.
pub async fn pick<P: Provider>(
    db: &Db,
    provider: Option<&P>,
    data: &Path,
    media_hash: Option<&str>,
    local: Vec<Found>,
    languages: &[String],
    today: &str,
) -> Result<Option<Found>, FetchError> {
    if !crate::worth_fetching(&local, languages) {
        return Ok(crate::choose(&local, languages).cloned());
    }
    if let (Some(p), Some(hash)) = (provider, media_hash) {
        if let Some(f) = fetch(db, p, data, hash, languages, today).await? {
            return Ok(Some(f));
        }
    }
    // A forced track, or nothing, is still better than an error.
    Ok(crate::choose(&local, languages).cloned())
}

/// Find a subtitle for the file with `media_hash` in one of `languages` (preferred
/// first). `data` is the data folder; files go under `cache/subtitles/`. `today` is the
/// UTC date, `YYYY-MM-DD`. `Ok(None)`: nothing to be had today.
pub async fn fetch<P: Provider>(
    db: &Db,
    provider: &P,
    data: &Path,
    media_hash: &str,
    languages: &[String],
    today: &str,
) -> Result<Option<Found>, FetchError> {
    let repo = SubtitleRepository::new(db);
    for lang in languages {
        if let Some(c) = repo.cached(media_hash, lang).await? {
            if data.join(&c.relative_path).is_file() {
                return Ok(Some(found(c, lang)));
            }
        }
    }
    if !repo
        .reserve(provider.id(), today, provider.daily_budget())
        .await?
    {
        return Ok(None);
    }
    match download(&repo, provider, data, media_hash, languages, today).await {
        Ok(Some(f)) => Ok(Some(f)),
        Ok(None) => {
            repo.release(provider.id(), today).await?;
            Ok(None)
        }
        Err(FetchError::QuotaReached) => {
            // The provider counts differently from us: believe it, keep the reservation.
            repo.set_server_remaining(provider.id(), today, 0).await?;
            Ok(None)
        }
        Err(e) => {
            repo.release(provider.id(), today).await?;
            Err(e)
        }
    }
}

/// What a provider offers for this file, best first (Phase 16: choosing a subtitle in the
/// player). A search spends no download, so no budget is reserved for it.
pub async fn offers<P: Provider>(
    provider: &P,
    media_hash: &str,
    languages: &[String],
) -> Result<Vec<Offer>, FetchError> {
    let mut all = provider.search(media_hash, languages).await?;
    let rank = |o: &Offer| {
        let lang = languages
            .iter()
            .position(|l| l.eq_ignore_ascii_case(&o.language))
            .unwrap_or(usize::MAX);
        (lang, !o.hash_match, -o.downloads)
    };
    all.sort_by_key(rank);
    Ok(all)
}

/// Download the offer the viewer chose, within today's budget: reserved first, given back
/// if nothing comes of it. `Ok(None)`: the budget is spent for today.
pub async fn take<P: Provider>(
    db: &Db,
    provider: &P,
    data: &Path,
    media_hash: &str,
    offer: &Offer,
    today: &str,
) -> Result<Option<Found>, FetchError> {
    let repo = SubtitleRepository::new(db);
    if !repo
        .reserve(provider.id(), today, provider.daily_budget())
        .await?
    {
        return Ok(None);
    }
    match save(&repo, provider, data, media_hash, offer, today).await {
        Ok(f) => Ok(Some(f)),
        Err(FetchError::QuotaReached) => {
            repo.set_server_remaining(provider.id(), today, 0).await?;
            Ok(None)
        }
        Err(e) => {
            repo.release(provider.id(), today).await?;
            Err(e)
        }
    }
}

async fn download<P: Provider>(
    repo: &SubtitleRepository<'_>,
    provider: &P,
    data: &Path,
    media_hash: &str,
    languages: &[String],
    today: &str,
) -> Result<Option<Found>, FetchError> {
    let offers = provider.search(media_hash, languages).await?;
    let Some(offer) = best(&offers, languages) else {
        return Ok(None);
    };
    save(repo, provider, data, media_hash, offer, today)
        .await
        .map(Some)
}

/// One offer, downloaded, written under `cache/subtitles/` and remembered.
async fn save<P: Provider>(
    repo: &SubtitleRepository<'_>,
    provider: &P,
    data: &Path,
    media_hash: &str,
    offer: &Offer,
    today: &str,
) -> Result<Found, FetchError> {
    let got = provider.download(&offer.file_id).await?;
    if let Some(left) = got.remaining {
        repo.set_server_remaining(provider.id(), today, left)
            .await?;
    }
    let relative = format!(
        "cache/subtitles/{}.{}.{}.srt",
        media_hash,
        offer.language,
        offer
            .file_id
            .replace(|c: char| !c.is_ascii_alphanumeric(), "")
    );
    let path = data.join(&relative);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, got.text)?;
    let entry = CachedSubtitle {
        provider: provider.id().to_string(),
        file_id: offer.file_id.clone(),
        relative_path: relative,
    };
    repo.remember(media_hash, &offer.language, &entry).await?;
    Ok(found(entry, &offer.language))
}

fn found(c: CachedSubtitle, lang: &str) -> Found {
    Found {
        origin: Origin::Downloaded {
            provider: c.provider,
            file_id: c.file_id,
            relative_path: c.relative_path,
        },
        language: Some(lang.to_string()),
        forced: false,
    }
}
