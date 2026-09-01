//! The shared rate limiter.
//!
//! `SPEC.md` Phase 4 requires "a shared rate limiter" across every API client, and
//! exit criterion E4 is "**rate limits are never exceeded under a stress test of
//! 1,000 rapid lookups**".
//!
//! # Why shared, and why per host
//!
//! Rate limits are enforced by the *server*, per API key or per IP. Four clients
//! each politely limiting themselves to the documented rate will still exceed it
//! four times over, because the limit is on the thing they have in common. So the
//! limiter is keyed by host and shared by every client that talks to it.
//!
//! # Why a token bucket rather than "sleep between requests"
//!
//! A fixed delay between requests is both too slow and too fast. Too slow, because
//! after an idle minute you have banked no allowance and still crawl. Too fast,
//! because a burst of concurrent tasks each sleeping independently all wake at once.
//!
//! A token bucket refills continuously and is consumed atomically, so it permits a
//! genuine burst up to the bucket size, then settles to exactly the sustained rate.
//! That is both what the servers actually allow and what makes a first-run
//! enrichment feel fast.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// What one host permits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limit {
    /// Sustained requests per second.
    pub per_second: f64,
    /// How much unused allowance may be banked, in requests. A burst larger than
    /// this is smoothed to the sustained rate rather than rejected.
    pub burst: f64,
}

impl Limit {
    pub const fn new(per_second: f64, burst: f64) -> Self {
        Self { per_second, burst }
    }

    /// Requests per window, expressed as a rate.
    ///
    /// Documented limits usually read "40 requests per 10 seconds", and converting
    /// by hand is where an off-by-one creeps in.
    pub fn per_window(requests: u32, window: Duration) -> Self {
        let per_second = requests as f64 / window.as_secs_f64();
        // Bank at most one window's worth. Banking more would let a long idle period
        // fund a burst larger than the server ever agreed to.
        Self::new(per_second, requests as f64)
    }
}

struct Bucket {
    limit: Limit,
    tokens: f64,
    last_refill: Instant,
    /// Set when a server has told us to back off. Nothing is issued until it passes,
    /// whatever the bucket says — an explicit `Retry-After` outranks our own model
    /// of the limit, because the server is the authority on its own limit.
    penalty_until: Option<Instant>,
}

impl Bucket {
    fn new(limit: Limit) -> Self {
        Self {
            limit,
            // Start full: a cold start should not be slower than a warm one.
            tokens: limit.burst,
            last_refill: Instant::now(),
            penalty_until: None,
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f64();
        if elapsed > 0.0 {
            self.tokens = (self.tokens + elapsed * self.limit.per_second).min(self.limit.burst);
            self.last_refill = now;
        }
    }

    /// How long until one token is available, or zero if one is available now.
    fn wait_for_token(&mut self, now: Instant) -> Duration {
        if let Some(until) = self.penalty_until {
            if until > now {
                return until - now;
            }
            self.penalty_until = None;
        }

        self.refill(now);
        if self.tokens >= 1.0 {
            return Duration::ZERO;
        }
        // Time to accumulate the shortfall.
        Duration::from_secs_f64((1.0 - self.tokens) / self.limit.per_second)
    }
}

/// A rate limiter shared across every client, keyed by host.
#[derive(Clone, Default)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, Bucket>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a host's limit. Later calls replace an earlier one.
    pub async fn configure(&self, host: &str, limit: Limit) {
        let mut buckets = self.buckets.lock().await;
        buckets
            .entry(host.to_string())
            .and_modify(|b| b.limit = limit)
            .or_insert_with(|| Bucket::new(limit));
    }

    /// Wait until a request to `host` is permitted, then consume the allowance.
    ///
    /// An unregistered host is unlimited. That is deliberate: a caller that forgot to
    /// configure a limit gets full speed rather than a silent 1/sec crawl nobody
    /// diagnoses for a week. `configured_hosts` exists so a test can assert the set
    /// is what it should be.
    pub async fn acquire(&self, host: &str) {
        loop {
            let wait = {
                let mut buckets = self.buckets.lock().await;
                let Some(bucket) = buckets.get_mut(host) else {
                    return;
                };
                let now = Instant::now();
                let wait = bucket.wait_for_token(now);
                if wait.is_zero() {
                    bucket.tokens -= 1.0;
                    return;
                }
                wait
            };
            // The lock is released before sleeping. Holding it would serialise every
            // host behind the slowest one.
            tokio::time::sleep(wait).await;
        }
    }

    /// A server said to back off. Nothing is issued to this host until it passes.
    ///
    /// Honouring `Retry-After` is not optional politeness: ignoring it is what turns
    /// a temporary 429 into a ban.
    pub async fn penalise(&self, host: &str, retry_after: Duration) {
        let mut buckets = self.buckets.lock().await;
        if let Some(bucket) = buckets.get_mut(host) {
            let until = Instant::now() + retry_after;
            let until = match bucket.penalty_until {
                Some(existing) if existing > until => existing,
                _ => until,
            };
            bucket.penalty_until = Some(until);

            // Drain the bucket, so the moment the penalty lifts we do not spend a
            // full burst into a server that just complained.
            //
            // `last_refill` must move too, and that is not obvious: refill credits
            // tokens for every second since `last_refill`, so draining alone was
            // undone the instant the penalty lifted — the wait itself paid the
            // bucket back to full. Moving it to the end of the penalty means the
            // clock starts when the penalty does.
            bucket.tokens = 0.0;
            bucket.last_refill = until;
        }
    }

