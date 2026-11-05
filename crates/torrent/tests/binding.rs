//! Phase 7 E7, as far as code can take it: "Binding to a specific network interface
//! works, verified by disabling that adapter mid-download and confirming traffic stops
//! rather than falling back."
//!
//! librqbit could not do this on Windows at all; `crates/vendor/librqbit-dualstack-
//! sockets` adds it (IP_UNICAST_IF). These tests check the two halves that do not need
//! someone to pull a cable: a bound socket uses ONLY its adapter, and a bad adapter name
//! is refused at start rather than silently ignored. The live half — a VPN dropped
//! mid-download — is a manual check, recorded in docs/eval-results.md.

use std::time::Duration;

use librqbit_dualstack_sockets::{tcp_connect, BindDevice, ConnectOpts};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use sinephile_torrent::{Engine, EngineConfig, TorrentError};

fn loopback_name() -> String {
    NetworkInterface::show()
        .unwrap()
        .into_iter()
        .find(|n| n.addr.iter().any(|a| a.ip().is_loopback()))
        .map(|n| n.name)
        .expect("a loopback adapter")
}

#[tokio::test]
async fn an_adapter_that_does_not_exist_stops_the_engine_starting() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = EngineConfig::new(dir.path());
    c.dht = false;
    c.bind_interface = Some("no adapter has this name".into());
    match Engine::start(c).await {
        Err(TorrentError::Start(m)) => assert!(m.contains("bind device"), "{m}"),
        Err(e) => panic!("wrong error: {e}"),
        Ok(_) => panic!("started with a bind adapter that does not exist — traffic would leak"),
    }
}

#[tokio::test]
async fn bound_to_loopback_local_traffic_still_flows() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let bd = BindDevice::new_from_name(&loopback_name()).unwrap();
    let conn = tokio::time::timeout(
        Duration::from_secs(3),
        tcp_connect(
            addr,
            ConnectOpts {
                bind_device: Some(&bd),
                ..Default::default()
            },
        ),
    )
    .await
    .expect("timed out");
    assert!(
        conn.is_ok(),
        "binding broke traffic on its own adapter: {conn:?}"
    );
}

/// Needs the internet: `BINDING_TARGET=<ip:port> cargo test -p sinephile-torrent
/// --test binding -- --ignored`. Bound to loopback, an outside address must be
/// unreachable — the socket may not leave by another adapter — while unbound it connects.
#[tokio::test]
#[ignore]
async fn bound_to_one_adapter_traffic_never_leaves_by_another() {
    let target = std::env::var("BINDING_TARGET")
        .expect("BINDING_TARGET=ip:port")
        .parse()
        .unwrap();
    let unbound = tokio::time::timeout(
        Duration::from_secs(5),
        tcp_connect(target, ConnectOpts::default()),
    )
    .await;
    assert!(
        matches!(unbound, Ok(Ok(_))),
        "the target is not reachable at all: {unbound:?}"
    );
    let bd = BindDevice::new_from_name(&loopback_name()).unwrap();
    let bound = tokio::time::timeout(
        Duration::from_secs(5),
        tcp_connect(
            target,
            ConnectOpts {
                bind_device: Some(&bd),
                ..Default::default()
            },
        ),
    )
    .await;
    assert!(
        !matches!(bound, Ok(Ok(_))),
        "a socket bound to loopback reached the internet: it fell back"
    );
}
