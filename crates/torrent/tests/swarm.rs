//! The engine against a real swarm on loopback (see `common`).

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use rand::{Rng, SeedableRng};
use sinephile_torrent::{SeedingPolicy, State, StreamServer, Wanted};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

const LAYOUT: &[(&str, usize)] = &[
    ("Test.Film.1922.mkv", 12 * MIB + 12_345),
    ("Sample/test.film.sample.mkv", 2 * MIB),
    ("readme.txt", 900),
];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_leecher_gets_every_byte_and_finds_the_feature() {
    let s = swarm(LAYOUT, None).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;

    let main = leech.main_file(&id, Wanted::default()).unwrap();
    assert_eq!(
        main.path, "Test.Film.1922.mkv",
        "not the sample, not the readme"
    );

    wait_finished(&leech, &id, Duration::from_secs(60)).await;
    for (rel, bytes) in &s.files {
        let file = leech
            .files(&id)
            .unwrap()
            .into_iter()
            .find(|f| &f.path == rel)
            .unwrap();
        let on_disk = std::fs::read(leech.file_path(&id, &file).unwrap()).unwrap();
        assert!(on_disk == *bytes, "{rel} differs from the source");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seeding_stops_at_completion_by_default() {
    let s = swarm(LAYOUT, None).await;
    let (a, b) = (temp(), temp());
    let (stopper, id1) = s.leecher(a.path(), SeedingPolicy::StopAtCompletion).await;
    let (keeper, id2) = s.leecher(b.path(), SeedingPolicy::KeepSeeding).await;
    wait_finished(&stopper, &id1, Duration::from_secs(60)).await;
    wait_finished(&keeper, &id2, Duration::from_secs(60)).await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(
        stopper.status(&id1).unwrap().state,
        State::Paused,
        "still seeding"
    );
    assert_eq!(
        keeper.status(&id2).unwrap().state,
        State::Live,
        "the user chose to keep seeding"
    );
}

/// GET through the streaming server, with an optional Range header.
async fn get(url: &str, range: Option<&str>) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let rest = url.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_once('/').unwrap();
    let mut sock = tokio::net::TcpStream::connect(host).await.unwrap();
    let mut req = format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\n");
    if let Some(r) = range {
        req.push_str(&format!("Range: {r}\r\n"));
    }
    req.push_str("\r\n");
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).await.unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    (status, headers, raw[split + 4..].to_vec())
}

