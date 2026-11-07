//! Automatic fallback (SPEC.md Phase 9 E4): with a fake player for the rules, and with
//! the real one, headless, for the 10-second budget.

use std::collections::HashMap;
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sinephile_playback::fallback::{start, Attempt, FallbackError, Patience, Screen};
use sinephile_player::{dll_path, Options, Player};
use sinephile_sources::{Entry, Resolver};
use source_protocol::{
    async_trait, BackendCapabilities, Health, MediaKind, MediaQuery, PlayableStream, Provenance,
    SourceBackend, SourceCandidate, SourceError, Transport,
};

/// A source whose candidates resolve to their own key as a URL, unless the key says
/// `unresolvable`.
struct Echo;

#[async_trait]
impl SourceBackend for Echo {
    fn id(&self) -> &str {
        "echo"
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            media: vec![MediaKind::Film],
            transports: vec![Transport::Http],
            network: false,
        }
    }
    async fn find(&self, _q: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError> {
        Ok(vec![])
    }
    async fn resolve(&self, c: &SourceCandidate) -> Result<PlayableStream, SourceError> {
        if c.key.contains("unresolvable") {
            return Err(SourceError::Unavailable("gone".into()));
        }
        Ok(if c.key.starts_with("http") {
            PlayableStream::Http {
                url: c.key.clone(),
                content_type: None,
                size_bytes: None,
            }
        } else {
            PlayableStream::File {
                path: c.key.clone(),
            }
        })
    }
}

fn resolver() -> Resolver {
    Resolver::new(vec![Entry::new(Arc::new(Echo))])
}

fn candidate(key: &str) -> SourceCandidate {
    SourceCandidate {
        key: key.into(),
        title: "T".into(),
        year: None,
        transport: Transport::Http,
        quality: Default::default(),
        languages: vec![],
        subtitles: vec![],
        codec: Default::default(),
        size_bytes: None,
        duration_seconds: None,
        health: Health::Direct,
        locator: None,
        provenance: Provenance {
            backend: "echo".into(),
            ..Default::default()
        },
    }
}

#[derive(Clone, Copy)]
enum Act {
    /// mpv reports an error at once.
    Fail,
    /// Never loads.
    Hang,
    /// Loads, then shows a frame after this long.
    FrameAfter(Duration),
}

/// A player that does what its script says for each target.
#[derive(Default)]
struct Fake {
    script: HashMap<String, Act>,
    opened: Mutex<Vec<String>>,
    /// Becomes stale after this many opens.
    stale_after: Option<usize>,
}

impl Fake {
    fn act(&self) -> Act {
        let last = self.opened.lock().unwrap().last().cloned().unwrap();
        self.script[&last]
    }
}

impl Screen for Fake {
    fn open(&self, target: &str, _start: Option<f64>) -> Result<(), String> {
        self.opened.lock().unwrap().push(target.into());
        Ok(())
    }
    fn wait_loaded(&self, timeout: Duration) -> Result<(), String> {
        match self.act() {
            Act::Fail => Err("404".into()),
            Act::Hang => {
                std::thread::sleep(timeout);
                Err("timed out".into())
            }
            Act::FrameAfter(_) => Ok(()),
        }
    }
    fn wait_first_frame(&self, timeout: Duration) -> Result<(), String> {
        match self.act() {
            Act::FrameAfter(d) if d <= timeout => {
                std::thread::sleep(d);
                Ok(())
            }
            _ => {
                std::thread::sleep(timeout);
                Err("timed out".into())
            }
        }
    }
    fn superseded(&self) -> bool {
        self.stale_after
            .is_some_and(|n| self.opened.lock().unwrap().len() >= n)
    }
}

fn fake(script: &[(&str, Act)]) -> Arc<Fake> {
    Arc::new(Fake {
        script: script.iter().map(|(k, a)| (k.to_string(), *a)).collect(),
        ..Default::default()
    })
}

const QUICK: Patience = Patience {
    load: Duration::from_millis(150),
    frame: Duration::from_millis(400),
};

fn first(target: &str) -> (String, String) {
    (target.into(), "echo".into())
}

#[tokio::test]
async fn a_failing_first_choice_falls_through_to_the_next() {
    let screen = fake(&[
        ("http://a.example/1", Act::Fail),
        ("http://a.example/2", Act::Hang),
        ("http://a.example/3", Act::FrameAfter(Duration::ZERO)),
    ]);
    let rest = vec![
        candidate("http://a.example/2"),
        candidate("http://a.example/3"),
    ];
    let started = start(
        screen.clone(),
        first("http://a.example/1"),
        rest,
        &resolver(),
        None,
        None,
        QUICK,
    )
    .await
    .unwrap();
    assert_eq!(started.open, "http://a.example/3");
    assert_eq!(
        started
            .failed
            .iter()
            .map(|a| a.reason.as_str())
            .collect::<Vec<_>>(),
        ["did not load: 404", "did not load: timed out"],
        "each skipped copy says why"
    );
}

#[tokio::test]
async fn slow_is_not_broken() {
    // Loaded, bytes arriving, the frame comes after longer than the load patience.
    let screen = fake(&[(
        "http://a.example/slow",
        Act::FrameAfter(Duration::from_millis(300)),
    )]);
    let started = start(
        screen,
        first("http://a.example/slow"),
        vec![],
        &resolver(),
        None,
        None,
        QUICK,
    )
    .await
    .unwrap();
    assert!(started.failed.is_empty());
}

