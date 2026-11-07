//! The user's own qBittorrent, over its Web API (ADR-0029): detected, never bundled, never
//! required. **A transport only**: it is told what to fetch and asked how far it has got.
//! Its search plugins, RSS feeds and tracker lists are never read, and `tools/guard`
//! rejects any code that would.
//!
//! The endpoints used, and nothing else: `app/version`, `auth/login`, `torrents/add`,
//! `torrents/info`, `torrents/files`, `torrents/stop` (`pause` before qBittorrent 5),
//! `torrents/start` (`resume`), `torrents/delete`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Deserialize;
use sinephile_metadata_api::transport::{Method, Request, Response};
use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::Download;
use sinephile_torrent::{TorrentFile, Wanted};

use crate::backend::{Backend, BackendError, Progress};

pub struct Qbittorrent {
    transport: Arc<dyn Transport>,
    /// "http://localhost:8080", no trailing slash.
    base: String,
    user: Option<String>,
    password: Option<String>,
    /// The session cookie after a login ("SID=…").
    cookie: Mutex<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct Info {
    #[serde(default)]
    progress: f64,
    #[serde(default)]
    total_size: i64,
    #[serde(default)]
    size: i64,
    #[serde(default)]
    completed: i64,
    #[serde(default)]
    dlspeed: f64,
    #[serde(default)]
    num_seeds: u32,
    #[serde(default)]
    num_leechs: u32,
    #[serde(default)]
    state: String,
    #[serde(default)]
    save_path: String,
}

#[derive(Debug, Deserialize)]
struct File {
    name: String,
    size: u64,
}

/// "a b&c" -> "a+b%26c": `application/x-www-form-urlencoded`.
fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

/// The info hash a download names, lower-case hex: from an `infohash:` source, or a
/// magnet's `btih` (hex, or base32 as some clients write it).
pub fn info_hash(d: &Download) -> Option<String> {
    if let Some(h) = d.source.strip_prefix("infohash:") {
        return valid_hex(h.split('|').next().unwrap_or_default());
    }
    let rest = d.source.split("btih:").nth(1)?;
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    match token.len() {
        40 => valid_hex(&token),
        32 => base32_to_hex(&token),
        _ => None,
    }
}

/// What `torrents/add` is given for a download: its magnet as the source gave it, or one
/// built here from the info hash and the trackers the source named at run time, so
/// qBittorrent finds the swarm as quickly as librqbit does. The hash is hex and needs no
/// escaping; each tracker is percent-encoded whole. The app holds no link of its own (§2.1).
pub fn link(d: &Download) -> Option<String> {
    if d.source.starts_with("magnet:") {
        return Some(d.source.clone());
    }
    let hash = info_hash(d)?;
    let trackers = match crate::backend::source_of(d).ok()? {
        sinephile_torrent::Source::InfoHash { trackers, .. } => trackers,
        _ => Vec::new(),
    };
    let mut query = format!("xt=urn:btih:{hash}");
    for t in &trackers {
        query.push_str("&tr=");
        query.extend(url::form_urlencoded::byte_serialize(t.as_bytes()));
    }
    let mut magnet = url::Url::parse("magnet:").ok()?;
    magnet.set_query(Some(&query));
    Some(magnet.into())
}

fn valid_hex(h: &str) -> Option<String> {
    (h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
}

/// RFC 4648 base32 (A–Z, 2–7) to hex: 32 characters, 20 bytes.
fn base32_to_hex(s: &str) -> Option<String> {
    let mut bits: u64 = 0;
    let mut count = 0;
    let mut out = Vec::with_capacity(20);
    for c in s.chars() {
        let v = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u64 - 'A' as u64,
            c @ '2'..='7' => c as u64 - '2' as u64 + 26,
            _ => return None,
        };
        bits = (bits << 5) | v;
        count += 5;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    (out.len() == 20).then(|| out.iter().map(|b| format!("{b:02x}")).collect())
}

impl Qbittorrent {
    pub fn new(
        transport: Arc<dyn Transport>,
        base: &str,
        user: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            transport,
            base: base.trim_end_matches('/').to_string(),
            user,
            password,
            cookie: Mutex::new(None),
        }
    }

    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        let mut r = Request::get(format!("{}/api/v2/{path}", self.base)).limit(8 * 1024 * 1024);
        r.method = method;
        if let Some(b) = body {
            r.body = Some(b);
            r = r.header("content-type", "application/x-www-form-urlencoded");
        }
        // qBittorrent refuses a request whose Referer does not match its own address.
        r = r.header("referer", self.base.clone());
        if let Some(c) = self.cookie.lock().ok().and_then(|c| c.clone()) {
            r = r.header("cookie", c);
        }
        r
    }

