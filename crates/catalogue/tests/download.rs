//! Resumed downloads land where they belong (Phase 16, round 9, D90).
//!
//! The author's meaning file came down the right size with the right header and 5 MB of the
//! wrong bytes 66 MiB in: the same file's own data, 1,005,504 bytes late. A resume had asked
//! for "the rest from byte N", been answered with a `206 Partial Content` that began somewhere
//! else, and appended it as if it began at N. The answer says where it begins
//! (`Content-Range`); the downloader never read it. These tests serve a file over plain HTTP
//! from a thread, honestly or not, and check the file on disk is the file.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use sinephile_catalogue::Downloader;

/// How the test server answers a `Range` request.
#[derive(Clone, Copy)]
enum Answer {
    /// `206`, from where it was asked, saying so.
    Honest,
    /// `206`, from `shift` bytes earlier than asked, and saying so in `Content-Range`.
    Elsewhere { shift: usize },
    /// `206`, from where it was asked, without a `Content-Range`.
    Unlabelled,
}

/// A file served over HTTP on 127.0.0.1 until the test ends; records each request's `Range`.
fn serve(body: Vec<u8>, answer: Answer) -> (String, Arc<Mutex<Vec<Option<u64>>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!(
        "http://{}/meaning.bin",
        listener.local_addr().expect("addr")
    );
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut range = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("range: bytes=") {
                    range = rest.trim().trim_end_matches('-').parse::<u64>().ok();
                }
            }
            log.lock().expect("log").push(range);
            let total = body.len();
            let (head, from) = match (range, answer) {
                (None, _) => (format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\n"), 0),
                (Some(at), Answer::Honest) => {
                    let at = at as usize;
                    (
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {at}-{}/{total}\r\n",
                            total - at,
                            total - 1
                        ),
                        at,
                    )
                }
                (Some(at), Answer::Elsewhere { shift }) => {
                    let start = at as usize - shift;
                    (
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{total}\r\n",
                            total - start,
                            total - 1
                        ),
                        start,
                    )
                }
                (Some(at), Answer::Unlabelled) => {
                    let at = at as usize;
                    (
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n",
                            total - at
                        ),
                        at,
                    )
                }
            };
            let _ = stream.write_all(format!("{head}Connection: close\r\n\r\n").as_bytes());
            let _ = stream.write_all(&body[from..]);
        }
    });
    (url, asked)
}

/// Bytes that differ everywhere, so a shifted copy cannot pass for the original.
fn file_of(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 + i / 251) as u8).collect()
}

#[tokio::test]
async fn an_honest_resume_continues_from_where_it_stopped() {
    let body = file_of(300_000);
    let (url, asked) = serve(body.clone(), Answer::Honest);
    let dir = tempfile::tempdir().expect("tempdir");
    let to = dir.path().join("meaning.bin");
    std::fs::write(to.with_extension("part"), &body[..120_000]).expect("part");

    let done = Downloader::new()
        .fetch(&url, &to, |_| {})
        .await
        .expect("fetch");

    assert!(
        std::fs::read(&to).expect("read") == body,
        "the file on disk is the file"
    );
    assert_eq!(done.resumed_from, 120_000);
    assert_eq!(*asked.lock().unwrap(), [Some(120_000)]);
}

#[tokio::test]
async fn a_resume_answered_from_elsewhere_starts_over_and_the_file_is_whole() {
    // The author's case: asked from N, answered from N minus about a megabyte.
    let body = file_of(300_000);
    let (url, asked) = serve(body.clone(), Answer::Elsewhere { shift: 40_000 });
    let dir = tempfile::tempdir().expect("tempdir");
    let to = dir.path().join("meaning.bin");
    std::fs::write(to.with_extension("part"), &body[..120_000]).expect("part");

    let done = Downloader::new()
        .fetch(&url, &to, |_| {})
        .await
        .expect("fetch");

    assert!(
        std::fs::read(&to).expect("read") == body,
        "the file on disk is the file, not a shifted copy of part of it"
    );
    assert_eq!(done.resumed_from, 0, "it started over");
    assert_eq!(*asked.lock().unwrap(), [Some(120_000), None]);
}

#[tokio::test]
async fn a_partial_answer_that_does_not_say_where_it_starts_is_not_trusted() {
    let body = file_of(200_000);
    let (url, asked) = serve(body.clone(), Answer::Unlabelled);
    let dir = tempfile::tempdir().expect("tempdir");
    let to = dir.path().join("meaning.bin");
    std::fs::write(to.with_extension("part"), &body[..50_000]).expect("part");

    Downloader::new()
        .fetch(&url, &to, |_| {})
        .await
        .expect("fetch");

    assert!(
        std::fs::read(&to).expect("read") == body,
        "the file on disk is the file"
    );
    assert_eq!(*asked.lock().unwrap(), [Some(50_000), None]);
}
