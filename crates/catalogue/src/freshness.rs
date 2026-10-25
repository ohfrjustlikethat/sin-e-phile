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

/// Check, and refresh if the answer is yes. **This is what the application calls on
/// launch**, and it is deliberately the whole policy in one place.
///
/// # Automatic and silent, by the author's decision
///
/// A refresh that is due runs in the background without asking. The alternative — a
/// prompt saying an update is available — puts a decision in front of someone who opened
/// the app to watch something, about a dataset they have never heard of. Progress is
/// visible through `CatalogueRepository::readiness` for any screen that wants it.
///
/// # Returns what happened, and never fails the caller
///
/// A launch-time task that can return an error invites a caller that handles it by
/// crashing. The outcome is a value; the caller logs it.
pub async fn refresh_if_stale(
    db: &Db,
    transport: &dyn Transport,
    datasets: &std::path::Path,
) -> Outcome {
    let freshness = check(db, transport).await;
    let Freshness::Stale { validator } = freshness else {
        return match freshness {
            Freshness::UpToDate => Outcome::AlreadyCurrent,
            _ => Outcome::CouldNotTell,
        };
    };

    // WHERE THE VALIDATOR IS RECORDED, AND WHY IT IS HERE.
    //
    // After the download, before the load. Measured, having got it wrong twice:
    //
    //   after the whole refresh — the app exits when its window closes, the refresh
    //     takes about a minute, and a user who opens and closes quickly never reaches
    //     the end. Nothing is ever remembered, so EVERY launch re-downloads 216 MB.
    //     Observed on real launches: two in a row, the second adding `titles_added: 0`.
    //   before the download — an interrupted refresh looks complete, and the catalogue
    //     silently never updates again.
    //
    // The download is the expensive, all-or-nothing half; the load is resumable through
    // `Job`. So the boundary between them is the only place that is wrong in neither
    // direction: killed mid-download, nothing is recorded and it is fetched again;
    // killed mid-load, the bytes are kept and the job resumes.
    let validator = match validator {
        Some(validator) => Some(validator),
        // Nothing was cached, so `check` returned stale without asking. Ask now.
        None => current_validator(transport, &cache_key()).await,
    };

    if let Err(error) = crate::refresh::download(datasets).await {
        tracing::warn!(%error, "catalogue download failed; carrying on with what we have");
        return Outcome::Failed;
    }
    if let Some(validator) = &validator {
        if let Err(error) = remember(db, validator).await {
            tracing::warn!(%error, "downloaded, but could not record the validator");
        }
    }

    match crate::refresh::load(db, datasets).await {
        Ok(refreshed) => Outcome::Refreshed {
            titles_added: refreshed.titles_added,
        },
        Err(error) => {
            // The bytes are on disk and the validator is recorded, so this does not
            // re-download; `Job` resumes the load on the next launch.
            tracing::warn!(%error, "catalogue load failed; it will resume next launch");
            Outcome::Failed
        }
    }
}

/// The publisher's current validator, or `None` if it will not say.
///
/// Separate from [`check`] because it answers a different question: not "has this
/// changed" but "what is it now", which is what a first refresh needs in order to have
/// something to compare against next time.
async fn current_validator(transport: &dyn Transport, url: &str) -> Option<String> {
    let response = transport.send(Request::head(url)).await.ok()?;
    if !(200..300).contains(&response.status) {
        return None;
    }
    response
        .header("etag")
        .or_else(|| response.header("last-modified"))
        .map(str::to_string)
}

/// What a launch-time refresh did, for the log and for any screen that cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    AlreadyCurrent,
    Refreshed {
        titles_added: i64,
    },
    /// Offline, or the publisher would not say. Not an error.
    CouldNotTell,
    /// It was due and it did not work. Also not an error, to the caller.
    Failed,
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
    async fn nothing_is_downloaded_when_the_catalogue_is_already_current() {
        // The whole point of the check: a launch where IMDb has not republished must
        // cost one HEAD request and nothing else. If this ever regresses, every launch
        // pays 216 MB.
        let db = db().await;
        remember(&db, "\"abc-27\"").await.expect("remember");

        let transport = FakeTransport::default();
        transport.push(Response::new(200, "").with_header("etag", "\"abc-27\""));

        let outcome = refresh_if_stale(&db, &transport, std::path::Path::new("/nonexistent")).await;
        assert_eq!(outcome, Outcome::AlreadyCurrent);
        assert_eq!(
            transport.requests.lock().expect("lock").len(),
            1,
            "exactly one HEAD, and no download"
        );
    }

    #[tokio::test]
    async fn the_first_refresh_learns_a_validator_so_the_second_launch_is_free() {
        // THE BUG THIS CAUGHT, measured on two real launches before it was written: the
        // first check is stale WITHOUT a request, so it carries no validator, so nothing
        // was remembered, so the second launch downloaded 216 MB to add nothing. The
        // per-launch download this module exists to prevent, arriving through the one
        // path that skips the HEAD.
        //
        // A refresh cannot run in a test, so this asserts the half that was wrong: with
        // no stored validator, the check is stale and carries none — and `remember` is
        // therefore the only thing standing between the first launch and the second.
        let db = db().await;
        let transport = FakeTransport::default();

        assert_eq!(
            check(&db, &transport).await,
            Freshness::Stale { validator: None },
            "a fresh catalogue is stale and has nothing to remember"
        );

        // What `refresh_if_stale` now does after a successful first refresh.
        let learned = current_validator(
            &{
                let t = FakeTransport::default();
                t.push(Response::new(200, "").with_header("etag", "\"abc-27\""));
                t
            },
            "https://example.invalid/title.basics.tsv.gz",
        )
        .await;
        assert_eq!(learned.as_deref(), Some("\"abc-27\""));
        remember(&db, &learned.expect("learned"))
            .await
            .expect("remember");

        // The second launch now costs one HEAD and finds nothing to do.
        let second = FakeTransport::default();
        second.push(Response::new(200, "").with_header("etag", "\"abc-27\""));
        assert_eq!(check(&db, &second).await, Freshness::UpToDate);
    }

    #[tokio::test]
    async fn being_unable_to_tell_never_triggers_a_refresh() {
        // `/nonexistent` would make a real refresh fail loudly. Reaching it at all would
        // mean an offline launch had decided to re-download the catalogue.
        let db = db().await;
        remember(&db, "\"abc-27\"").await.expect("remember");

        let transport = FakeTransport::default();
        transport.push_error(sinephile_metadata_api::TransportError::Timeout);

        let outcome = refresh_if_stale(&db, &transport, std::path::Path::new("/nonexistent")).await;
        assert_eq!(outcome, Outcome::CouldNotTell);
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