    async fn login(&self) -> Result<(), BackendError> {
        let (Some(user), Some(pass)) = (&self.user, &self.password) else {
            return Err(BackendError::Refused(
                "qBittorrent asks for a user name and password".into(),
            ));
        };
        let r = self
            .transport
            .send(self.request(
                Method::Post,
                "auth/login",
                Some(form(&[("username", user), ("password", pass)])),
            ))
            .await
            .map_err(|_| BackendError::Unavailable("qBittorrent"))?;
        if r.status != 200 || r.body.trim() != "Ok." {
            return Err(BackendError::Refused(
                "qBittorrent did not accept the user name and password".into(),
            ));
        }
        let sid = r
            .header("set-cookie")
            .and_then(|c| c.split(';').next())
            .map(str::to_string);
        if let Ok(mut c) = self.cookie.lock() {
            *c = sid;
        }
        Ok(())
    }

    /// One call, logging in once if asked to.
    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, BackendError> {
        let send = |b: Option<String>| self.transport.send(self.request(method, path, b));
        let mut r = send(body.clone())
            .await
            .map_err(|_| BackendError::Unavailable("qBittorrent"))?;
        if r.status == 403 {
            self.login().await?;
            r = send(body)
                .await
                .map_err(|_| BackendError::Unavailable("qBittorrent"))?;
        }
        Ok(r)
    }

    /// qBittorrent 5 renamed pause and resume to stop and start; try the new name first.
    async fn either(&self, new: &str, old: &str, hash: &str) -> Result<(), BackendError> {
        let body = form(&[("hashes", hash)]);
        let r = self
            .call(Method::Post, &format!("torrents/{new}"), Some(body.clone()))
            .await?;
        let r = if r.status == 404 {
            self.call(Method::Post, &format!("torrents/{old}"), Some(body))
                .await?
        } else {
            r
        };
        if r.status == 200 {
            Ok(())
        } else {
            Err(BackendError::Refused(format!(
                "qBittorrent answered {}",
                r.status
            )))
        }
    }

    async fn info(&self, hash: &str) -> Result<Option<Info>, BackendError> {
        let r = self
            .call(
                Method::Get,
                &format!("torrents/info?{}", form(&[("hashes", hash)])),
                None,
            )
            .await?;
        if r.status != 200 {
            return Err(BackendError::Refused(format!(
                "qBittorrent answered {}",
                r.status
            )));
        }
        let list: Vec<Info> = serde_json::from_str(&r.body).map_err(|e| {
            BackendError::Refused(format!("qBittorrent's answer did not parse: {e}"))
        })?;
        Ok(list.into_iter().next())
    }
}

/// Whether `torrents/add` took it. Before 5.2: 200 and "Ok." (or "Fails."). From 5.2:
/// 200, or 202 while it is still being fetched, with counts in JSON.
fn took(status: u16, body: &str) -> bool {
    if !(200..300).contains(&status) || body.trim() == "Fails." {
        return false;
    }
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(v) if v.is_object() => {
            let n = |k: &str| v[k].as_u64().unwrap_or(0);
            n("failure_count") == 0 || n("success_count") + n("pending_count") > 0
        }
        _ => true,
    }
}

