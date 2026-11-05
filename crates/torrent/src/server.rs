//! The local streaming server — SPEC.md Phase 7: "a local HTTP server exposing an
//! in-progress torrent as a seekable stream with correct HTTP range support, so libmpv
//! can treat it as an ordinary file."
//!
//! Small on purpose, and written here rather than pulled in as a framework, because the
//! whole of its job is four rules that each need a test:
//!
//! 1. **Loopback only.** It binds `127.0.0.1`, never `0.0.0.0`: a stream is for this
//!    machine's player, not the network (§15 Phase 22's security review names this).
//! 2. **A per-session token in every path.** Another program on this machine that finds
//!    the port still cannot read a stream without the token, which is random per launch
//!    and never written down.
//! 3. **Byte-correct ranges.** `Range: bytes=a-b`, `a-` and `-n`, answered with `206` and
//!    `Content-Range`; a range past the end is `416`. Phase 7 E5 compares every byte.
//! 4. **One request per connection** (`Connection: close`). mpv's HTTP client opens a
//!    new connection per seek anyway, and it keeps the server free of keep-alive state.

use std::collections::HashMap;
use std::future::Future;
use std::io::{self, SeekFrom};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::distributions::Alphanumeric;
use rand::Rng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Anything that can be read from an offset: a torrent's file stream, or a file on disk.
pub trait ReadSeek: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T: AsyncRead + AsyncSeek + Send + Unpin> ReadSeek for T {}

pub type OpenFuture<'a> = Pin<Box<dyn Future<Output = io::Result<Box<dyn ReadSeek>>> + Send + 'a>>;

/// What the server serves at one address.
pub trait Source: Send + Sync + 'static {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn content_type(&self) -> &str;
    /// A fresh reader per request: two seeks in flight must not share a position.
    fn open(&self) -> OpenFuture<'_>;
}

/// A file on disk — what a finished torrent is served from (see `Engine`).
pub struct DiskFile {
    pub path: std::path::PathBuf,
    pub len: u64,
}

impl Source for DiskFile {
    fn len(&self) -> u64 {
        self.len
    }
    fn content_type(&self) -> &str {
        content_type_for(&self.path.to_string_lossy())
    }
    fn open(&self) -> OpenFuture<'_> {
        Box::pin(async move {
            let f = tokio::fs::File::open(&self.path).await?;
            Ok(Box::new(f) as Box<dyn ReadSeek>)
        })
    }
}

pub fn content_type_for(name: &str) -> &'static str {
    match name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("avi") => "video/x-msvideo",
        Some("mov") => "video/quicktime",
        Some("ts" | "m2ts") => "video/mp2t",
        Some("mpg" | "mpeg") => "video/mpeg",
        Some("ogv") => "video/ogg",
        _ => "application/octet-stream",
    }
}

/// Headers past this are not a media player talking to us.
const MAX_HEADER_BYTES: usize = 16 * 1024;
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);

type Routes = Arc<Mutex<HashMap<String, Arc<dyn Source>>>>;

pub struct StreamServer {
    addr: SocketAddr,
    token: String,
    routes: Routes,
    task: tokio::task::JoinHandle<()>,
}

