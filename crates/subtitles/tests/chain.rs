//! The chain against a fake network: what it asks, and what it never asks (Phase 10 E3,
//! E4). Every address is a reserved test domain except the provider's own, which the
//! client builds.

use std::sync::Arc;

use sinephile_metadata_api::{FakeTransport, Response, Transport};
use sinephile_persistence::repositories::SubtitleRepository;
use sinephile_persistence::Db;
use sinephile_subtitles::fetch::{fetch, offers, pick, take};
use sinephile_subtitles::opensubtitles::{OpenSubtitles, FREE_DAILY};
use sinephile_subtitles::{Found, Origin};

const KEY: &str = "Ab0123456789cdEF0123456789abcd01";
const DAY: &str = "2026-11-06";

fn en() -> Vec<String> {
    vec!["en".into()]
}

fn search(file_id: i64, lang: &str, hash_match: bool) -> Response {
    Response::new(
        200,
        format!(
            r#"{{"data":[{{"attributes":{{"language":"{lang}","download_count":10,"moviehash_match":{hash_match},"files":[{{"file_id":{file_id}}}]}}}}]}}"#
        ),
    )
}

/// One search, one download answer, one file — pushed in reverse: the fake is a stack.
fn one_download(fake: &FakeTransport, file_id: i64, remaining: i64) {
    fake.push(Response::new(
        200,
        "1\n00:00:01,000 --> 00:00:02,000\nHello\n",
    ));
    fake.push(Response::new(
        200,
        format!(r#"{{"link":"https://dl.example.test/{file_id}.srt","remaining":{remaining}}}"#),
    ));
    fake.push(search(file_id, "en", true));
}

struct Setup {
    db: Db,
    fake: Arc<FakeTransport>,
    os: OpenSubtitles,
    dir: tempfile::TempDir,
}

async fn setup() -> Setup {
    let fake = Arc::new(FakeTransport::new());
    let os = OpenSubtitles::new(fake.clone() as Arc<dyn Transport>, KEY.into());
    Setup {
        db: Db::in_memory().await.unwrap(),
        fake,
        os,
        dir: tempfile::tempdir().unwrap(),
    }
}

#[tokio::test]
async fn an_embedded_track_is_used_without_a_single_request() {
    let s = setup().await;
    let embedded = Found {
        origin: Origin::Embedded { track_id: 3 },
        language: Some("en".into()),
        forced: false,
    };
    let got = pick(
        &s.db,
        Some(&s.os),
        s.dir.path(),
        Some("8e245d9679d31e12"),
        vec![embedded.clone()],
        &en(),
        DAY,
    )
    .await
    .unwrap();
    assert_eq!(got, Some(embedded));
    assert_eq!(
        s.fake.request_count(),
        0,
        "no network for an embedded track"
    );
}

#[tokio::test]
async fn a_download_is_asked_for_properly_and_cached() {
    let s = setup().await;
    one_download(&s.fake, 1234, 4);
    let got = fetch(
        &s.db,
        &s.os,
        s.dir.path(),
        "8E245D9679D31E12",
        &["fr".into(), "en".into()],
        DAY,
    )
    .await
    .unwrap()
    .expect("a subtitle");
    let requests = s.fake.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[0]
            .url
            .ends_with("/subtitles?languages=en,fr&moviehash=8e245d9679d31e12"),
        "{}",
        requests[0].url
    );
    assert!(requests[0]
        .headers
        .iter()
        .any(|(k, v)| k == "Api-Key" && v == KEY));
    assert_eq!(requests[1].body.as_deref(), Some(r#"{"file_id":1234}"#));
    assert!(
        requests[2].headers.iter().all(|(k, _)| k != "Api-Key"),
        "the file link gets no key"
    );
    let Origin::Downloaded { relative_path, .. } = &got.origin else {
        panic!("{got:?}")
    };
    assert!(s.dir.path().join(relative_path).is_file());
    // What the provider said is left is kept.
    let q = SubtitleRepository::new(&s.db)
        .quota("opensubtitles", DAY)
        .await
        .unwrap();
    assert_eq!((q.used, q.server_remaining), (1, Some(4)));

    // The same file and language again: from the cache, no request.
    let again = fetch(&s.db, &s.os, s.dir.path(), "8E245D9679D31E12", &en(), DAY)
        .await
        .unwrap();
    assert_eq!(again, Some(got));
    assert_eq!(s.fake.request_count(), 3);
}

#[tokio::test]
async fn the_daily_quota_is_never_exceeded() {
    // Phase 10 E4. Every download is reserved before it is asked for; once the budget is
    // spent, nothing more is even searched for.
    let s = setup().await;
    for i in 0..FREE_DAILY {
        one_download(&s.fake, 100 + i, 99);
        let hash = format!("{:016x}", i + 1);
        assert!(fetch(&s.db, &s.os, s.dir.path(), &hash, &en(), DAY)
            .await
            .unwrap()
            .is_some());
    }
    let before = s.fake.request_count();
    let more = fetch(&s.db, &s.os, s.dir.path(), "00000000000000ff", &en(), DAY)
        .await
        .unwrap();
    assert_eq!(more, None);
    assert_eq!(
        s.fake.request_count(),
        before,
        "no request once the budget is spent"
    );
    assert_eq!(
        SubtitleRepository::new(&s.db)
            .quota("opensubtitles", DAY)
            .await
            .unwrap()
            .used,
        FREE_DAILY
    );
    // Tomorrow is a new day.
    one_download(&s.fake, 999, 4);
    assert!(fetch(
        &s.db,
        &s.os,
        s.dir.path(),
        "00000000000000ff",
        &en(),
        "2026-11-07"
    )
    .await
    .unwrap()
    .is_some());
}

#[tokio::test]
async fn the_providers_own_count_wins_when_it_is_lower() {
    let s = setup().await;
    one_download(&s.fake, 1, 0); // "0 left"
    assert!(
        fetch(&s.db, &s.os, s.dir.path(), "0000000000000001", &en(), DAY)
            .await
            .unwrap()
            .is_some()
    );
    let before = s.fake.request_count();
    assert_eq!(
        fetch(&s.db, &s.os, s.dir.path(), "0000000000000002", &en(), DAY)
            .await
            .unwrap(),
        None
    );
    assert_eq!(s.fake.request_count(), before);
}

#[tokio::test]
async fn a_failed_attempt_gives_its_reservation_back() {
    let s = setup().await;
    // Nothing in English: the search was free, so the reservation returns.
    s.fake.push(search(5, "de", true));
    assert_eq!(
        fetch(&s.db, &s.os, s.dir.path(), "0000000000000003", &en(), DAY)
            .await
            .unwrap(),
        None
    );
    // A refused key: an error, and the reservation returns.
    s.fake.push(Response::new(401, "{}"));
    assert!(
        fetch(&s.db, &s.os, s.dir.path(), "0000000000000004", &en(), DAY)
            .await
            .is_err()
    );
    assert_eq!(
        SubtitleRepository::new(&s.db)
            .quota("opensubtitles", DAY)
            .await
            .unwrap()
            .used,
        0
    );
}

#[tokio::test]
async fn the_best_offer_is_this_files_own_timing() {
    let s = setup().await;
    s.fake
        .push(Response::new(200, "1\n00:00:01,000 --> 00:00:02,000\nHi\n"));
    s.fake.push(Response::new(
        200,
        r#"{"link":"https://dl.example.test/b.srt","remaining":3}"#,
    ));
    s.fake.push(Response::new(
        200,
        r#"{"data":[
            {"attributes":{"language":"en","download_count":900,"moviehash_match":false,"files":[{"file_id":1}]}},
            {"attributes":{"language":"en","download_count":3,"moviehash_match":true,"files":[{"file_id":2}]}}
        ]}"#,
    ));
    fetch(&s.db, &s.os, s.dir.path(), "0000000000000005", &en(), DAY)
        .await
        .unwrap()
        .unwrap();
    let requests = s.fake.requests.lock().unwrap().clone();
    assert_eq!(
        requests[1].body.as_deref(),
        Some(r#"{"file_id":2}"#),
        "hash match beats popularity"
    );
}

#[tokio::test]
async fn the_viewer_chooses_among_the_offers_and_only_a_download_spends_the_budget() {
    // Phase 16: "download subtitles like in VLC" from the player. The search is free; the
    // chosen offer is downloaded within today's budget, and a failure gives it back.
    let s = setup().await;
    s.fake.push(Response::new(
        200,
        r#"{"data":[
          {"attributes":{"language":"de","download_count":900,"moviehash_match":true,"release":"Ein Film","files":[{"file_id":1}]}},
          {"attributes":{"language":"en","download_count":10,"moviehash_match":false,"release":"Film.2019.WEB","files":[{"file_id":2}]}},
          {"attributes":{"language":"en","download_count":5,"moviehash_match":true,"release":"Film.2019.BluRay","files":[{"file_id":3}]}}]}"#,
    ));
    let found = offers(&s.os, "0000000000000009", &en()).await.unwrap();
    let order: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|o| (o.file_id.as_str(), o.release.as_deref()))
        .collect();
    assert_eq!(
        order,
        [
            ("3", Some("Film.2019.BluRay")),
            ("2", Some("Film.2019.WEB")),
            ("1", Some("Ein Film"))
        ],
        "the preferred language first, this file's own timing first within it"
    );
    let quota = || async {
        SubtitleRepository::new(&s.db)
            .quota("opensubtitles", DAY)
            .await
            .unwrap()
            .used
    };
    assert_eq!(quota().await, 0, "searching spends nothing");

    // The viewer picks the WEB release, not the best: that one is downloaded.
    s.fake.push(Response::new(
        200,
        "1\n00:00:01,000 --> 00:00:02,000\nHello\n",
    ));
    s.fake.push(Response::new(
        200,
        r#"{"link":"https://dl.example.test/2.srt","remaining":7}"#,
    ));
    let got = take(
        &s.db,
        &s.os,
        s.dir.path(),
        "0000000000000009",
        &found[1],
        DAY,
    )
    .await
    .unwrap()
    .expect("downloaded");
    let Origin::Downloaded {
        file_id,
        relative_path,
        ..
    } = &got.origin
    else {
        panic!("{got:?}")
    };
    assert_eq!(file_id, "2");
    assert!(s.dir.path().join(relative_path).is_file());
    assert_eq!(quota().await, 1);

    // A refused download gives its reservation back.
    s.fake.push(Response::new(401, "{}"));
    assert!(take(
        &s.db,
        &s.os,
        s.dir.path(),
        "0000000000000009",
        &found[2],
        DAY
    )
    .await
    .is_err());
    assert_eq!(quota().await, 1);
}