/// Finished, by qBittorrent's states: everything after the download is an "UP" state.
fn finished(i: &Info) -> bool {
    i.progress >= 1.0 || i.state.ends_with("UP")
}

#[async_trait]
impl Backend for Qbittorrent {
    fn name(&self) -> &'static str {
        "qbittorrent"
    }

    async fn available(&self) -> bool {
        match self.call(Method::Get, "app/version", None).await {
            Ok(r) => r.status == 200,
            Err(_) => false,
        }
    }

    /// It is told what to fetch by link or hash; a `.torrent` file's bytes stay with
    /// librqbit (the transport carries text, and nothing here writes temp files for it).
    fn accepts(&self, d: &Download) -> bool {
        info_hash(d).is_some()
    }

    async fn holds(&self, id: &str) -> bool {
        matches!(self.info(id).await, Ok(Some(_)))
    }

    async fn add(&self, d: &Download, dir: &Path) -> Result<String, BackendError> {
        let hash = info_hash(d).ok_or_else(|| {
            BackendError::Refused(format!("cannot hand {} to qBittorrent", d.title))
        })?;
        let link = link(d).unwrap_or_else(|| hash.clone());
        let limit = d
            .limit_kib
            .map(|k| (k * 1024).to_string())
            .unwrap_or_default();
        // Laid out as librqbit lays it out, so a download files identically either way
        // (E2): its own folder named for the title, the torrent's files straight inside.
        let save = dir
            .join(crate::backend::folder_name(&d.title))
            .to_string_lossy()
            .into_owned();
        let mut fields = vec![
            ("urls", link.as_str()),
            ("savepath", save.as_str()),
            ("contentLayout", "NoSubfolder"),
            // The same, as qBittorrent before 4.3.2 asks for it.
            ("root_folder", "false"),
        ];
        if !limit.is_empty() {
            fields.push(("dlLimit", limit.as_str()));
        }
        let r = self
            .call(Method::Post, "torrents/add", Some(form(&fields)))
            .await?;
        if !took(r.status, &r.body) {
            return Err(BackendError::Refused(format!(
                "qBittorrent did not take {}",
                d.title
            )));
        }
        Ok(hash)
    }

    async fn progress(&self, id: &str) -> Result<Progress, BackendError> {
        let i = self
            .info(id)
            .await?
            .ok_or_else(|| BackendError::Unknown(id.into()))?;
        let total = if i.total_size > 0 {
            i.total_size
        } else {
            i.size
        };
        let error = matches!(i.state.as_str(), "error" | "missingFiles")
            .then(|| format!("qBittorrent reports {}", i.state));
        Ok(Progress {
            done: i.completed.max(0) as u64,
            total: total.max(0) as u64,
            speed: i.dlspeed,
            peers: i.num_seeds + i.num_leechs,
            finished: finished(&i),
            error,
        })
    }

    async fn pause(&self, id: &str) -> Result<(), BackendError> {
        self.either("stop", "pause", id).await
    }

    async fn resume(&self, id: &str) -> Result<(), BackendError> {
        self.either("start", "resume", id).await
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<(), BackendError> {
        let body = form(&[
            ("hashes", id),
            ("deleteFiles", if delete_files { "true" } else { "false" }),
        ]);
        let r = self
            .call(Method::Post, "torrents/delete", Some(body))
            .await?;
        if r.status == 200 {
            Ok(())
        } else {
            Err(BackendError::Refused(format!(
                "qBittorrent answered {}",
                r.status
            )))
        }
    }

    async fn video_file(&self, id: &str) -> Result<PathBuf, BackendError> {
        let info = self
            .info(id)
            .await?
            .ok_or_else(|| BackendError::Unknown(id.into()))?;
        let r = self
            .call(
                Method::Get,
                &format!("torrents/files?{}", form(&[("hash", id)])),
                None,
            )
            .await?;
        let files: Vec<File> = serde_json::from_str(&r.body).map_err(|e| {
            BackendError::Refused(format!("qBittorrent's file list did not parse: {e}"))
        })?;
        let listed: Vec<TorrentFile> = files
            .iter()
            .enumerate()
            .map(|(index, f)| TorrentFile {
                index,
                path: f.name.replace('\\', "/"),
                len: f.size,
            })
            .collect();
        let chosen = sinephile_torrent::select::choose(&listed, Wanted::default())
            .ok_or_else(|| BackendError::Refused("no video file in it".into()))?;
        Ok(Path::new(&info.save_path).join(&files[chosen].name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(source: &str) -> Download {
        Download {
            id: 1,
            media_item_id: None,
            title: "T".into(),
            source: source.into(),
            torrent: None,
            backend: "qbittorrent".into(),
            backend_id: None,
            priority: 0,
            state: sinephile_persistence::repositories::DownloadState::Queued,
            total_bytes: None,
            done_bytes: 0,
            limit_kib: None,
            not_before: None,
            reason: None,
            error: None,
            file_path: None,
            added_at: String::new(),
            finished_at: None,
        }
    }

    #[test]
    fn the_info_hash_is_read_from_hex_or_base32() {
        let hex = "abcdef0123456789".repeat(3);
        let hex = &hex[..40];
        assert_eq!(
            info_hash(&d(&format!("infohash:{hex}"))).as_deref(),
            Some(hex)
        );
        // 20 zero bytes are 32 'A's in base32 and 40 '0's in hex.
        let zeros = "0".repeat(40);
        assert_eq!(
            base32_to_hex(&"A".repeat(32)).as_deref(),
            Some(zeros.as_str())
        );
        assert_eq!(info_hash(&d("infohash:not-a-hash")), None);
        assert_eq!(info_hash(&d("a torrent file")), None);
    }

    #[test]
    fn a_hash_and_its_trackers_go_to_qbittorrent_as_one_link() {
        let hex = "0123456789abcdef".repeat(3);
        let hex = &hex[..40];
        let l = link(&d(&format!(
            "infohash:{hex}|udp://example.net:6969|http://example.org/x?a=1&b=2"
        )))
        .unwrap();
        let parsed = url::Url::parse(&l).unwrap();
        assert_eq!(parsed.scheme(), "magnet");
        let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs[0], ("xt".into(), format!("urn:btih:{hex}")));
        assert!(
            l.contains(&format!("xt=urn:btih:{hex}")),
            "the hash unescaped: {l}"
        );
        assert_eq!(
            pairs[1..],
            [
                ("tr".into(), "udp://example.net:6969".into()),
                ("tr".into(), "http://example.org/x?a=1&b=2".into()),
            ],
            "every tracker, each one whole"
        );
        // No trackers: the hash alone, still a link qBittorrent takes.
        let bare = url::Url::parse(&link(&d(&format!("infohash:{hex}"))).unwrap()).unwrap();
        assert_eq!(bare.query_pairs().count(), 1);
        assert_eq!(link(&d("a torrent file")), None);
    }

    #[test]
    fn an_add_is_taken_as_each_version_answers_it() {
        assert!(took(200, "Ok."), "before 5.2");
        assert!(!took(200, "Fails."));
        assert!(took(
            200,
            r#"{"added_torrent_ids":["x"],"failure_count":0,"pending_count":0,"success_count":1}"#
        ));
        assert!(
            took(
                202,
                r#"{"added_torrent_ids":[],"failure_count":0,"pending_count":1,"success_count":0}"#
            ),
            "5.2, still fetching"
        );
        assert!(!took(
            200,
            r#"{"added_torrent_ids":[],"failure_count":1,"pending_count":0,"success_count":0}"#
        ));
        assert!(!took(415, ""));
    }

    #[test]
    fn a_form_is_url_encoded() {
        assert_eq!(
            form(&[("urls", "a b&c"), ("savepath", r"D:\Downloads")]),
            "urls=a+b%26c&savepath=D%3A%5CDownloads"
        );
    }
}