fn header<'a>(h: &'a [(String, String)], k: &str) -> Option<&'a str> {
    h.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_stream_is_byte_identical_to_the_source_whole_and_in_ranges() {
    // Phase 7 E5: "Range requests are byte-correct — verified by streaming a file and
    // comparing its hash to a full download." Compared byte for byte, which is stricter.
    let s = swarm(LAYOUT, None).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;
    let leech = Arc::new(leech);
    let file = leech.main_file(&id, Wanted::default()).unwrap();
    let server = StreamServer::start().await.unwrap();
    let url = server.publish("film", leech.stream_source(&id, &file).unwrap());
    let source = s.bytes("Test.Film.1922.mkv");
    let len = source.len();

    // Ranges first, while the torrent is still downloading: these come from the live
    // stream, out of order, which is exactly what a seeking player does.
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    for _ in 0..20 {
        let a = rng.gen_range(0..len - 1);
        let b = (a + rng.gen_range(1..300_000)).min(len - 1);
        let (status, h, body) = get(&url, Some(&format!("bytes={a}-{b}"))).await;
        assert_eq!(status, 206);
        assert_eq!(
            header(&h, "content-range"),
            Some(format!("bytes {a}-{b}/{len}").as_str())
        );
        assert!(body == source[a..=b], "range {a}-{b} differs");
    }
    let (status, _, tail) = get(&url, Some("bytes=-1000")).await;
    assert_eq!((status, tail.as_slice()), (206, &source[len - 1000..]));

    let (status, h, whole) = get(&url, None).await;
    assert_eq!(status, 200);
    assert_eq!(header(&h, "content-type"), Some("video/x-matroska"));
    assert_eq!(header(&h, "accept-ranges"), Some("bytes"));
    assert!(whole == source, "the full stream differs from the source");

    let (status, h, _) = get(&url, Some(&format!("bytes={len}-"))).await;
    assert_eq!(status, 416);
    assert_eq!(
        header(&h, "content-range"),
        Some(format!("bytes */{len}").as_str())
    );

    // The token is the key: the same path without it is not found.
    let bad = url.replace(&url.split('/').nth(3).unwrap().to_string(), "wrong-token");
    assert_eq!(get(&bad, None).await.0, 404);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_seek_into_an_unbuffered_region_is_served_quickly() {
    // Phase 7 E2's mechanism, on loopback: the seek moves librqbit's lookahead window.
    let s = swarm(&[("Test.Film.1922.mkv", 40 * MIB)], Some(8 * MIB as u32)).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;
    let mut stream = leech.stream(&id, 0).await.unwrap();
    let mut buf = vec![0u8; MIB];
    stream.read_exact(&mut buf).await.unwrap();
    let source = s.bytes("Test.Film.1922.mkv");
    let at = 32 * MIB as u64;
    let start = Instant::now();
    stream.seek(std::io::SeekFrom::Start(at)).await.unwrap();
    stream.read_exact(&mut buf).await.unwrap();
    let took = start.elapsed();
    assert!(
        buf == source[at as usize..at as usize + MIB],
        "wrong bytes after the seek"
    );
    assert!(took < Duration::from_secs(5), "seek took {took:?}");
    println!("seek to 80% and read 1 MiB: {took:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dead_swarm_fails_fast_with_a_clear_error() {
    // Phase 7 E4. The only "peer" is a port nobody listens on.
    let s = swarm(LAYOUT, None).await;
    let dead = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    let dir = temp();
    let mut config = offline_config(dir.path());
    config.dead_swarm_after = std::time::Duration::from_secs(3);
    let engine = Arc::new(sinephile_torrent::Engine::start(config).await.unwrap());
    let id = engine
        .add(
            sinephile_torrent::Source::File(s.torrent.clone()),
            sinephile_torrent::AddOptions {
                initial_peers: vec![dead],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let file = engine.main_file(&id, Wanted::default()).unwrap();
    let server = StreamServer::start().await.unwrap();
    let url = server.publish("film", engine.stream_source(&id, &file).unwrap());
    let start = Instant::now();
    let err = engine.wait_for_peers(&id).await.unwrap_err();
    let took = start.elapsed();
    assert!(err.to_string().contains("nobody is sharing this"), "{err}");
    assert!(took < Duration::from_secs(5), "took {took:?}");
    // Through the server the request ends rather than hanging — and says why, with a
    // 503, rather than promising a 206 whose body never arrives.
    let ended = tokio::time::timeout(Duration::from_secs(10), get(&url, Some("bytes=0-99"))).await;
    let (status, _, body) = ended.expect("the streaming request hung on a dead swarm");
    assert_eq!((status, body.len()), (503, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_download_survives_a_restart() {
    // Subtask 7.7: stop mid-download, start again from the same data directory.
    let s = swarm(&[("Test.Film.1922.mkv", 24 * MIB)], Some(4 * MIB as u32)).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let before = leech.status(&id).unwrap();
    assert!(!before.finished, "too fast to test a restart: {before:?}");
    leech.stop().await;
    // The record of which pieces are done is on disk the moment stop returns. librqbit
    // writes it from a background task, every 16 MiB and when a torrent is dropped; the
    // 10 MiB here never reached the first, so the restart below depended on winning a race
    // with the second (D83: lost twice on CI).
    let saved: u32 = std::fs::read_dir(dir.path().join("torrent-session"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "bitv"))
        .map(|e| {
            std::fs::read(e.path())
                .unwrap()
                .iter()
                .map(|b| b.count_ones())
                .sum::<u32>()
        })
        .sum();
    assert!(
        saved > 0,
        "no finished piece was on disk when stop returned"
    );

    let mut config = offline_config(dir.path());
    config.seeding = SeedingPolicy::KeepSeeding;
    let again = sinephile_torrent::Engine::start(config).await.unwrap();
    let listed = again.list();
    assert_eq!(listed.len(), 1, "the torrent was not remembered");
    assert_eq!(listed[0].id, id);
    // Progress came back from disk (fastresume), not from zero.
    again.wait_ready(&id).await.unwrap();
    let resumed = again.status(&id).unwrap();
    assert!(
        resumed.progress_bytes >= before.progress_bytes && resumed.progress_bytes > 0,
        "progress lost: {} before the stop, {} after ({resumed:?})",
        before.progress_bytes,
        resumed.progress_bytes
    );
    // With DHT and trackers off nothing can find the seeder again, which in real use
    // they would. Stand in for them: forget the torrent but keep its files, and add it
    // back with the seeder as a peer. The kept pieces are re-checked, not re-fetched.
    again.remove(&id, false).await.unwrap();
    let readded = again
        .add(
            sinephile_torrent::Source::File(s.torrent.clone()),
            sinephile_torrent::AddOptions {
                initial_peers: vec![s.peer],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(readded, id);
    assert!(
        again.status(&id).unwrap().progress_bytes >= before.progress_bytes,
        "the kept pieces were thrown away"
    );
    wait_finished(&again, &id, Duration::from_secs(60)).await;
    let file = again.files(&id).unwrap().remove(0);
    let on_disk = std::fs::read(again.file_path(&id, &file).unwrap()).unwrap();
    assert!(
        on_disk == s.bytes("Test.Film.1922.mkv"),
        "the resumed file differs"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn prepare_turns_a_torrent_into_a_playable_address() {
    let s = swarm(LAYOUT, None).await;
    let dir = temp();
    let mut config = offline_config(dir.path());
    config.seeding = SeedingPolicy::KeepSeeding;
    let engine = Arc::new(sinephile_torrent::Engine::start(config).await.unwrap());
    let server = StreamServer::start().await.unwrap();
    let prepared = sinephile_torrent::play::prepare(
        &engine,
        &server,
        sinephile_torrent::Source::File(s.torrent.clone()),
        None,
        Wanted::default(),
        sinephile_torrent::AddOptions {
            initial_peers: vec![s.peer],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(prepared.file.path, "Test.Film.1922.mkv");
    let (status, _, body) = get(&prepared.url, Some("bytes=0-65535")).await;
    assert_eq!(status, 206);
    assert!(body == s.bytes("Test.Film.1922.mkv")[..65536]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_resolved_torrent_the_engine_holds_plays_without_being_added_again() {
    use sinephile_torrent::play::{target, Target};
    use source_protocol::PlayableStream;
    let s = swarm(LAYOUT, None).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;
    let leech = Arc::new(leech);
    let server = StreamServer::start().await.unwrap();
    // DHT and trackers are off, so if this went to the network for metadata it would
    // never return: holding the torrent already is the only way it can succeed.
    let stream = PlayableStream::Torrent {
        info_hash: id.to_uppercase(),
        file_index: None,
        trackers: vec![],
    };
    let t = tokio::time::timeout(
        Duration::from_secs(10),
        target(&leech, &server, &stream, Wanted::default()),
    )
    .await
    .expect("went looking for metadata")
    .unwrap();
    let Target::Url(url) = t else {
        panic!("not a url")
    };
    let (status, _, body) = get(&url, Some("bytes=0-1023")).await;
    assert_eq!(
        (status, body.as_slice()),
        (206, &s.bytes("Test.Film.1922.mkv")[..1024])
    );
    let http = PlayableStream::Http {
        url: "https://films.example/a.mp4".into(),
        content_type: None,
        size_bytes: None,
    };
    assert_eq!(
        target(&leech, &server, &http, Wanted::default())
            .await
            .unwrap(),
        Target::Url("https://films.example/a.mp4".into())
    );
}

/// Found in Phase 13 by the advisor's trials: the reported download rate was 8.4 times too
/// low (librqbit's "mbps" is mebibytes per second, read as megabits). The rate the engine
/// reports must match the rate the bytes actually arrive at.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reported_rate_is_the_rate_bytes_arrive_at() {
    let s = swarm(&[("Test.Film.1922.mkv", 48 * MIB)], Some(4 * MIB as u32)).await;
    let dir = temp();
    let (leech, id) = s.leecher(dir.path(), SeedingPolicy::KeepSeeding).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let a = leech.status(&id).unwrap();
    let t = std::time::Instant::now();
    let mut reported = Vec::new();
    for _ in 0..8 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        reported.push(leech.status(&id).unwrap().download_bps);
    }
    let b = leech.status(&id).unwrap();
    assert!(!b.finished, "too fast to measure");
    let actual = (b.progress_bytes - a.progress_bytes) as f64 / t.elapsed().as_secs_f64();
    let shown = reported.iter().sum::<f64>() / reported.len() as f64;
    println!(
        "bytes arrived at {:.2} MiB/s; the engine reported {:.2} MiB/s",
        actual / MIB as f64,
        shown / MIB as f64
    );
    assert!(
        (shown - actual).abs() <= actual * 0.35,
        "reported {:.2} MiB/s, actual {:.2} MiB/s",
        shown / MIB as f64,
        actual / MIB as f64
    );
}
