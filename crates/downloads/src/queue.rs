//! Which downloads should be running now: a pure function of the queue, the clock and the
//! number allowed at once.
//!
//! - Highest priority first, then the oldest.
//! - Never one scheduled for later ("after midnight") before its time.
//! - Never one the user paused: only they resume it.
//! - A running download is held (back to the queue) only when something more important
//!   needs its place; one that is merely scheduled later keeps running once started.

use sinephile_persistence::repositories::{Download, DownloadState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Start (or resume) it.
    Start(i64),
    /// Running, but something more important needs the place: back to the queue.
    Hold(i64),
}

/// `now` and `not_before` are ISO 8601 UTC ("2026-11-08T00:00:00Z"), which order as text.
pub fn plan(open: &[Download], now: &str, max_active: usize) -> Vec<Step> {
    let mut eligible: Vec<&Download> = open
        .iter()
        .filter(|d| match d.state {
            DownloadState::Active => true,
            DownloadState::Queued => d.not_before.as_deref().is_none_or(|t| t <= now),
            _ => false,
        })
        .collect();
    eligible.sort_by_key(|d| (std::cmp::Reverse(d.priority), d.id));
    let wanted: Vec<i64> = eligible.iter().take(max_active).map(|d| d.id).collect();

    let mut steps = Vec::new();
    for d in open {
        let want = wanted.contains(&d.id);
        match d.state {
            DownloadState::Queued if want => steps.push(Step::Start(d.id)),
            DownloadState::Active if !want => steps.push(Step::Hold(d.id)),
            _ => {}
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(id: i64, state: DownloadState, priority: i64, not_before: Option<&str>) -> Download {
        Download {
            id,
            media_item_id: None,
            title: format!("film {id}"),
            source: "infohash:x".into(),
            torrent: None,
            backend: "librqbit".into(),
            backend_id: None,
            priority,
            state,
            total_bytes: None,
            done_bytes: 0,
            limit_kib: None,
            not_before: not_before.map(str::to_string),
            reason: None,
            error: None,
            file_path: None,
            added_at: String::new(),
            finished_at: None,
        }
    }

    const NOW: &str = "2026-11-07T21:00:00Z";
    use DownloadState::*;

    #[test]
    fn the_most_important_start_first_then_the_oldest() {
        let q = [
            d(1, Queued, 0, None),
            d(2, Queued, 5, None),
            d(3, Queued, 0, None),
        ];
        assert_eq!(plan(&q, NOW, 2), vec![Step::Start(1), Step::Start(2)]);
    }

    #[test]
    fn a_scheduled_download_waits_for_its_time() {
        let q = [
            d(1, Queued, 9, Some("2026-11-08T00:00:00Z")),
            d(2, Queued, 0, None),
        ];
        assert_eq!(plan(&q, NOW, 1), vec![Step::Start(2)], "before midnight");
        assert_eq!(
            plan(&q, "2026-11-08T00:00:01Z", 1),
            vec![Step::Start(1)],
            "after midnight it goes first"
        );
    }

    #[test]
    fn something_more_important_takes_a_running_downloads_place() {
        let q = [d(1, Active, 0, None), d(2, Queued, 5, None)];
        assert_eq!(plan(&q, NOW, 1), vec![Step::Hold(1), Step::Start(2)]);
        // With room for both, nothing is held.
        assert_eq!(plan(&q, NOW, 2), vec![Step::Start(2)]);
    }

    #[test]
    fn a_paused_download_is_never_started_by_the_queue() {
        let q = [d(1, Paused, 9, None), d(2, Queued, 0, None)];
        assert_eq!(plan(&q, NOW, 3), vec![Step::Start(2)]);
    }

    #[test]
    fn a_running_download_scheduled_later_keeps_running() {
        let q = [d(1, Active, 0, Some("2026-11-09T00:00:00Z"))];
        assert!(plan(&q, NOW, 1).is_empty());
    }

    #[test]
    fn an_already_full_queue_is_left_alone() {
        let q = [
            d(1, Active, 0, None),
            d(2, Active, 0, None),
            d(3, Queued, 0, None),
        ];
        assert!(plan(&q, NOW, 2).is_empty());
    }
}
