//! The addon conformance check — `docs/specs/addon-protocol.md` §8.
//!
//! Runs the app's own addon client, through the app's own untrusted transport, against
//! an addon somewhere else: fetch and check its manifest exactly as the install screen
//! does, run a find, resolve the first candidate, and check a key it never issued is
//! refused. Ignored by default because it needs an addon running:
//!
//! ```text
//! SINEPHILE_ADDON=http://127.0.0.1:8765/manifest.json \
//! SINEPHILE_ADDON_QUERY="Nosferatu" \
//!   cargo test -p sinephile-sources --test conformance -- --ignored --nocapture
//! ```

use std::sync::Arc;

use sinephile_metadata_api::HttpTransport;
use sinephile_sources::addons::{describe, fetch_manifest};
use sinephile_sources::backends::addon::AddonBackend;
use source_protocol::{MediaKind, MediaQuery, SourceBackend, SourceError};

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn conformance() {
    let address = std::env::var("SINEPHILE_ADDON")
        .expect("set SINEPHILE_ADDON to the addon's manifest address");
    let title = std::env::var("SINEPHILE_ADDON_QUERY").unwrap_or_else(|_| "Nosferatu".into());
    let http = Arc::new(HttpTransport::untrusted());

    let (url, manifest) = fetch_manifest(&*http, &address)
        .await
        .unwrap_or_else(|e| panic!("manifest refused: {e}"));
    let shown = describe(&url, &manifest, &[]);
    println!(
        "manifest ok: {} {} — {} {}",
        shown.name,
        shown.version,
        shown.finds,
        shown.delivers.join(" ")
    );

    let backend = AddonBackend::new(manifest.clone(), http);
    let kind = if manifest.media.contains(&MediaKind::Film) {
        MediaKind::Film
    } else {
        MediaKind::Episode
    };
    let mut query = MediaQuery::film(title.clone(), None);
    query.kind = kind;
    let found = backend
        .find(&query)
        .await
        .unwrap_or_else(|e| panic!("find failed: {e}"));
    println!("find ok: {} candidate(s) for {title:?}", found.len());
    assert!(
        !found.is_empty(),
        "the addon found nothing for {title:?}; try SINEPHILE_ADDON_QUERY"
    );

    let stream = backend
        .resolve(&found[0])
        .await
        .unwrap_or_else(|e| panic!("resolve failed: {e}"));
    println!("resolve ok: {stream:?}");

    let mut forged = found[0].clone();
    forged.key = "a key this addon never issued".into();
    match backend.resolve(&forged).await {
        Err(SourceError::NotFound(_)) => println!("unknown key ok: refused with not-found"),
        other => panic!("an unknown key should be a 404 (spec §4), got {other:?}"),
    }
}
