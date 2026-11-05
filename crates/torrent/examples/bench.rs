//! Phase 7's live measurements (E1, E2 and this engine's half of E6), against a legal,
//! well-seeded torrent the person running it supplies. Nothing about the torrent is
//! written down here or anywhere in the repository (SPEC.md §2.1).
//!
//! ```text
//! cargo run --release -p sinephile-torrent --example bench -- <path-to-.torrent> <scratch-dir> [bulk-seconds]
//! ```
//!
//! Prints one line per measurement. Every timing goes through the same path playback
//! will: the engine, the streaming server, and an HTTP range request on loopback.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sinephile_torrent::{AddOptions, Engine, EngineConfig, Source, StreamServer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn range(url: &str, from: u64, len: u64) -> (Duration, Duration) {
    let rest = url.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_once('/').unwrap();
    let start = Instant::now();
    let mut sock = tokio::net::TcpStream::connect(host).await.unwrap();
    let req = format!(
        "GET /{path} HTTP/1.1\r\nHost: {host}\r\nRange: bytes={from}-{}\r\n\r\n",
        from + len - 1
    );
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 64 * 1024];
    let mut first = None;
    let mut got = 0usize;
    loop {
        let n = sock.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        first.get_or_insert(start.elapsed());
        got += n;
    }
    assert!(got as u64 >= len, "short read: {got}");
    (first.unwrap(), start.elapsed())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let torrent = std::fs::read(&args[0]).expect("a .torrent file");
    let scratch = std::path::PathBuf::from(&args[1]);
    let bulk = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(60u64);

    let t0 = Instant::now();
    let mut config = EngineConfig::new(&scratch);
    config.dead_swarm_after = Duration::from_secs(60);
    let engine = Arc::new(Engine::start(config).await.unwrap());
    println!("session started          {:>8.0} ms", t0.elapsed().as_secs_f64() * 1e3);

    let t1 = Instant::now();
    let id = engine.add(Source::File(torrent), AddOptions::default()).await.unwrap();
    println!("torrent live             {:>8.0} ms", t1.elapsed().as_secs_f64() * 1e3);

    // The largest file: a disc image is not a video, so the selection rules do not apply.
    let file = engine.files(&id).unwrap().into_iter().max_by_key(|f| f.len).unwrap();
    let server = StreamServer::start().await.unwrap();
    let url = server.publish("bench", engine.stream_source(&id, &file).unwrap());

    let (ttfb, mib) = range(&url, 0, 1 << 20).await;
    println!("first byte (local HTTP)  {:>8.0} ms", ttfb.as_secs_f64() * 1e3);
    println!("first 1 MiB              {:>8.0} ms", mib.as_secs_f64() * 1e3);
    println!(
        "E1 proxy: add -> 1 MiB   {:>8.0} ms   (target < 8000)",
        t1.elapsed().as_secs_f64() * 1e3
    );

    let at = file.len * 6 / 10;
    let (sfb, smib) = range(&url, at, 1 << 20).await;
    println!("seek to 60%: first byte  {:>8.0} ms", sfb.as_secs_f64() * 1e3);
    println!("seek to 60%: 1 MiB       {:>8.0} ms   (E2 target < 5000)", smib.as_secs_f64() * 1e3);

    let before = engine.status(&id).unwrap().progress_bytes;
    let tb = Instant::now();
    tokio::time::sleep(Duration::from_secs(bulk)).await;
    let s = engine.status(&id).unwrap();
    let rate = (s.progress_bytes - before) as f64 / tb.elapsed().as_secs_f64();
    println!(
        "bulk over {bulk} s           {:>8.2} MiB/s   ({} live peers at the end)",
        rate / (1 << 20) as f64,
        s.live_peers
    );
    engine.remove(&id, true).await.ok();
}
