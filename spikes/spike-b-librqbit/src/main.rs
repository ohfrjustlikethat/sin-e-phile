//! Spike B — librqbit sequential streaming (SPEC.md Phase 1, risk R2).
//!
//! SPIKE CODE. Throwaway. Not the Phase 7 design.
//!
//! Two questions, and the API audit matters more than the number:
//!
//!   Q1  Does the API expose enough control to build the Phase 7 deadline
//!       scheduler? Without runtime piece prioritisation there is nothing to
//!       build on and R2 fires regardless of how fast it downloads.
//!   Q2  Time to first usable bytes on a healthy swarm. R2's trigger is 20 s
//!       (the budget is 8 s; 20 s is where the gap stops being tuning).
//!
//! Also measured, because Phase 7 has an exit criterion for it: time to first
//! byte after seeking into an unbuffered region (budget 5 s).
//!
//! POSTURE (SPEC.md §2.1): the torrent URL is a REQUIRED ARGUMENT and is never
//! committed. `tools/guard` blocks `.torrent` URLs and bare infohashes by
//! design, so hardcoding one here would fail the pre-commit hook — correctly.
//! Use a legal, well-seeded source: an Internet Archive item or a Linux ISO.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use librqbit::{AddTorrentOptions, AddTorrentResponse, Session, SessionOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

const READ_CHUNK: usize = 1024 * 1024; // 1 MiB counts as "usable bytes"

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "librqbit=warn,spike_b_librqbit=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: spike-b-librqbit <torrent-url|magnet|path> [output-dir]\n\
             \n\
             Use a LEGAL, well-seeded source (Internet Archive item, Linux ISO).\n\
             The URL is an argument on purpose and must never be committed."
        );
        std::process::exit(2);
    }
    let source = args[1].clone();
    let out_dir = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| std::env::temp_dir().join("spike-b").to_string_lossy().into());
    std::fs::create_dir_all(&out_dir).ok();

    println!("output dir: {out_dir}");
    println!();

    let t_start = Instant::now();
    let session = Session::new_with_opts(out_dir.clone().into(), SessionOptions::default())
        .await
        .context("creating session")?;
    println!("session created      {:>8.0} ms", ms(t_start));

    // ── Metadata ─────────────────────────────────────────────────────────────
    let t_meta = Instant::now();
    let handle = match session
        .add_torrent(
            librqbit::AddTorrent::from_cli_argument(&source)?,
            Some(AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await
        .context("adding torrent")?
    {
        AddTorrentResponse::Added(_, h) => h,
        AddTorrentResponse::AlreadyManaged(_, h) => h,
        AddTorrentResponse::ListOnly(_) => anyhow::bail!("list-only response"),
    };
    println!("metadata resolved    {:>8.0} ms", ms(t_meta));

    // The torrent starts in `initializing`; `stream()` needs it Live.
    let t_live = Instant::now();
    tokio::time::timeout(Duration::from_secs(60), handle.wait_until_initialized())
        .await
        .context("timed out waiting for the torrent to go live")??;
    println!("torrent live         {:>8.0} ms", ms(t_live));

    // Pick the largest file — the video in a real torrent.
    let (file_id, file_name, file_len) = handle.with_metadata(|m| {
        let mut best = (0usize, String::new(), 0u64);
        for (i, fi) in m.file_infos.iter().enumerate() {
            if fi.len > best.2 {
                best = (i, fi.relative_filename.to_string_lossy().into(), fi.len);
            }
        }
        best
    })?;
    println!(
        "largest file         #{file_id}  {:.1} MiB  {file_name}",
        file_len as f64 / 1048576.0
    );
    println!();

    // ── Q2: time to first usable bytes ───────────────────────────────────────
    let t_stream = Instant::now();
    let mut stream = handle
        .clone()
        .stream(file_id)
        .await
        .context("opening stream")?;

    let mut buf = vec![0u8; READ_CHUNK];
    let mut got = 0usize;
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut first_byte_ms: Option<f64> = None;

    while got < READ_CHUNK && Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(20), stream.read(&mut buf[got..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                if first_byte_ms.is_none() {
                    first_byte_ms = Some(ms(t_stream));
                    println!("FIRST BYTE           {:>8.0} ms", first_byte_ms.unwrap());
                }
                got += n;
            }
            Ok(Err(e)) => {
                println!("read error: {e}");
                break;
            }
            Err(_) => {
                println!("  ...still waiting ({} KiB so far)", got / 1024);
            }
        }
    }
    let ttfb = ms(t_stream);
    println!(
        "FIRST {} KiB          {:>8.0} ms   <- time to first usable bytes",
        got / 1024,
        ttfb
    );

    let stats = handle.stats();
    println!(
        "swarm                peers live={} queued={}  fetched {:.1} MiB",
        stats.live.as_ref().map(|l| l.snapshot.peer_stats.live).unwrap_or(0),
        stats.live.as_ref().map(|l| l.snapshot.peer_stats.queued).unwrap_or(0),
        stats.progress_bytes as f64 / 1048576.0
    );
    println!();

    // ── Seek re-prioritisation (Phase 7 exit criterion: < 5 s) ───────────────
    let seek_to = file_len / 2;
    println!("seeking to 50% ({:.1} MiB) — an unbuffered region", seek_to as f64 / 1048576.0);
    let t_seek = Instant::now();
    stream.seek(std::io::SeekFrom::Start(seek_to)).await?;

    let mut got2 = 0usize;
    let deadline2 = Instant::now() + Duration::from_secs(60);
    while got2 < 256 * 1024 && Instant::now() < deadline2 {
        match tokio::time::timeout(Duration::from_secs(20), stream.read(&mut buf[got2..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => got2 += n,
            Ok(Err(e)) => {
                println!("read error after seek: {e}");
                break;
            }
            Err(_) => println!("  ...still waiting after seek"),
        }
    }
    let seek_ms = ms(t_seek);
    println!(
        "AFTER SEEK: {} KiB    {:>8.0} ms   <- re-prioritisation latency",
        got2 / 1024,
        seek_ms
    );

    println!();
    println!("=== SPIKE B SUMMARY ===");
    println!("  time to first usable bytes  {ttfb:.0} ms   (R2 trigger: > 20000 ms)");
    println!("  seek re-prioritisation      {seek_ms:.0} ms   (Phase 7 target: < 5000 ms)");
    println!(
        "  verdict                     {}",
        if got >= READ_CHUNK && ttfb < 20000.0 {
            "PASS"
        } else {
            "SEE ABOVE"
        }
    );

    drop(stream);
    session.stop().await;
    Ok(())
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
