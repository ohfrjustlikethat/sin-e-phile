//! Addons end to end, against a REAL HTTP server on loopback speaking the protocol,
//! through the real untrusted transport. The fake transport cannot redirect, stall or
//! send five megabytes, and those are exactly the behaviours under test (E3).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use sinephile_metadata_api::HttpTransport;
use sinephile_persistence::repositories::SourceRepository;
use sinephile_persistence::Db;
use sinephile_sources::addons::{describe, fetch_manifest, install, installed, InstallError};
use sinephile_sources::backends::{all, Context};
use sinephile_sources::registry::resolver_from;
use source_protocol::{MediaQuery, PlayableStream, SourceError};

/// How the fake addon answers one path.
#[derive(Clone)]
enum Reply {
    Json(String),
    Redirect(&'static str),
    Huge,
    Stall(Duration),
}

/// A tiny HTTP/1.1 server: one thread, one connection at a time is enough, and every
/// response closes the connection so no keep-alive state can leak between tests.
fn addon(routes: Vec<(&'static str, Reply)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let routes = routes.clone();
            thread::spawn(move || answer(stream, &routes));
        }
    });
    base
}

fn answer(mut stream: TcpStream, routes: &[(&'static str, Reply)]) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split(' ').nth(1).unwrap_or("/").to_string();
    let mut length = 0;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).unwrap();
        if h.trim().is_empty() {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let reply = routes
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, r)| r.clone());
    let head = |status: &str, extra: &str, len: usize| {
        format!("HTTP/1.1 {status}\r\nConnection: close\r\nContent-Type: application/json\r\n{extra}Content-Length: {len}\r\n\r\n")
    };
    let _ = match reply {
        Some(Reply::Json(json)) => {
            stream.write_all((head("200 OK", "", json.len()) + &json).as_bytes())
        }
        Some(Reply::Redirect(to)) => {
            stream.write_all(head("302 Found", &format!("Location: {to}\r\n"), 0).as_bytes())
        }
        Some(Reply::Huge) => {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            let chunk = vec![b' '; 64 * 1024];
            (0..80).try_for_each(|_| stream.write_all(&chunk))
        }
        Some(Reply::Stall(d)) => {
            thread::sleep(d);
            stream
                .write_all(head("200 OK", "", 2).as_bytes())
                .and_then(|_| stream.write_all(b"{}"))
        }
        None => stream.write_all(head("404 Not Found", "", 0).as_bytes()),
    };
}

fn manifest_for(base: &str) -> String {
    format!(
        r#"{{"protocol":1,"id":"test.loopback","name":"Loopback Films","version":"0.1",
            "description":"A test addon.","media":["film"],"transports":["http"],
            "endpoints":{{"find":"{base}/find","resolve":"{base}/resolve"}}}}"#
    )
}

const FOUND: &str = r#"{"candidates":[{"key":"nosferatu-1","title":"Nosferatu","year":1922,
    "transport":"http","locator":"https://cdn.example/nosferatu.mp4",
    "provenance":{"backend":"local"}}]}"#;
const RESOLVED: &str = r#"{"stream":{"kind":"http","url":"https://cdn.example/nosferatu.mp4"}}"#;

fn untrusted() -> Arc<HttpTransport> {
    Arc::new(HttpTransport::untrusted())
}

