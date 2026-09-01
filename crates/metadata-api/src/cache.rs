//! TTL policy for the persistent response cache.
//!
//! The storage lives in `crates/persistence` (migration 0008, the `http_cache`
//! table); the *policy* lives here, because deciding how long a thing stays fresh
//! is a metadata concern rather than a database one — and because a policy with no
//! database is testable without one.
//!
//! # Freshness and usability are different
//!
//! `SPEC.md` Phase 4 requires "graceful offline behaviour", and that is only
//! possible if an expired entry is still *usable*. Expiry means "refresh this when
//! you can", not "delete this". When the network is unreachable, a month-old cast
//! list is better than an empty screen — and far better than an error.
//!
//! So there are three states, not two: **fresh**, **stale but serveable**, and
//! **too old to be worth keeping**.

use std::time::Duration;

/// What kind of resource a response holds. TTLs differ by orders of magnitude
/// between them, so one global TTL would be wrong for all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resource {
    /// A film's or series' details: cast, crew, synopsis. Essentially immutable
    /// once released — a 1954 film's director is not going to change.
    Detail,
    /// Search results. Cheap to refetch and quick to go stale as the catalogue moves.
    Search,
    /// Artwork URLs. Stable, and the images themselves are separately disk-cached.
    Images,
    /// Airing schedules. The whole point is that they are current; a stale one is
    /// actively misleading, which is different from merely unhelpful.
    Schedule,
    /// Ratings and vote counts. Constantly moving, but nothing breaks if they lag.
    Ratings,
}

impl Resource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detail => "detail",
            Self::Search => "search",
            Self::Images => "images",
            Self::Schedule => "schedule",
            Self::Ratings => "ratings",
        }
    }

    /// How long a response stays fresh.
    pub fn ttl(self) -> Duration {
        const DAY: u64 = 60 * 60 * 24;
        match self {
            // A released film's details do not change. Thirty days is about
            // corrections and late additions, not about the facts moving.
            Self::Detail => Duration::from_secs(30 * DAY),
            Self::Images => Duration::from_secs(14 * DAY),
            Self::Ratings => Duration::from_secs(7 * DAY),
            Self::Search => Duration::from_secs(DAY),
            // Short on purpose. A schedule that is wrong is worse than one that is
            // missing, because the user acts on it.
            Self::Schedule => Duration::from_secs(6 * 60 * 60),
        }
    }

    /// Beyond this, an entry is not worth keeping even for offline use.
    ///
    /// Generous, because the alternative to a very old cached answer is usually no
    /// answer at all. The exception is schedules, where a months-old answer is
    /// actively wrong rather than merely dated.
    pub fn max_age(self) -> Duration {
        match self {
            Self::Schedule => Duration::from_secs(60 * 60 * 24 * 7),
            _ => Duration::from_secs(60 * 60 * 24 * 365),
        }
    }
}

/// What the cache can do with an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Within its TTL. Use it and do not call the network.
    Fresh,
    /// Past its TTL but still useful. Serve it if the network is unavailable, and
    /// refresh it in the background otherwise.
    Stale,
    /// Old enough to be discarded.
    Expired,
}

/// Classify an entry by how long ago it was fetched.
pub fn freshness(resource: Resource, age: Duration) -> Freshness {
    if age < resource.ttl() {
        Freshness::Fresh
    } else if age < resource.max_age() {
        Freshness::Stale
    } else {
        Freshness::Expired
    }
}

/// May this entry be served right now?
///
/// The `offline` parameter is the whole point: the same stale entry is a *refresh
/// candidate* when the network is up and *the answer* when it is not.
pub fn serveable(resource: Resource, age: Duration, offline: bool) -> bool {
    match freshness(resource, age) {
        Freshness::Fresh => true,
        Freshness::Stale => offline,
        Freshness::Expired => false,
    }
}