#[tokio::test]
async fn an_unresolvable_fallback_is_skipped_and_recorded() {
    let screen = fake(&[
        ("http://a.example/1", Act::Fail),
        ("http://a.example/3", Act::FrameAfter(Duration::ZERO)),
    ]);
    let rest = vec![
        candidate("http://a.example/unresolvable"),
        candidate("http://a.example/3"),
    ];
    let started = start(
        screen.clone(),
        first("http://a.example/1"),
        rest,
        &resolver(),
        None,
        None,
        QUICK,
    )
    .await
    .unwrap();
    assert_eq!(started.open, "http://a.example/3");
    assert_eq!(started.failed.len(), 2);
    assert_eq!(
        *screen.opened.lock().unwrap(),
        ["http://a.example/1", "http://a.example/3"],
        "nothing unresolvable is opened"
    );
}

#[tokio::test]
async fn when_nothing_starts_every_reason_is_kept() {
    let screen = fake(&[
        ("http://a.example/1", Act::Fail),
        ("http://a.example/2", Act::Fail),
    ]);
    let err = start(
        screen,
        first("http://a.example/1"),
        vec![candidate("http://a.example/2")],
        &resolver(),
        None,
        None,
        QUICK,
    )
    .await
    .unwrap_err();
    assert_eq!(
        err,
        FallbackError::Exhausted(vec![
            Attempt {
                backend: "echo".into(),
                reason: "did not load: 404".into()
            },
            Attempt {
                backend: "echo".into(),
                reason: "did not load: 404".into()
            },
        ])
    );
}

#[tokio::test]
async fn a_newer_play_stops_the_old_fallback() {
    // The viewer pressed Play on something else while the first choice was failing:
    // the old loop must not open its next copy over the new film.
    let screen = Arc::new(Fake {
        stale_after: Some(1),
        ..Arc::try_unwrap(fake(&[
            ("http://a.example/1", Act::Fail),
            ("http://a.example/2", Act::FrameAfter(Duration::ZERO)),
        ]))
        .ok()
        .unwrap()
    });
    let err = start(
        screen.clone(),
        first("http://a.example/1"),
        vec![candidate("http://a.example/2")],
        &resolver(),
        None,
        None,
        QUICK,
    )
    .await
    .unwrap_err();
    assert_eq!(err, FallbackError::Superseded);
    assert_eq!(screen.opened.lock().unwrap().len(), 1);
}

#[test]
fn only_honest_copies_below_the_chosen_one_are_fallbacks() {
    use sinephile_playback::{Context, PlayPlan};
    let mut fake4k = candidate("fake");
    fake4k.quality.resolution = source_protocol::Resolution::Uhd;
    fake4k.size_bytes = Some(1_000_000);
    fake4k.duration_seconds = Some(6000);
    let ranked = sinephile_selection::rank(
        vec![candidate("a"), candidate("b"), candidate("c"), fake4k],
        &Context::default(),
    );
    let plan = PlayPlan {
        media_item_id: 1,
        title: "T".into(),
        open: "x".into(),
        start: None,
        backend: "echo".into(),
        offered: 4,
        score: ranked[1].1.clone(),
        offers: Default::default(),
        original_language: None,
        chosen: 1,
        ranked,
        stream: None,
        runtime_minutes: None,
    };
    let keys: Vec<String> = plan.fallbacks().into_iter().map(|c| c.key).collect();
    assert_eq!(keys, ["c"], "not the one above, not the fake");
}

// ── The real player ──────────────────────────────────────────────────────────

/// The real player, headless, as the app's `Screen` is.
struct Real(Player);

impl Screen for Real {
    fn open(&self, target: &str, start: Option<f64>) -> Result<(), String> {
        self.0.load(target, start).map_err(|e| e.to_string())
    }
    fn wait_loaded(&self, timeout: Duration) -> Result<(), String> {
        self.0.wait_loaded(timeout).map_err(|e| e.to_string())
    }
    fn wait_first_frame(&self, timeout: Duration) -> Result<(), String> {
        self.0
            .wait_first_frame(timeout)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dead_first_choice_falls_back_within_ten_seconds() {
    // Phase 9 E4, with the app's real patience. The top choice is a server that accepts
    // the connection and then sends nothing, the hardest failure to see: no error ever
    // arrives, only silence.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = format!("http://{}/film.mp4", silent.local_addr().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let film = dir.path().join("film.mkv");
    let ok = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=24:duration=5",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
        ])
        .arg(&film)
        .status()
        .expect("FFmpeg must be on PATH")
        .success();
    assert!(ok);

    let player = Player::new(
        &dll_path(),
        &Options {
            headless: true,
            ..Default::default()
        },
    )
    .unwrap();
    let t0 = Instant::now();
    let started = start(
        Arc::new(Real(player)),
        first(&dead),
        vec![candidate(&film.to_string_lossy())],
        &resolver(),
        None,
        None,
        Patience::default(),
    )
    .await
    .unwrap();
    let took = t0.elapsed();
    println!("fallback: dead first choice -> first frame of the next in {took:?}");
    assert_eq!(started.open, film.to_string_lossy());
    assert_eq!(started.failed.len(), 1);
    assert!(took < Duration::from_secs(10), "{took:?}");
    drop(silent);
}
