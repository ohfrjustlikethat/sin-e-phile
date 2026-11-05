//! The two properties the real transport must have for addresses a user supplied
//! (Phase 6 E3): a body cap that a lying or absent `Content-Length` cannot get round,
//! and no redirects. Both are tested against a raw HTTP server on loopback, because
//! the fake transport cannot lie about a length.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use sinephile_metadata_api::{HttpTransport, Request, Transport, TransportError};

/// Serve one canned response per connection, `n` times, on a free loopback port.
fn serve(response: Vec<u8>, n: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().take(n) {
            let mut stream = stream.unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(&response);
        }
    });
    format!("http://{addr}/")
}

fn unlimited_length_body(bytes: usize) -> Vec<u8> {
    // No Content-Length: the body runs until the connection closes, so only the
    // running count can stop it.
    let mut r =
        b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n".to_vec();
    r.extend(std::iter::repeat_n(b'a', bytes));
    r
}

#[tokio::test]
async fn a_body_with_no_length_is_cut_off_at_the_cap() {
    let url = serve(unlimited_length_body(5 * 1024 * 1024), 1);
    let err = HttpTransport::untrusted()
        .send(Request::get(url).limit(64 * 1024))
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::TooLarge(65536)), "{err}");
}

#[tokio::test]
async fn a_declared_length_over_the_cap_is_refused_before_reading() {
    let mut r = b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\n".to_vec();
    r.extend_from_slice(b"{}");
    let url = serve(r, 1);
    let err = HttpTransport::untrusted()
        .send(Request::get(url).limit(1024))
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::TooLarge(_)), "{err}");
}

#[tokio::test]
async fn a_body_under_the_cap_arrives_whole() {
    let url = serve(unlimited_length_body(1000), 1);
    let r = HttpTransport::untrusted()
        .send(Request::get(url).limit(1024))
        .await
        .unwrap();
    assert_eq!((r.status, r.body.len()), (200, 1000));
}

#[tokio::test]
async fn the_untrusted_client_does_not_follow_redirects() {
    let r =
        b"HTTP/1.1 302 Found\r\nLocation: http://elsewhere.invalid/\r\nContent-Length: 0\r\n\r\n";
    let url = serve(r.to_vec(), 1);
    let resp = HttpTransport::untrusted()
        .send(Request::get(url))
        .await
        .unwrap();
    assert_eq!(resp.status, 302, "the redirect came back to the caller");
    assert_eq!(resp.header("location"), Some("http://elsewhere.invalid/"));
}
