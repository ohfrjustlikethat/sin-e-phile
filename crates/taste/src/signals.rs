//! What a viewing means (SPEC.md Phase 15). The player reports seconds actually played,
//! where it stopped and how long the film is; this says whether that was a viewing at all,
//! and whether it was completed or abandoned (and so where).

/// Played for less than this, it was a look, not a viewing.
pub const SKIM_SECONDS: f64 = 60.0;
/// Past this share of the runtime it was finished: credits mean almost nobody reaches 100%.
pub const COMPLETED_SHARE: f64 = 0.9;
/// Or within this many seconds of the end, for a short film whose credits are a larger share.
pub const COMPLETED_TAIL: f64 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// Not a viewing: nothing is recorded.
    Skimmed,
    Completed,
    /// Stopped at this many seconds in.
    Abandoned {
        at: f64,
    },
}

/// The outcome of one viewing. `duration` unknown (a live stream, a file not yet probed):
/// never "completed", only played.
pub fn outcome(played: f64, stopped_at: f64, duration: Option<f64>) -> Outcome {
    if played < SKIM_SECONDS {
        return Outcome::Skimmed;
    }
    match duration.filter(|d| *d > 0.0) {
        Some(d) if stopped_at >= d * COMPLETED_SHARE || stopped_at >= d - COMPLETED_TAIL => {
            Outcome::Completed
        }
        _ => Outcome::Abandoned { at: stopped_at },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewing_is_skimmed_completed_or_abandoned_somewhere() {
        assert_eq!(outcome(30.0, 30.0, Some(6000.0)), Outcome::Skimmed);
        assert_eq!(
            outcome(5500.0, 5500.0, Some(6000.0)),
            Outcome::Completed,
            "past 90%"
        );
        // A 40-minute episode with 6 minutes of credits.
        assert_eq!(
            outcome(2100.0, 2100.0, Some(2400.0)),
            Outcome::Completed,
            "within 5 minutes of the end"
        );
        assert_eq!(
            outcome(300.0, 300.0, Some(6000.0)),
            Outcome::Abandoned { at: 300.0 },
            "5 minutes in"
        );
        assert_eq!(
            outcome(4800.0, 4800.0, Some(6000.0)),
            Outcome::Abandoned { at: 4800.0 },
            "80 minutes in"
        );
        // Reaching the end counts, whatever this session played: a viewing resumed for its
        // last half is a film finished across two sittings.
        assert_eq!(outcome(70.0, 5990.0, Some(6000.0)), Outcome::Completed);
        assert_eq!(
            outcome(400.0, 400.0, None),
            Outcome::Abandoned { at: 400.0 },
            "no runtime known"
        );
    }
}