/// Strip anything secret from a URL before it is used as a cache key.
///
/// **A key must never reach the cache table.** ADR-0027 makes TMDB keys per-user and
/// per-profile, and §2.4 means the database is copied around with the app folder —
/// so a key stored here would travel to wherever the folder went. This is the one
/// place that could happen, so it is handled here rather than trusted to callers.
pub fn cache_key(url: &str) -> String {
    const SECRETS: &[&str] = &[
        "api_key",
        "apikey",
        "api-key",
        "token",
        "access_token",
        "key",
    ];

    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };

    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let name = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
            !SECRETS.contains(&name.as_str())
        })
        .collect();

    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 60 * 60 * 24;

    #[test]
    fn a_released_films_details_stay_fresh_for_a_month() {
        assert_eq!(
            freshness(Resource::Detail, Duration::from_secs(29 * DAY)),
            Freshness::Fresh
        );
        assert_eq!(
            freshness(Resource::Detail, Duration::from_secs(31 * DAY)),
            Freshness::Stale
        );
    }

    #[test]
    fn a_schedule_goes_stale_in_hours_because_a_wrong_one_misleads() {
        assert_eq!(
            freshness(Resource::Schedule, Duration::from_secs(60 * 60 * 5)),
            Freshness::Fresh
        );
        assert_eq!(
            freshness(Resource::Schedule, Duration::from_secs(60 * 60 * 7)),
            Freshness::Stale
        );
        assert_eq!(
            freshness(Resource::Schedule, Duration::from_secs(30 * DAY)),
            Freshness::Expired,
            "a month-old airing schedule is not worth keeping"
        );
    }

    #[test]
    fn a_stale_entry_is_the_answer_when_offline_and_a_hint_when_online() {
        // The requirement is "graceful offline behaviour", and it only works if
        // expiry and deletion are different things.
        let month = Duration::from_secs(31 * DAY);
        assert!(
            !serveable(Resource::Detail, month, false),
            "refresh it when online"
        );
        assert!(
            serveable(Resource::Detail, month, true),
            "serve it when offline"
        );
    }

    #[test]
    fn an_expired_entry_is_not_served_even_offline() {
        let ancient = Duration::from_secs(400 * DAY);
        assert!(!serveable(Resource::Detail, ancient, true));
    }

    #[test]
    fn a_fresh_entry_is_served_either_way() {
        let hour = Duration::from_secs(3600);
        assert!(serveable(Resource::Detail, hour, false));
        assert!(serveable(Resource::Detail, hour, true));
    }

    #[test]
    fn an_api_key_never_becomes_part_of_a_cache_key() {
        // The database is copied with the app folder (§2.4). A key stored here would
        // travel with it.
        assert_eq!(
            cache_key("https://example.test/movie/1?api_key=SECRET&language=en"),
            "https://example.test/movie/1?language=en"
        );
        assert_eq!(
            cache_key("https://example.test/movie/1?API_KEY=SECRET"),
            "https://example.test/movie/1",
            "the match is case-insensitive"
        );
        assert_eq!(
            cache_key("https://example.test/movie/1?access_token=SECRET&page=2"),
            "https://example.test/movie/1?page=2"
        );
    }

    #[test]
    fn a_url_with_no_query_is_unchanged() {
        assert_eq!(
            cache_key("https://example.test/movie/1"),
            "https://example.test/movie/1"
        );
    }

    #[test]
    fn ordinary_parameters_survive_because_they_identify_the_resource() {
        // Dropping these would make two different requests share a cache entry.
        assert_eq!(
            cache_key("https://example.test/search?query=ran&year=1985"),
            "https://example.test/search?query=ran&year=1985"
        );
    }

    #[test]
    fn every_resource_has_a_distinct_stored_name() {
        let names: Vec<&str> = [
            Resource::Detail,
            Resource::Search,
            Resource::Images,
            Resource::Schedule,
            Resource::Ratings,
        ]
        .iter()
        .map(|r| r.as_str())
        .collect();
        let unique: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "a collision would merge two policies"
        );
    }

    #[test]
    fn max_age_is_never_shorter_than_the_ttl() {
        // Otherwise an entry would go from fresh straight to expired, and the stale
        // window that makes offline work would not exist.
        for resource in [
            Resource::Detail,
            Resource::Search,
            Resource::Images,
            Resource::Schedule,
            Resource::Ratings,
        ] {
            assert!(
                resource.max_age() > resource.ttl(),
                "{:?} has no stale window",
                resource
            );
        }
    }
}
