//! Phase 7 E3: "Sequential mode never starves — the buffer ahead of the playhead is
//! maintained under a documented bandwidth floor."
//!
//! A simulated player reads the file at a fixed bitrate, one chunk at a time, each chunk
//! due at a fixed moment after a short start-up buffer. The swarm is one seeder whose
//! upload is capped, so the bandwidth available is known exactly. "Starves" is measured
//! as lateness: how long after its moment a chunk arrived.
//!
//! The documented floor (docs/specs/streaming-scheduler.md): **available bandwidth at
//! least 1.5× the bitrate**, with a 2-second start-up buffer. Above it, no chunk may be
//! late by more than 250 ms. Below it — the negative control — playback must visibly
//! starve, or this test is not measuring anything.

mod common;

use std::time::{Duration, Instant};

use common::*;
use sinephile_torrent::SeedingPolicy;
use tokio::io::AsyncReadExt;

const BITRATE: usize = 2 * MIB; // bytes per second of "film"
const CHUNK: usize = 256 * KIB;
const STARTUP: Duration = Duration::from_secs(2);
const PLAY_SECONDS: usize = 12;

/// Play `PLAY_SECONDS` of a film at `BITRATE` over a swarm capped at `bandwidth`, and
/// return the worst lateness of any chunk.
async fn worst_lateness(bandwidth: usize) -> Duration {
    let len = BITRATE * (PLAY_SECONDS + 4);
    let s = swarm(&[("Test.Film.1922.mkv", len)], Some(bandwidth as u32)).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;
    let mut stream = leech.stream(&id, 0).await.unwrap();
    let mut buf = vec![0u8; CHUNK];

    let start = Instant::now();
    let mut worst = Duration::ZERO;
    let chunks = BITRATE * PLAY_SECONDS / CHUNK;
    for i in 0..chunks {
        stream.read_exact(&mut buf).await.unwrap();
        let due = STARTUP + Duration::from_secs_f64((i * CHUNK) as f64 / BITRATE as f64);
        let now = start.elapsed();
        worst = worst.max(now.saturating_sub(due));
        // A player consumes at the bitrate; it does not race ahead of its own clock.
        if now < due {
            tokio::time::sleep(due - now).await;
        }
    }
    worst
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn above_the_floor_the_playhead_never_waits() {
    let worst = worst_lateness(BITRATE * 3 / 2).await;
    println!("1.5x the bitrate: worst lateness {worst:?}");
    assert!(
        worst <= Duration::from_millis(250),
        "starved by {worst:?} above the floor"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn below_the_floor_it_does_starve_so_the_test_can_fail() {
    let worst = worst_lateness(BITRATE * 7 / 10).await;
    println!("0.7x the bitrate: worst lateness {worst:?}");
    assert!(
        worst > Duration::from_secs(1),
        "did not starve at 0.7x — the harness measures nothing"
    );
}