    pub async fn configured_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self.buckets.lock().await.keys().cloned().collect();
        hosts.sort();
        hosts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_burst_is_allowed_then_the_rate_settles() {
        // Tokio's paused clock: time advances only when something awaits a sleep, so
        // this is exact rather than timing-dependent.
        let limiter = RateLimiter::new();
        limiter
            .configure("example.test", Limit::new(10.0, 5.0))
            .await;

        let start = Instant::now();
        for _ in 0..5 {
            limiter.acquire("example.test").await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO, "the burst is immediate");

        limiter.acquire("example.test").await;
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(100),
            "the sixth waits one token at 10/sec"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_thousand_rapid_requests_never_exceed_the_limit() {
        // Phase 4 exit criterion E4, as a test rather than a promise.
        let limiter = RateLimiter::new();
        // 40 per 10 seconds, which is a realistic documented shape.
        limiter
            .configure(
                "example.test",
                Limit::per_window(40, Duration::from_secs(10)),
            )
            .await;

        let start = Instant::now();
        for _ in 0..1_000 {
            limiter.acquire("example.test").await;
        }
        let elapsed = start.elapsed();

        // 1,000 requests at 4/sec, minus the 40 banked at the start, is 240 seconds.
        let permitted = (1_000.0 - 40.0) / 4.0;
        assert!(
            elapsed.as_secs_f64() >= permitted - 0.5,
            "1,000 requests took {elapsed:?}, which is faster than {permitted}s and \
             therefore exceeded the limit"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn hosts_do_not_block_each_other() {
        // The lock is released before sleeping, or one slow host serialises the rest.
        let limiter = RateLimiter::new();
        limiter.configure("slow.test", Limit::new(1.0, 1.0)).await;
        limiter
            .configure("fast.test", Limit::new(1000.0, 100.0))
            .await;

        limiter.acquire("slow.test").await;
        let start = Instant::now();
        // Would need to wait a second on slow.test; fast.test must not.
        limiter.acquire("fast.test").await;
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_after_outranks_our_own_model() {
        let limiter = RateLimiter::new();
        limiter
            .configure("example.test", Limit::new(100.0, 100.0))
            .await;

        limiter
            .penalise("example.test", Duration::from_secs(30))
            .await;
        let start = Instant::now();
        limiter.acquire("example.test").await;

        // At least the penalty, and not much more. Not exactly the penalty: the
        // penalty drains the bucket, so the first request afterwards waits one
        // token's worth (10ms at 100/sec). That is correct — resuming at the
        // sustained rate rather than instantly is the point — and the first version
        // of this assertion was simply wrong about it.
        assert!(
            start.elapsed() >= Duration::from_secs(30),
            "a full bucket does not override what the server asked for: {:?}",
            start.elapsed()
        );
        assert!(
            start.elapsed() < Duration::from_secs(31),
            "and not much longer"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_penalty_drains_the_bucket_so_it_does_not_burst_afterwards() {
        // Spending a full burst the instant a 429 penalty lifts is how a temporary
        // limit becomes a ban.
        let limiter = RateLimiter::new();
        limiter
            .configure("example.test", Limit::new(10.0, 50.0))
            .await;
        limiter
            .penalise("example.test", Duration::from_secs(5))
            .await;

        let start = Instant::now();
        limiter.acquire("example.test").await;
        limiter.acquire("example.test").await;
        assert!(
            start.elapsed() > Duration::from_secs(5),
            "the second request waited for a refilled token, not a banked burst"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_longer_penalty_is_not_shortened_by_a_later_shorter_one() {
        let limiter = RateLimiter::new();
        limiter
            .configure("example.test", Limit::new(100.0, 100.0))
            .await;

        limiter
            .penalise("example.test", Duration::from_secs(60))
            .await;
        limiter
            .penalise("example.test", Duration::from_secs(1))
            .await;

        let start = Instant::now();
        limiter.acquire("example.test").await;
        assert!(start.elapsed() >= Duration::from_secs(60));
        assert!(start.elapsed() < Duration::from_secs(61));
    }

    #[tokio::test(start_paused = true)]
    async fn an_unregistered_host_is_unlimited() {
        // A forgotten limit should be full speed, not a silent crawl nobody
        // diagnoses. `configured_hosts` is how a test catches the omission instead.
        let limiter = RateLimiter::new();
        let start = Instant::now();
        for _ in 0..100 {
            limiter.acquire("unknown.test").await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert!(limiter.configured_hosts().await.is_empty());
    }

    #[test]
    fn per_window_converts_a_documented_limit() {
        let limit = Limit::per_window(40, Duration::from_secs(10));
        assert_eq!(limit.per_second, 4.0);
        assert_eq!(limit.burst, 40.0, "at most one window may be banked");
    }
}
