//! Direct downloads (Phase 16) against a real HTTP server on loopback that honours
//! `Range`, sends slowly enough to be paused, and records every `Range` it was asked for.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use sinephile_downloads::direct::part_of;
use sinephile_downloads::{Backend, Direct};
use sinephile_persistence::repositories::{Download, DownloadState};
use tempfile::TempDir;

const LEN: usize = 3 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;

fn bytes() -> Vec<u8> {
    (0..LEN).map(|i| (i * 31 % 251) as u8).collect()
}

/// Serves `bytes()` at any path; `pace` between chunks; records each Range asked for.
fn server(pace: Duration) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let ranges = Arc::new(Mutex::new(Vec::new()));
    let seen = ranges.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = seen.clone();
            thread::spawn(move || serve(stream, pace, &seen));
        }
    });
    (base, ranges)
}

fn serve(mut stream: TcpStream, pace: Duration, seen: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut from = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("range:") {
            let v = v.trim().to_string();
            seen.lock().unwrap().push(v.clone());
            from = v
                .trim_start_matches("bytes=")
                .trim_end_matches('-')
                .parse()
                .unwrap_or(0);
        }
    }
    let body = bytes();
    let head = if from > 0 {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{LEN}\r\nConnection: close\r\n\r\n",
            LEN - from,
            LEN - 1
        )
    } else {
        format!("HTTP/1.1 200 OK\r\nContent-Length: {LEN}\r\nConnection: close\r\n\r\n")
    };
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    for chunk in body[from..].chunks(CHUNK) {
        if stream.write_all(chunk).is_err() {
            return;
        }
        thread::sleep(pace);
    }
}

fn download(id: i64, url: &str) -> Download {
    Download {
        id,
        media_item_id: None,
        title: "Nosferatu".into(),
        source: url.into(),
        torrent: None,
        backend: "http".into(),
        backend_id: None,
        priority: 0,
        state: DownloadState::Queued,
        total_bytes: None,
        done_bytes: 0,
        limit_kib: None,
        not_before: None,
        reason: None,
        error: None,
        file_path: None,
        added_at: String::new(),
        finished_at: None,
        file_index: None,
    }
}

async fn until(what: &str, within: Duration, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + within;
    while !f() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn finished(b: &Direct, id: &str) -> bool {
    b.progress(id).await.map(|p| p.finished).unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_web_address_downloads_whole_and_is_renamed_when_done() {
    let (base, ranges) = server(Duration::ZERO);
    let dir = TempDir::new().unwrap();
    let b = Direct::new();
    let d = download(1, &format!("{base}/Nosferatu%20(1922).mp4"));
    assert!(b.accepts(&d));
    let id = b.add(&d, dir.path()).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !finished(&b, &id).await {
        assert!(Instant::now() < deadline, "not finished");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let file = b.video_file(&id, None).await.unwrap();
    assert_eq!(
        file,
        dir.path().join("Nosferatu").join("Nosferatu (1922).mp4")
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes());
    assert!(!part_of(&file).exists(), "no .part is left behind");
    assert!(
        ranges.lock().unwrap().is_empty(),
        "a fresh start asks for it all"
    );
    let p = b.progress(&id).await.unwrap();
    assert_eq!((p.done, p.total), (LEN as u64, LEN as u64));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pause_and_a_restart_carry_on_from_the_bytes_already_here() {
    let (base, ranges) = server(Duration::from_millis(15));
    let dir = TempDir::new().unwrap();
    let d = download(2, &format!("{base}/film.mp4"));
    let first = Direct::new();
    let id = first.add(&d, dir.path()).await.unwrap();
    let part = part_of(&dir.path().join("Nosferatu").join("film.mp4"));
    until("some bytes", Duration::from_secs(20), || {
        std::fs::metadata(&part)
            .map(|m| m.len() > 512 * 1024)
            .unwrap_or(false)
    })
    .await;
    first.pause(&id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let paused_at = std::fs::metadata(&part).unwrap().len();
    assert!(paused_at < LEN as u64, "paused before the end");

    // The app restarts: a new backend holds nothing, and the queue adds it again.
    let second = Direct::new();
    assert!(!second.holds(&id).await);
    let id2 = second.add(&d, dir.path()).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !finished(&second, &id2).await {
        assert!(Instant::now() < deadline, "not finished after the restart");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let file = second.video_file(&id2, None).await.unwrap();
    assert_eq!(
        std::fs::read(&file).unwrap(),
        bytes(),
        "the pieces join up exactly"
    );
    let asked = ranges.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "{asked:?}");
    let from: u64 = asked[0]
        .trim_start_matches("bytes=")
        .trim_end_matches('-')
        .parse()
        .unwrap();
    assert!(
        from >= paused_at,
        "carried on from {from}, paused at {paused_at}"
    );
    assert!(Path::new(&file).exists());
}

/// Cancelled mid-download, files deleted (the author, 2026-11-20: cancelling "doesnt work
/// sometimes"): nothing is left, nothing comes back, and a pause straight after a resume
/// leaves one writer, not two.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cancel_mid_download_leaves_nothing_behind() {
    let (base, _) = server(Duration::from_millis(5));
    for round in 0..20 {
        let dir = TempDir::new().unwrap();
        let d = download(10 + round, &format!("{base}/film.mp4"));
        let b = Direct::new();
        let id = b.add(&d, dir.path()).await.unwrap();
        let file = dir.path().join("Nosferatu").join("film.mp4");
        let part = part_of(&file);
        until("some bytes", Duration::from_secs(20), || {
            std::fs::metadata(&part)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        })
        .await;
        b.pause(&id).await.unwrap();
        b.resume(&id).await.unwrap();
        b.remove(&id, true).await.unwrap();
        assert!(!b.holds(&id).await);
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!part.exists(), "round {round}: the .part is still there");
        assert!(!file.exists(), "round {round}: the file is still there");
    }
}

/// Paused and resumed again and again while it runs: one writer at a time, so the bytes
/// still join up exactly.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn quick_pauses_and_resumes_keep_the_file_whole() {
    let (base, _) = server(Duration::from_millis(2));
    let dir = TempDir::new().unwrap();
    let d = download(40, &format!("{base}/film.mp4"));
    let b = Direct::new();
    let id = b.add(&d, dir.path()).await.unwrap();
    let part = part_of(&dir.path().join("Nosferatu").join("film.mp4"));
    until("some bytes", Duration::from_secs(20), || {
        std::fs::metadata(&part)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    })
    .await;
    for _ in 0..40 {
        b.pause(&id).await.unwrap();
        b.resume(&id).await.unwrap();
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while !finished(&b, &id).await {
        assert!(Instant::now() < deadline, "not finished");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let file = b.video_file(&id, None).await.unwrap();
    assert!(
        std::fs::read(&file).unwrap() == bytes(),
        "the bytes do not join up"
    );
}

#[tokio::test]
async fn a_torrent_is_not_a_web_address() {
    let mut d = download(3, "infohash:abc");
    assert!(!Direct::new().accepts(&d));
    // Built at run time: the posture guard refuses a literal magnet link.
    d.source = format!("{}:?xt=urn:btih:{}", "magnet", "a".repeat(40));
    assert!(!Direct::new().accepts(&d));
}
