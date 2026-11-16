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
//!
//! A Stremio add-on (ADR-0047) is asked by IMDb id: `SINEPHILE_ADDON_IMDB`, Nosferatu's
//! (`tt0013442`) unless set; a series is asked for its season 1, episode 1.

use std::sync::Arc;

use sinephile_metadata_api::HttpTransport;
use sinephile_sources::addons::{describe, fetch_manifest};
use sinephile_sources::backends::addon::AddonBackend;
use sinephile_sources::backends::stremio::StremioBackend;
use source_protocol::{AddonManifest, MediaKind, MediaQuery, SourceBackend, SourceError};

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

    let backend: Box<dyn SourceBackend> = match &manifest {
        AddonManifest::Native(m) => Box::new(AddonBackend::new(m.clone(), http)),
        AddonManifest::Stremio(m) => Box::new(StremioBackend::new(m.clone(), http)),
    };
    let kind = if manifest.media().contains(&MediaKind::Film) {
        MediaKind::Film
    } else {
        MediaKind::Episode
    };
    let mut query = MediaQuery::film(title.clone(), None);
    query.kind = kind;
    query.ids.imdb =
        Some(std::env::var("SINEPHILE_ADDON_IMDB").unwrap_or_else(|_| "tt0013442".into()));
    if kind == MediaKind::Episode {
        (query.season, query.episode) = (Some(1), Some(1));
    }
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

    // A Stremio candidate's key IS its stream: there is no key to forge at the add-on.
    if matches!(manifest, AddonManifest::Stremio(_)) {
        return;
    }
    let mut forged = found[0].clone();
    forged.key = "a key this addon never issued".into();
    match backend.resolve(&forged).await {
        Err(SourceError::NotFound(_)) => println!("unknown key ok: refused with not-found"),
        other => panic!("an unknown key should be a 404 (spec §4), got {other:?}"),
    }
}
