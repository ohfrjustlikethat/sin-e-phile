//! Is the catalogue out of date, and is it worth finding out? (ADR-0030 layer 1, D37)
//!
//! # The problem this exists to solve
//!
//! `refresh` works and is measured at 71 seconds, and until now **nothing ever called
//! it**. It was a command the author typed. An installed application's catalogue was
//! therefore frozen at install time, which is precisely the outcome ADR-0030 was written
//! to prevent, and the author asked for the opposite: *"it should be updated each time
//! someone opens the app."*
//!
//! # Why that instruction is affordable, taken literally
//!
//! `title.basics.tsv.gz` is 216 MB. Downloading it on every launch is not shippable, and
//! a naive "refresh on open" means exactly that. But IMDb serves **validators**, measured
//! 2026-09-07:
//!
//! ```text
//! HEAD https://datasets.imdbws.com/title.basics.tsv.gz
//!   Last-Modified: Sun, 06 Sep 2026 00:48:16 GMT
//!   ETag:          "c5a942c76317cde815f77fcf113ed80a-27"
//! ```
//!
//! So the check is one HEAD request comparing an ETag. IMDb republishes daily, so on a
//! typical launch the answer is "nothing changed" for the price of a few hundred bytes,
//! and a real download happens at most once a day.
//!
//! **HEAD rather than a conditional GET.** A GET carrying `If-None-Match` returns 304
//! with an empty body, which is equally cheap — *when the validator matches*. When it
//! does not, it returns 216 MB, and the first launch after an install has no validator
//! to send at all. HEAD cannot surprise anyone.
//!
//! # This never blocks and never throws
//!
//! A launch-time network call that can fail must not be able to stop the application
//! from opening, so every failure resolves to [`Freshness::Unknown`] and the app carries
//! on with the catalogue it has. Being offline is not an error here — it is Tuesday.

use sinephile_metadata_api::store::CacheStore;
use sinephile_metadata_api::{Request, Resource, Store, Transport};
use sinephile_persistence::Db;

use crate::imdb::TITLE_BASICS;

/// What a check concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    /// The publisher's copy is the one already ingested. Nothing to do.
    UpToDate,
    /// Something changed, or nothing has ever been ingested. A refresh is worth running.
    Stale { validator: Option<String> },
    /// The question could not be answered — offline, a refused request, a server with no
    /// validators. **Never a reason to refresh and never a reason to fail**: an app that
    /// would not open because a dataset host was down would be worse than a stale one.
    Unknown,
}

impl Freshness {
    pub fn should_refresh(&self) -> bool {
        matches!(self, Freshness::Stale { .. })
    }
}

/// What the cache stores this check's validator under.
///
/// The URL as the key, like every other entry, so the existing `http_cache` machinery
/// applies unchanged — including its rule that no API key ever reaches this table.
fn cache_key() -> String {
    TITLE_BASICS.url()
}

/// Ask the publisher whether its copy differs from the one already ingested.
///
/// Cheap by construction: one HEAD, a string comparison, no body.
pub async fn check(db: &Db, transport: &dyn Transport) -> Freshness {
    let store = CacheStore::new(db);
    let url = cache_key();

    // `include_stale`, because this entry is deliberately kept for a year and its age
    // is irrelevant: it is a validator to compare against, not a body to serve.
    let known = match store.get(&url, Resource::Detail, false, true).await {
        Ok(Some(cached)) => cached.etag.clone().or(cached.last_modified.clone()),
        // Never ingested, or the entry was purged. A refresh is due and no request is
        // needed to know it — which is also what makes the very first launch free.
        Ok(None) => return Freshness::Stale { validator: None },
        Err(_) => return Freshness::Unknown,
    };
    let Some(known) = known else {
        return Freshness::Stale { validator: None };
    };

    let response = match transport.send(Request::head(&url)).await {
        Ok(response) => response,
        Err(_) => return Freshness::Unknown,
    };
    if !(200..300).contains(&response.status) {
        return Freshness::Unknown;
    }

    let current = response
        .header("etag")
        .or_else(|| response.header("last-modified"));
    let Some(current) = current else {
        // A publisher that stops serving validators cannot be diffed. Refreshing on
        // every launch because of it would be the 216 MB-per-launch behaviour this
        // module exists to avoid, so the honest answer is that we do not know.
        return Freshness::Unknown;
    };

    if current == known {
        Freshness::UpToDate
    } else {
        Freshness::Stale {
            validator: Some(current.to_string()),
        }
    }
}

/// Record the validator a completed refresh ingested, so the next check has something to
/// compare against.
///
/// Called **after** a refresh succeeds, never before: storing it up front would make a
/// failed or interrupted refresh look like a completed one, and the catalogue would then
/// never update again.
pub async fn remember(db: &Db, validator: &str) -> Result<(), crate::job::JobError> {
    let store = CacheStore::new(db);
    let url = cache_key();
    store
        .put(Store::ok(&url, "imdb", Resource::Detail, "").with_etag(Some(validator)))
        .await
        .map_err(|e| crate::job::JobError::step("freshness", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinephile_metadata_api::transport::{FakeTransport, Response};

    async fn db() -> Db {
        Db::in_memory().await.expect("open")
    }

    #[tokio::test]
    async fn a_catalogue_that_has_never_been_refreshed_is_stale_without_asking_anyone() {
        // The first launch after an install must not need the network to know there is
        // work to do — and must not spend a request discovering it.
        let db = db().await;
        let transport = FakeTransport::default();

        assert_eq!(
            check(&db, &transport).await,
            Freshness::Stale { validator: None }
        );
        assert!(
            transport.requests.lock().expect("lock").is_empty(),
            "no request should have been made"
        );
    }

    #[tokio::test]
    async fn an_unchanged_validator_means_nothing_to_do() {
        let db = db().await;
        remember(&db, "\"abc-27\"").await.expect("remember");

        let transport = FakeTransport::default();
        transport.push(Response::new(200, "").with_header("etag", "\"abc-27\""));

        assert_eq!(check(&db, &transport).await, Freshness::UpToDate);
    }

    #[tokio::test]
    async fn a_changed_validator_is_stale_and_carries_the_new_one() {
        let db = db().await;
        remember(&db, "\"abc-27\"").await.expect("remember");

        let transport = FakeTransport::default();
        transport.push(Response::new(200, "").with_header("etag", "\"def-28\""));

        assert_eq!(
            check(&db, &transport).await,
            Freshness::Stale {
                validator: Some("\"def-28\"".into())
            }
        );
    }

    #[tokio::test]
    async fn being_offline_is_not_an_error_and_is_not_a_refresh() {
        // The failure that matters: an app that will not open because a dataset host is
        // unreachable. Unknown means "carry on with what we have".
        let db = db().await;
        remember(&db, "\"abc-27\"").await.expect("remember");

        let transport = FakeTransport::default();
        transport.push_error(sinephile_metadata_api::TransportError::Network(
            "offline".into(),
        ));

        let freshness = check(&db, &transport).await;
        assert_eq!(freshness, Freshness::Unknown);
        assert!(!freshness.should_refresh());
    }

    #[tokio::test]
    async fn a_publisher_that_stops_serving_validators_is_unknown_not_stale() {
        // Treating "no validator" as stale would download 216 MB on every launch, which
        // is the exact behaviour this module exists to prevent.
        let db = db().await;
        remember(&db, "\"abc-27\"").await.expect("remember");

        let transport = FakeTransport::default();
        transport.push(Response::new(200, ""));

        assert_eq!(check(&db, &transport).await, Freshness::Unknown);
    }
}
