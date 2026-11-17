//! Posters fetched from a real HTTP server on loopback (Phase 16, ADR-0048): fetched once
//! and kept, a missing one skipped for the next address, and nothing that is not an image.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use sinephile_artwork::{ArtworkCache, Posters};

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let mut buf = image::RgbImage::new(width, height);
    for (x, y, pixel) in buf.enumerate_pixels_mut() {
        *pixel = image::Rgb([
            (x * 7 % 256) as u8,
            (y * 11 % 256) as u8,
            ((x + y) % 256) as u8,
        ]);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(buf)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .expect("write jpeg");
    out.into_inner()
}

/// `/poster` is a JPEG, `/page` is HTML, anything else 404. Counts every request.
fn server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let image = jpeg(60, 90);
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            count.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                    break;
                }
            }
            let (status, kind, body): (&str, &str, Vec<u8>) = if first.contains("/poster ") {
                ("200 OK", "image/jpeg", image.clone())
            } else if first.contains("/page ") {
                ("200 OK", "text/html", b"<html>not a poster</html>".to_vec())
            } else {
                ("404 Not Found", "text/plain", b"no".to_vec())
            };
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    (base, asked)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_poster_is_fetched_once_and_kept() {
    let (base, asked) = server();
    let dir = tempfile::tempdir().unwrap();
    let posters = Posters::new(ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET));
    // A missing one first: skipped for the next address.
    let urls = [format!("{base}/missing"), format!("{base}/poster")];
    let first = posters.first(&urls).await.expect("a poster");
    assert_eq!(&first[..4], b"RIFF", "kept as WebP");
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    let again = posters.first(&urls).await.expect("the same poster");
    assert_eq!(again, first);
    assert_eq!(
        asked.load(Ordering::SeqCst),
        2,
        "the second time comes from the disk"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_that_is_not_an_image_is_kept_and_a_miss_is_not_asked_again() {
    let (base, asked) = server();
    let dir = tempfile::tempdir().unwrap();
    let posters = Posters::new(ArtworkCache::new(dir.path(), ArtworkCache::DEFAULT_BUDGET));
    let urls = [format!("{base}/page"), format!("{base}/missing")];
    assert!(posters.first(&urls).await.is_none());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    assert!(posters.first(&urls).await.is_none());
    assert_eq!(
        asked.load(Ordering::SeqCst),
        2,
        "a title with no poster is not asked again"
    );
    assert!(posters
        .first(&["file:///C:/Windows/win.ini".to_string()])
        .await
        .is_none());
}
