//! Phase 6 E4: "Backend timeout: one slow backend never delays results from the
//! others."
//!
//! These run on tokio's PAUSED clock (`start_paused`). Sleeps complete instantly in
//! real time but advance virtual time exactly, so "the fast backend answered at
//! 10 ms, before the slow one's 300 ms timeout" is measured, not guessed, and the
//! test cannot be flaky on a loaded CI runner.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sinephile_sources::{Entry, Resolver};
use source_protocol::{
    async_trait, BackendCapabilities, MediaKind, MediaQuery, PlayableStream, Provenance,
    SourceBackend, SourceCandidate, SourceError, Transport,
};
use tokio::time::Instant;

/// A backend that answers after `delay` with `n` candidates sharing `locator`s.
struct Fake {
    id: &'static str,
    delay: Duration,
    locators: Vec<&'static str>,
    behaviour: Behaviour,
    network: bool,
    finished: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
enum Behaviour {
    Answer,
    Fail,
    Panic,
}

impl Fake {
    fn new(id: &'static str, delay_ms: u64, locators: &[&'static str]) -> Self {
        Self {
            id,
            delay: Duration::from_millis(delay_ms),
            locators: locators.to_vec(),
            behaviour: Behaviour::Answer,
            network: true,
            finished: Arc::new(AtomicBool::new(false)),
        }
    }
    fn behaving(mut self, b: Behaviour) -> Self {
        self.behaviour = b;
        self
    }
    fn local(mut self) -> Self {
        self.network = false;
        self
    }
    fn entry(self, timeout_ms: u64) -> Entry {
        Entry::new(Arc::new(self)).with_timeout(Duration::from_millis(timeout_ms))
    }
}

#[async_trait]
impl SourceBackend for Fake {
    fn id(&self) -> &str {
        self.id
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Transport::Http],
            network: self.network,
        }
    }
    async fn find(&self, _q: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        tokio::time::sleep(self.delay).await;
        self.finished.store(true, Ordering::SeqCst);
        match self.behaviour {
            Behaviour::Answer => Ok(self
                .locators
                .iter()
                .map(|l| SourceCandidate {
                    key: format!("{}:{l}", self.id),
                    title: "Nosferatu".into(),
                    year: Some(1922),
                    transport: Transport::Http,
                    quality: Default::default(),
                    languages: vec![],
                    subtitles: vec![],
                    codec: Default::default(),
                    size_bytes: None,
                    health: Default::default(),
                    locator: Some(l.to_string()),
                    // A backend lying about who it is: the resolver must overwrite this.
                    provenance: Provenance {
                        backend: "impostor".into(),
                        ..Default::default()
                    },
                })
                .collect()),
            Behaviour::Fail => Err(SourceError::Unavailable("down".into())),
            Behaviour::Panic => panic!("a badly written backend"),
        }
    }
    async fn resolve(&self, c: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        tokio::time::sleep(self.delay).await;
        Ok(PlayableStream::Http {
            url: c.locator.clone().unwrap_or_default(),
            content_type: None,
            size_bytes: None,
        })
    }
}

fn query() -> MediaQuery {
    MediaQuery::film("Nosferatu", Some(1922))
}

#[tokio::test(start_paused = true)]
async fn a_fast_backend_answers_before_a_slow_one_times_out() {
    let resolver = Resolver::new(vec![
        Fake::new("slow", 60_000, &["https://slow.example/a"]).entry(300),
        Fake::new("fast", 10, &["https://fast.example/a"]).entry(300),
    ]);
    let start = Instant::now();
    let mut rx = resolver.find(&query());

    let first = rx.recv().await.unwrap();
    assert_eq!(first.outcome.backend, "fast");
    assert_eq!(first.candidates.len(), 1);
    let at = start.elapsed();
    assert!(
        at < Duration::from_millis(50),
        "fast results arrived at {at:?}"
    );

    let second = rx.recv().await.unwrap();
    assert_eq!(second.outcome.backend, "slow");
    assert_eq!(second.outcome.result, Err(SourceError::Timeout(300)));
    assert!(
        rx.recv().await.is_none(),
        "one message per backend, then closed"
    );
}