impl StreamServer {
    /// Bind a free loopback port and start serving.
    pub async fn start() -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let token: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let routes: Routes = Arc::default();
        let task = tokio::spawn(accept(listener, token.clone(), routes.clone()));
        Ok(Self {
            addr,
            token,
            routes,
            task,
        })
    }

    /// Serve `source` and return the address the player should open.
    pub fn publish(&self, id: &str, source: Arc<dyn Source>) -> String {
        let id: String = id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        self.routes
            .lock()
            .expect("routes")
            .insert(id.clone(), source);
        format!("http://{}/{}/{}", self.addr, self.token, id)
    }

    pub fn unpublish(&self, id: &str) {
        self.routes.lock().expect("routes").remove(id);
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for StreamServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn accept(listener: TcpListener, token: String, routes: Routes) {
    loop {
        let Ok((socket, _)) = listener.accept().await else {
            continue;
        };
        let (token, routes) = (token.clone(), routes.clone());
        tokio::spawn(async move {
            if let Err(e) = serve(socket, &token, &routes).await {
                // A player closing the connection mid-body on a seek is normal.
                tracing::debug!(error = %e, "stream request ended");
            }
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Request {
    head: bool,
    path: String,
    range: Option<String>,
}

fn parse_request(text: &str) -> Result<Request, u16> {
    let mut lines = text.split("\r\n");
    let mut first = lines.next().ok_or(400u16)?.split(' ');
    let method = first.next().ok_or(400u16)?;
    let path = first.next().ok_or(400u16)?.to_string();
    let head = match method {
        "GET" => false,
        "HEAD" => true,
        _ => return Err(405),
    };
    let range = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("range"))
        .map(|(_, v)| v.trim().to_string());
    Ok(Request { head, path, range })
}

/// A range that cannot be served: answered with 416.
#[derive(Debug, PartialEq, Eq)]
pub struct Unsatisfiable;

/// The byte span to send, inclusive, or `Unsatisfiable` for 416. `Ok(None)` means the whole
/// file with a 200 — which is also the answer to a multi-range request, which RFC 9110
/// lets a server ignore.
pub fn byte_range(header: Option<&str>, len: u64) -> Result<Option<(u64, u64)>, Unsatisfiable> {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return Ok(None);
    };
    if spec.contains(',') {
        return Ok(None);
    }
    let (a, b) = spec.split_once('-').ok_or(Unsatisfiable)?;
    let (a, b) = (a.trim(), b.trim());
    if len == 0 {
        return Err(Unsatisfiable);
    }
    let span = match (a.is_empty(), b.is_empty()) {
        // -n: the last n bytes
        (true, false) => {
            let n: u64 = b.parse().map_err(|_| Unsatisfiable)?;
            if n == 0 {
                return Err(Unsatisfiable);
            }
            (len.saturating_sub(n), len - 1)
        }
        // a-
        (false, true) => (a.parse().map_err(|_| Unsatisfiable)?, len - 1),
        // a-b, with b clamped to the end
        (false, false) => {
            let (a, b): (u64, u64) = (
                a.parse().map_err(|_| Unsatisfiable)?,
                b.parse().map_err(|_| Unsatisfiable)?,
            );
            if b < a {
                return Err(Unsatisfiable);
            }
            (a, b.min(len - 1))
        }
        (true, true) => return Err(Unsatisfiable),
    };
    if span.0 >= len {
        return Err(Unsatisfiable);
    }
    Ok(Some(span))
}

async fn serve(mut socket: TcpStream, token: &str, routes: &Routes) -> io::Result<()> {
    let mut buf = Vec::with_capacity(1024);
    let read_head = async {
        let mut chunk = [0u8; 1024];
        loop {
            let n = socket.read(&mut chunk).await?;
            if n == 0 {
                return Ok::<bool, io::Error>(false);
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                return Ok(true);
            }
            if buf.len() > MAX_HEADER_BYTES {
                return Ok(false);
            }
        }
    };
    match tokio::time::timeout(HEADER_TIMEOUT, read_head).await {
        Ok(Ok(true)) => {}
        _ => return reply(&mut socket, 400, &[], None).await,
    }
    let text = String::from_utf8_lossy(&buf);
    let request = match parse_request(&text) {
        Ok(r) => r,
        Err(status) => return reply(&mut socket, status, &[], None).await,
    };

    // /<token>/<id>; anything else, including a wrong token, is a plain 404 that does
    // not say which part was wrong.
    let mut parts = request.path.trim_start_matches('/').splitn(2, '/');
    let (given, id) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let source = (constant_time_eq(given.as_bytes(), token.as_bytes()))
        .then(|| routes.lock().expect("routes").get(id).cloned())
        .flatten();
    let Some(source) = source else {
        return reply(&mut socket, 404, &[], None).await;
    };

    let len = source.len();
    let ctype = source.content_type().to_string();
    let (status, start, end) = match byte_range(request.range.as_deref(), len) {
        Ok(Some((a, b))) => (206, a, b),
        Ok(None) => (200, 0, len.saturating_sub(1)),
        Err(Unsatisfiable) => {
            let cr = format!("bytes */{len}");
            return reply(&mut socket, 416, &[("Content-Range", cr.as_str())], None).await;
        }
    };
    let body_len = if len == 0 { 0 } else { end - start + 1 };
    let content_range = format!("bytes {start}-{end}/{len}");
    let mut headers = vec![("Content-Type", ctype.as_str()), ("Accept-Ranges", "bytes")];
    if status == 206 {
        headers.push(("Content-Range", content_range.as_str()));
    }
    reply(&mut socket, status, &headers, Some(body_len)).await?;
    if request.head || body_len == 0 {
        return Ok(());
    }
    let mut reader = source.open().await?;
    reader.seek(SeekFrom::Start(start)).await?;
    let mut limited = reader.take(body_len);
    tokio::io::copy(&mut limited, &mut socket).await?;
    socket.shutdown().await
}

async fn reply(
    socket: &mut TcpStream,
    status: u16,
    headers: &[(&str, &str)],
    content_length: Option<u64>,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        _ => "Error",
    };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\n\r\n",
        content_length.unwrap_or(0)
    ));
    socket.write_all(head.as_bytes()).await?;
    if content_length.is_none() {
        socket.shutdown().await?;
    }
    Ok(())
}

