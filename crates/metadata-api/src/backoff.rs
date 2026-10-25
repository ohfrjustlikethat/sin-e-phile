//! Exponential backoff with jitter (`SPEC.md` Phase 4).
//!
//! # Why jitter is not optional
//!
//! Without it, every client that failed at the same moment retries at the same
//! moment. A first-run enrichment that fires two hundred requests into a service
//! having a bad thirty seconds will retry all two hundred simultaneously, three
//! times, and turn a transient blip into a self-inflicted outage. Full jitter — a
//! uniform draw from `[0, backoff]` rather than a fixed offset — spreads the
//! retries across the whole window.
//!
//! # What is worth retrying
//!
//! Only failures a retry could plausibly fix. A 404 means the thing is not there and
//! will not be there in four seconds; retrying it wastes the rate-limit allowance
//! that a request which *would* have succeeded needed. A 401 means the key is wrong,
//! and hammering a bad key is how a key gets revoked.

use std::time::Duration;

/// How a failed request should be retried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backoff {
    pub initial: Duration,
    pub max: Duration,
    pub multiplier: f64,
    pub max_attempts: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(500),
            // Beyond about half a minute the user has given up anyway, and holding
            // the attempt open stops anything else from using the allowance.
            max: Duration::from_secs(30),
            multiplier: 2.0,
            max_attempts: 4,
        }
    }
}

impl Backoff {
    /// The delay before attempt `attempt` (1-based), before jitter.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return Duration::ZERO;
        }
        let exponent = (attempt - 2) as i32;
        let scaled = self.initial.as_secs_f64() * self.multiplier.powi(exponent);
        Duration::from_secs_f64(scaled.min(self.max.as_secs_f64()))
    }

    /// The delay with full jitter applied, given a uniform sample in `[0, 1)`.
    ///
    /// The sample is a parameter rather than drawn here so the whole policy stays
    /// deterministic and testable — a randomised backoff that cannot be tested is
    /// how the retry storm it exists to prevent goes unnoticed.
    pub fn jittered(&self, attempt: u32, sample: f64) -> Duration {
        let base = self.delay_for(attempt);
        Duration::from_secs_f64(base.as_secs_f64() * sample.clamp(0.0, 1.0))
    }

    pub fn should_retry(&self, attempt: u32, outcome: Retryable) -> bool {
        attempt < self.max_attempts && outcome == Retryable::Yes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retryable {
    Yes,
    No,
}

/// Is this status worth trying again?
pub fn classify(status: u16) -> Retryable {
    match status {
        // Rate limited. Retryable, and the limiter is separately penalised so the
        // retry does not land straight back into the same wall.
        429 => Retryable::Yes,
        // Server-side. The next one may hit a healthy instance.
        500..=599 => Retryable::Yes,
        // 408 request timeout, 425 too early.
        408 | 425 => Retryable::Yes,
        // 401/403 — a bad key does not improve by being used again, and hammering
        // one is how it gets revoked. 404 — it is not there.
        _ => Retryable::No,
    }
}

/// A network error with no status at all: DNS failure, connection refused, TLS
/// handshake. Worth one more try, because these are frequently transient.
pub const TRANSPORT_FAILURE: Retryable = Retryable::Yes;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_attempt_does_not_wait() {
        assert_eq!(Backoff::default().delay_for(1), Duration::ZERO);
    }

    #[test]
    fn delays_double_and_then_stop_growing() {
        let backoff = Backoff::default();
        assert_eq!(backoff.delay_for(2), Duration::from_millis(500));
        assert_eq!(backoff.delay_for(3), Duration::from_secs(1));
        assert_eq!(backoff.delay_for(4), Duration::from_secs(2));
        assert_eq!(
            backoff.delay_for(50),
            Duration::from_secs(30),
            "capped, not grown to hours"
        );
    }

    #[test]
    fn jitter_spreads_across_the_whole_window() {
        // Full jitter, not "base plus a bit": the point is that two clients failing
        // together do not retry together.
        let backoff = Backoff::default();
        assert_eq!(backoff.jittered(3, 0.0), Duration::ZERO);
        assert_eq!(backoff.jittered(3, 1.0), Duration::from_secs(1));
        assert_eq!(backoff.jittered(3, 0.5), Duration::from_millis(500));
    }

    #[test]
    fn a_sample_outside_the_unit_interval_is_clamped() {
        let backoff = Backoff::default();
        assert_eq!(backoff.jittered(3, 5.0), Duration::from_secs(1));
        assert_eq!(backoff.jittered(3, -1.0), Duration::ZERO);
    }

    #[test]
    fn only_failures_a_retry_could_fix_are_retried() {
        assert_eq!(classify(429), Retryable::Yes, "rate limited");
        assert_eq!(classify(500), Retryable::Yes);
        assert_eq!(classify(503), Retryable::Yes);
        assert_eq!(classify(408), Retryable::Yes);

        assert_eq!(
            classify(404),
            Retryable::No,
            "it is not there in four seconds either"
        );
        assert_eq!(
            classify(401),
            Retryable::No,
            "hammering a bad key gets it revoked"
        );
        assert_eq!(classify(403), Retryable::No);
        assert_eq!(classify(200), Retryable::No);
    }

    #[test]
    fn attempts_are_bounded() {
        let backoff = Backoff::default();
        assert!(backoff.should_retry(1, Retryable::Yes));
        assert!(backoff.should_retry(3, Retryable::Yes));
        assert!(
            !backoff.should_retry(4, Retryable::Yes),
            "max_attempts is a ceiling, not a suggestion"
        );
        assert!(!backoff.should_retry(1, Retryable::No));
    }

    #[test]
    fn a_transport_failure_is_worth_one_more_try() {
        // DNS, connection refused, TLS — frequently a laptop waking up.
        assert_eq!(TRANSPORT_FAILURE, Retryable::Yes);
    }
}
