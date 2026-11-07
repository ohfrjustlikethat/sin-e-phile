//! The qBittorrent backend against a scripted stand-in of its Web API (no qBittorrent is
//! installed here; one real round trip is an author check, Phase 13 E2). The stand-in
//! records every request, so what is sent is asserted as much as what is read.

use std::path::Path;
use std::sync::Arc;

use sinephile_downloads::qbittorrent::Qbittorrent;
use sinephile_downloads::Backend;
use sinephile_metadata_api::transport::{FakeTransport, Method, Response};
use sinephile_persistence::repositories::{Download, DownloadState};

const BASE: &str = "http://qbit.test:8080";

fn hash() -> String {
    "a".repeat(40)
}

fn download(source: &str) -> Download {
    Download {
        id: 1,
        media_item_id: None,
        title: "Heat".into(),
        source: source.into(),
        torrent: None,
        backend: "qbittorrent".into(),
        backend_id: None,
        priority: 0,
        state: DownloadState::Queued,
        total_bytes: None,
        done_bytes: 0,
        limit_kib: Some(512),
        not_before: None,
        reason: None,
        error: None,
        file_path: None,
        added_at: String::new(),
        finished_at: None,
    }
}

fn client(fake: &Arc<FakeTransport>, login: bool) -> Qbittorrent {
    let (user, pass) = if login {
        (Some("admin".to_string()), Some("a secret".to_string()))
    } else {
        (None, None)
    };
    Qbittorrent::new(fake.clone(), BASE, user, pass)
}

#[tokio::test]
async fn not_running_is_unavailable_and_running_is_available() {
    let fake = Arc::new(FakeTransport::new());
    assert!(!client(&fake, false).available().await, "nothing answers");
    fake.push(Response::new(200, "v5.0.1"));
    assert!(client(&fake, false).available().await);
    assert!(fake
        .last_request()
        .unwrap()
        .url
        .ends_with("/api/v2/app/version"));
}

#[tokio::test]
async fn a_403_logs_in_once_and_the_retry_carries_the_cookie() {
    let fake = Arc::new(FakeTransport::new());
    // Last pushed, first answered: 403, then the login, then the retried call.
    fake.push(Response::new(200, "v5.0.1"));
    fake.push(Response::new(200, "Ok.").with_header("set-cookie", "SID=abc123; HttpOnly; path=/"));
    fake.push(Response::new(403, "Forbidden"));
    assert!(client(&fake, true).available().await);
    let requests = fake.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].url.ends_with("/api/v2/auth/login"));
    assert_eq!(
        requests[1].body.as_deref(),
        Some("username=admin&password=a+secret")
    );
    let cookie = requests[2]
        .headers
        .iter()
        .find(|(k, _)| k == "cookie")
        .map(|(_, v)| v.as_str());
    assert_eq!(cookie, Some("SID=abc123"));
}

#[tokio::test]
async fn adding_sends_the_hash_the_folder_and_the_limit() {
    let fake = Arc::new(FakeTransport::new());
    fake.push(Response::new(200, "Ok."));
    let qb = client(&fake, false);
    let d = download(&format!("infohash:{}", hash()));
    assert!(qb.accepts(&d));
    let id = qb.add(&d, Path::new(r"D:\Downloads")).await.unwrap();
    assert_eq!(id, hash());
    let r = fake.last_request().unwrap();
    assert_eq!(r.method, Method::Post);
    assert!(r.url.ends_with("/api/v2/torrents/add"));
    assert_eq!(
        r.body.as_deref(),
        Some(format!("urls={}&savepath=D%3A%5CDownloads&dlLimit=524288", hash()).as_str())
    );
    // A .torrent file's bytes are not qBittorrent's to take.
    let mut file = download("a torrent file");
    file.torrent = Some(vec![1, 2, 3]);
    assert!(!qb.accepts(&file));
}

#[tokio::test]
async fn progress_is_read_from_its_info_and_an_up_state_is_finished() {
    let fake = Arc::new(FakeTransport::new());
    let info = |state: &str, progress: f64| {
        Response::new(
            200,
            format!(
                r#"[{{"hash":"{}","progress":{progress},"total_size":1000,"completed":{},"dlspeed":2048,"num_seeds":3,"num_leechs":1,"state":"{state}","save_path":"D:\\Downloads"}}]"#,
                hash(),
                (progress * 1000.0) as i64
            ),
        )
    };
    fake.push(info("stalledUP", 1.0));
    fake.push(info("downloading", 0.4));
    let qb = client(&fake, false);
    let p = qb.progress(&hash()).await.unwrap();
    assert_eq!(
        (p.done, p.total, p.peers, p.finished),
        (400, 1000, 4, false)
    );
    assert_eq!(p.speed, 2048.0);
    assert!(qb.progress(&hash()).await.unwrap().finished);
}

#[tokio::test]
async fn pause_tries_the_new_name_first_then_the_old() {
    let fake = Arc::new(FakeTransport::new());
    fake.push(Response::new(200, ""));
    fake.push(Response::new(404, "Not Found"));
    client(&fake, false).pause(&hash()).await.unwrap();
    let requests = fake.requests.lock().unwrap().clone();
    assert!(
        requests[0].url.ends_with("/api/v2/torrents/stop"),
        "qBittorrent 5"
    );
    assert!(
        requests[1].url.ends_with("/api/v2/torrents/pause"),
        "earlier versions"
    );
}

#[tokio::test]
async fn the_video_file_is_chosen_from_its_file_list() {
    let fake = Arc::new(FakeTransport::new());
    fake.push(Response::new(
        200,
        r#"[{"name":"Heat.1995/Sample/sample.mkv","size":10000},{"name":"Heat.1995/Heat.1995.mkv","size":9000000},{"name":"Heat.1995/Heat.nfo","size":100}]"#,
    ));
    fake.push(Response::new(
        200,
        r#"[{"progress":1,"state":"uploading","save_path":"D:\\Downloads"}]"#,
    ));
    let file = client(&fake, false).video_file(&hash()).await.unwrap();
    assert_eq!(
        file,
        Path::new(r"D:\Downloads").join("Heat.1995/Heat.1995.mkv")
    );
}