#[tokio::test(start_paused = true)]
async fn the_whole_search_is_bounded_by_the_timeout_not_the_slowest_backend() {
    let resolver = Resolver::new(vec![
        Fake::new("hung", 3_600_000, &["https://hung.example/a"]).entry(500),
        Fake::new("ok", 20, &["https://ok.example/a"]).entry(500),
    ]);
    let start = Instant::now();
    let found = resolver.find_all(&query()).await;
    let took = start.elapsed();
    assert!(took <= Duration::from_millis(510), "took {took:?}");
    assert_eq!(found.candidates.len(), 1);
    assert_eq!(found.outcomes.len(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_timed_out_backend_is_stopped_not_abandoned() {
    let hung = Fake::new("hung", 10_000, &[]);
    let finished = hung.finished.clone();
    let resolver = Resolver::new(vec![hung.entry(100)]);
    resolver.find_all(&query()).await;
    // Long past when it would have finished had it been left running.
    tokio::time::sleep(Duration::from_secs(20)).await;
    assert!(
        !finished.load(Ordering::SeqCst),
        "the hung backend ran to completion"
    );
}

#[tokio::test(start_paused = true)]
async fn a_panicking_backend_fails_alone() {
    let resolver = Resolver::new(vec![
        Fake::new("broken", 5, &[])
            .behaving(Behaviour::Panic)
            .entry(1_000),
        Fake::new("down", 5, &[])
            .behaving(Behaviour::Fail)
            .entry(1_000),
        Fake::new("fine", 5, &["https://fine.example/a"]).entry(1_000),
    ]);
    let found = resolver.find_all(&query()).await;
    assert_eq!(found.candidates.len(), 1);
    let failed: Vec<_> = found
        .outcomes
        .iter()
        .filter(|o| o.result.is_err())
        .map(|o| o.backend.as_str())
        .collect();
    assert_eq!(failed.len(), 2, "{failed:?}");
}

#[tokio::test(start_paused = true)]
async fn the_same_source_from_two_backends_is_merged_and_provenance_is_stamped() {
    let resolver = Resolver::new(vec![
        Fake::new(
            "first",
            10,
            &["https://same.example/f.mp4", "https://only-a.example/g"],
        )
        .entry(1_000),
        Fake::new("second", 20, &["https://same.example/f.mp4"]).entry(1_000),
    ]);
    let found = resolver.find_all(&query()).await;
    assert_eq!(found.candidates.len(), 2);
    let shared = &found.candidates[0];
    assert_eq!(
        shared.provenance.backend, "first",
        "not the impostor's claim"
    );
    assert_eq!(shared.provenance.also_from, vec!["second".to_string()]);
}

#[tokio::test(start_paused = true)]
async fn offline_skips_network_backends_and_unsupported_kinds_are_not_asked() {
    let resolver = Resolver::new(vec![
        Fake::new("net", 5, &["https://n.example/a"]).entry(1_000),
        Fake::new("disk", 5, &["file:///C:/films/a.mkv"])
            .local()
            .entry(1_000),
    ])
    .offline(true);
    let found = resolver.find_all(&query()).await;
    assert_eq!(found.outcomes.len(), 1);
    assert_eq!(found.outcomes[0].backend, "disk");

    let mut episode = query();
    episode.kind = MediaKind::Episode;
    assert!(resolver.find_all(&episode).await.outcomes.is_empty());
}

#[tokio::test(start_paused = true)]
async fn resolve_goes_to_the_backend_that_found_it_under_its_timeout() {
    let resolver = Resolver::new(vec![
        Fake::new("quick", 5, &["https://q.example/a"]).entry(1_000),
        Fake::new("sluggish", 5_000, &["https://s.example/a"]).entry(200),
    ]);
    let found = resolver.find_all(&query()).await;
    let quick = found
        .candidates
        .iter()
        .find(|c| c.provenance.backend == "quick")
        .unwrap();
    assert!(matches!(
        resolver.resolve(quick).await,
        Ok(PlayableStream::Http { .. })
    ));

    // The sluggish one's find timed out, so build its candidate by hand.
    let mut slow = quick.clone();
    slow.provenance.backend = "sluggish".into();
    assert_eq!(
        resolver.resolve(&slow).await,
        Err(SourceError::Timeout(200))
    );

    slow.provenance.backend = "uninstalled".into();
    assert!(matches!(
        resolver.resolve(&slow).await,
        Err(SourceError::NotFound(_))
    ));
}