async fn install_from(base: &str, db: &Db) -> Context {
    let (url, manifest) = fetch_manifest(&*untrusted(), &format!("{base}/manifest.json"))
        .await
        .unwrap();
    install(db, &url, &manifest).await.unwrap();
    let (addons, refused) = installed(db).await.unwrap();
    assert!(refused.is_empty());
    Context {
        untrusted_http: untrusted(),
        addons,
        ..Context::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn paste_describe_install_find_resolve() {
    // The manifest must name the server's own address, so bind first, then route.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes = vec![
        ("/manifest.json", Reply::Json(manifest_for(&base))),
        ("/find", Reply::Json(FOUND.into())),
        ("/resolve", Reply::Json(RESOLVED.into())),
    ];
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let routes = routes.clone();
            thread::spawn(move || answer(stream, &routes));
        }
    });

    let db = Db::in_memory().await.unwrap();
    let (url, manifest) = fetch_manifest(&*untrusted(), &format!("{base}/manifest.json"))
        .await
        .unwrap();
    let shown = describe(&url, &manifest, &[]);
    assert_eq!(shown.name, "Loopback Films");
    assert_eq!(shown.finds, "Finds films.");
    assert_eq!(shown.delivers, ["Streams over the web."]);
    assert_eq!(shown.host, "127.0.0.1");
    assert!(!shown.replaces);

    install(&db, &url, &manifest).await.unwrap();
    let (addons, _) = installed(&db).await.unwrap();
    let ctx = Context {
        untrusted_http: untrusted(),
        addons,
        ..Context::default()
    };
    let resolver = resolver_from(&db, all(&ctx)).await.unwrap();
    assert!(resolver.ids().contains(&"addon:test.loopback"));

    let found = resolver
        .find_all(&MediaQuery::film("Nosferatu", Some(1922)))
        .await;
    let ours: Vec<_> = found
        .candidates
        .iter()
        .filter(|c| c.provenance.backend == "addon:test.loopback")
        .collect();
    assert_eq!(
        ours.len(),
        1,
        "the addon's claim to be `local` was overwritten"
    );
    let stream = resolver.resolve(ours[0]).await.unwrap();
    assert!(
        matches!(stream, PlayableStream::Http { ref url, .. } if url == "https://cdn.example/nosferatu.mp4")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manifest_that_redirects_or_is_missing_is_explained() {
    let base = addon(vec![(
        "/moved.json",
        Reply::Redirect("https://elsewhere.invalid/m.json"),
    )]);
    let err = fetch_manifest(&*untrusted(), &format!("{base}/moved.json"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, InstallError::Status { status: 302, .. }),
        "{err}"
    );
    assert!(err.to_string().contains("redirect"), "{err}");

    let err = fetch_manifest(&*untrusted(), &format!("{base}/nothing.json"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no manifest"), "{err}");

    let err = fetch_manifest(&*untrusted(), "http://example.com/m.json")
        .await
        .unwrap_err();
    assert!(
        matches!(err, InstallError::Manifest(_)),
        "plain http off-machine: {err}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hostile_addon_costs_only_itself() {
    // Three addons on one host would share an id-space; give each its own server.
    async fn hostile(db: &Db, id: &str, find: Reply) -> (Context, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let manifest = manifest_for(&base).replace("test.loopback", id);
        let routes = vec![("/manifest.json", Reply::Json(manifest)), ("/find", find)];
        thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let routes = routes.clone();
                thread::spawn(move || answer(s, &routes));
            }
        });
        (install_from(&base, db).await, format!("addon:{id}"))
    }

    let db = Db::in_memory().await.unwrap();
    let (_, redirecting) = hostile(
        &db,
        "test.redirect",
        Reply::Redirect("http://elsewhere.invalid/"),
    )
    .await;
    let (_, flooding) = hostile(&db, "test.flood", Reply::Huge).await;
    let (ctx, stalling) = hostile(&db, "test.stall", Reply::Stall(Duration::from_secs(30))).await;
    let (addons, _) = installed(&db).await.unwrap();
    assert_eq!(addons.len(), 3);
    let ctx = Context { addons, ..ctx };

    // Real backends with the resolver's real default timeout: the staller is bounded by
    // it, not by its own 30 s.
    let resolver = resolver_from(&db, all(&ctx)).await.unwrap();
    let start = Instant::now();
    let found = resolver
        .find_all(&MediaQuery::film("Nosferatu", Some(1922)))
        .await;
    let took = start.elapsed();

    let outcome = |id: &str| {
        found
            .outcomes
            .iter()
            .find(|o| o.backend == id)
            .unwrap()
            .result
            .clone()
    };
    assert!(
        matches!(outcome(&redirecting), Err(SourceError::InvalidResponse(ref m)) if m.contains("redirect"))
    );
    assert!(
        matches!(outcome(&flooding), Err(SourceError::Unavailable(ref m)) if m.contains("larger than")),
        "{:?}",
        outcome(&flooding)
    );
    assert!(
        matches!(outcome(&stalling), Err(SourceError::Timeout(_))),
        "{:?}",
        outcome(&stalling)
    );
    // Bounded by the resolver's default timeout, not the 30 s stall.
    assert!(took < Duration::from_secs(10), "took {took:?}");
}

#[tokio::test]
async fn a_stored_manifest_that_no_longer_passes_is_not_loaded() {
    let db = Db::in_memory().await.unwrap();
    // As if edited by hand: an endpoint moved to another host.
    let tampered = r#"{"protocol":1,"id":"test.tampered","name":"T","version":"1","description":"",
        "media":["film"],"transports":["http"],
        "endpoints":{"find":"https://evil.example/find","resolve":"https://addon.example/resolve"},
        "homepage":null}"#;
    SourceRepository::new(&db)
        .install_addon(
            "addon:test.tampered",
            "https://addon.example/manifest.json",
            tampered,
        )
        .await
        .unwrap();
    let (ok, refused) = installed(&db).await.unwrap();
    assert!(ok.is_empty());
    assert_eq!(refused, ["addon:test.tampered"]);
}