/// Comparing the token in constant time, so its length and prefix cannot be learned
/// by timing a local brute force.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_follow_rfc_9110() {
        assert_eq!(byte_range(None, 100), Ok(None));
        assert_eq!(byte_range(Some("bytes=0-0"), 100), Ok(Some((0, 0))));
        assert_eq!(byte_range(Some("bytes=10-19"), 100), Ok(Some((10, 19))));
        assert_eq!(
            byte_range(Some("bytes=90-500"), 100),
            Ok(Some((90, 99))),
            "clamped"
        );
        assert_eq!(byte_range(Some("bytes=50-"), 100), Ok(Some((50, 99))));
        assert_eq!(byte_range(Some("bytes=-10"), 100), Ok(Some((90, 99))));
        assert_eq!(byte_range(Some("bytes=-500"), 100), Ok(Some((0, 99))));
        assert_eq!(
            byte_range(Some("bytes=100-"), 100),
            Err(Unsatisfiable),
            "past the end"
        );
        assert_eq!(byte_range(Some("bytes=20-10"), 100), Err(Unsatisfiable));
        assert_eq!(byte_range(Some("bytes=-0"), 100), Err(Unsatisfiable));
        assert_eq!(byte_range(Some("bytes=x-y"), 100), Err(Unsatisfiable));
        assert_eq!(
            byte_range(Some("bytes=0-1,5-6"), 100),
            Ok(None),
            "multi-range ignored"
        );
        assert_eq!(byte_range(Some("items=0-1"), 100), Ok(None), "not bytes");
    }

    #[test]
    fn requests_parse_and_unknown_methods_are_refused() {
        let r = parse_request("GET /t/x HTTP/1.1\r\nHost: a\r\nRange: bytes=0-9\r\n\r\n").unwrap();
        assert_eq!(
            r,
            Request {
                head: false,
                path: "/t/x".into(),
                range: Some("bytes=0-9".into())
            }
        );
        assert!(parse_request("HEAD /t/x HTTP/1.1\r\n\r\n").unwrap().head);
        assert_eq!(parse_request("POST /t/x HTTP/1.1\r\n\r\n"), Err(405));
    }
}
