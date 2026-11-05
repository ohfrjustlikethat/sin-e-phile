//! The Internet Archive backend against RECORDED responses (`fixtures/internet-archive/`,
//! fetched 2026-11-05 and trimmed of thumbnails), and once — opt-in — against the
//! live service.
//!
//! The recorded half is what CI runs: it fixes the parsing and the rules about what
//! counts as a whole film to what the Archive actually returned, not to what its
//! documentation says it returns.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sinephile_metadata_api::transport::ResponseFuture;
use sinephile_metadata_api::{HttpTransport, Request, Response, Transport, TransportError};
use sinephile_sources::backends::archive::InternetArchiveBackend;
use source_protocol::{
    Health, MediaQuery, PlayableStream, Resolution, SourceBackend, SourceError, VideoCodec,
};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/internet-archive")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

/// Answers by URL, since the metadata lookups run concurrently and arrive in no fixed
/// order. Anything unrouted is a 404, as an unknown item would be.
struct Routes {
    routes: Vec<(&'static str, u16, String)>,
    asked: Mutex<Vec<String>>,
}

impl Routes {
    fn archive() -> Self {
        Self {
            routes: vec![
                ("advancedsearch.php", 200, fixture("search-nosferatu.json")),
                (
                    "/metadata/Nosferatu_most_complete_version_93_mins.",
                    200,
                    fixture("metadata-Nosferatu_most_complete_version_93_mins.json"),
                ),
                (
                    "/metadata/Nosferatu_DVD_quality",
                    200,
                    fixture("metadata-Nosferatu_DVD_quality.json"),
                ),
            ],
            asked: Mutex::new(Vec::new()),
        }
    }
}

impl Transport for Routes {
    fn send(&self, request: Request) -> ResponseFuture<'_> {
        self.asked.lock().unwrap().push(request.url.clone());
        let hit = self
            .routes
            .iter()
            .find(|(path, _, _)| request.url.contains(path))
            .map(|(_, status, body)| Response::new(*status, body.clone()))
            .unwrap_or_else(|| Response::new(404, "{}"));
        Box::pin(async move { Ok(hit) })
    }
}

#[tokio::test]
async fn finds_whole_films_from_recorded_responses() {
    let routes = Arc::new(Routes::archive());
    let backend = InternetArchiveBackend::new(routes.clone());
    let found = backend
        .find(&MediaQuery::film("Nosferatu", Some(1922)))
        .await
        .unwrap();

    // The most-downloaded item has one file each of DivX, Ogg and 512Kb MPEG4: three
    // whole films, tallest first. The DVD-quality item splits the film into five parts
    // in every format, so it yields NOTHING rather than a fifth of a film.
    let keys: Vec<&str> = found.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "Nosferatu_most_complete_version_93_mins./Nosferatu_1922_Symphony_of_Horror.avi",
            "Nosferatu_most_complete_version_93_mins./Nosferatu_1922_Symphony_of_Horror.ogv",
            "Nosferatu_most_complete_version_93_mins./Nosferatu_1922_Symphony_of_Horror_512kb.mp4",
        ]
    );
    let best = &found[0];
    assert_eq!(best.year, Some(1922));
    assert_eq!(best.quality.resolution, Resolution::Sd);
    assert_eq!(best.codec, VideoCodec::Mpeg4, "DivX is MPEG-4 Part 2");
    assert_eq!(best.size_bytes, Some(916_378_678));
    assert_eq!(best.health, Health::Direct);
    assert_eq!(
        best.provenance.page.as_deref(),
        Some("https://archive.org/details/Nosferatu_most_complete_version_93_mins.")
    );

    // 1929's "Nosferatu" is a different year; it was never looked up.
    let asked = routes.asked.lock().unwrap();
    assert!(
        !asked.iter().any(|u| u.contains("nosferatu_1929")),
        "{asked:?}"
    );
    assert!(
        asked.len() <= 5,
        "one search and at most four lookups, not {}",
        asked.len()
    );
}

#[tokio::test]
async fn resolve_builds_the_download_url_from_the_key_alone() {
    let backend = InternetArchiveBackend::new(Arc::new(Routes::archive()));
    let found = backend
        .find(&MediaQuery::film("Nosferatu", Some(1922)))
        .await
        .unwrap();
    let mut mp4 = found
        .iter()
        .find(|c| c.key.ends_with(".mp4"))
        .unwrap()
        .clone();
    // A forged locator is ignored: the URL comes from the checked key.
    mp4.locator = Some("https://elsewhere.example/evil.mp4".into());
    match backend.resolve(&mp4).await.unwrap() {
        PlayableStream::Http {
            url, content_type, ..
        } => {
            assert_eq!(url, "https://archive.org/download/Nosferatu_most_complete_version_93_mins./Nosferatu_1922_Symphony_of_Horror_512kb.mp4");
            assert_eq!(content_type.as_deref(), Some("video/mp4"));
        }
        other => panic!("{other:?}"),
    }
    mp4.key = "../../etc/passwd".into();
    assert!(matches!(
        backend.resolve(&mp4).await,
        Err(SourceError::NotFound(_))
    ));
}

#[tokio::test]
async fn an_archive_that_is_down_is_an_error_and_a_lost_lookup_costs_only_its_item() {
    struct Down;
    impl Transport for Down {
        fn send(&self, _r: Request) -> ResponseFuture<'_> {
            Box::pin(async { Err(TransportError::Network("refused".into())) })
        }
    }
    let backend = InternetArchiveBackend::new(Arc::new(Down));
    let err = backend
        .find(&MediaQuery::film("Nosferatu", None))
        .await
        .unwrap_err();
    assert!(matches!(err, SourceError::Unavailable(_)), "{err}");

    // Search answers, but one item's metadata is gone: the other item still counts.
    let mut routes = Routes::archive();
    routes
        .routes
        .retain(|(p, _, _)| !p.contains("most_complete"));
    let found = InternetArchiveBackend::new(Arc::new(routes))
        .find(&MediaQuery::film("Nosferatu", Some(1922)))
        .await
        .unwrap();
    assert!(
        found.is_empty(),
        "the remaining item is multi-part, so nothing — but no error"
    );
}

#[tokio::test]
async fn a_malformed_answer_is_an_invalid_response_not_a_panic() {
    let routes = Routes {
        routes: vec![("advancedsearch.php", 200, "<html>maintenance</html>".into())],
        asked: Mutex::new(Vec::new()),
    };
    let err = InternetArchiveBackend::new(Arc::new(routes))
        .find(&MediaQuery::film("Nosferatu", None))
        .await
        .unwrap_err();
    assert!(matches!(err, SourceError::InvalidResponse(_)), "{err}");
}

/// The live service. Not run by default or in CI — it needs the network and the
/// Archive's goodwill. `cargo test -p sinephile-sources --test archive -- --ignored`
#[tokio::test]
#[ignore]
async fn live_the_archive_returns_a_reachable_whole_film() {
    let http = Arc::new(HttpTransport::new());
    let backend = InternetArchiveBackend::new(http.clone());
    let found = backend
        .find(&MediaQuery::film("Night of the Living Dead", Some(1968)))
        .await
        .unwrap();
    assert!(!found.is_empty(), "no candidates");
    let PlayableStream::Http { url, .. } = backend.resolve(&found[0]).await.unwrap() else {
        panic!("not http")
    };
    let head = http.send(Request::head(url.clone())).await.unwrap();
    // The download host redirects to a storage node; 200 after following is success.
    assert_eq!(head.status, 200, "{url}");
    println!("{} candidates; first: {url}", found.len());
}
