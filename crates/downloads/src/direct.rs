//! `Direct`: downloads a web address (Phase 16, the author's brief item 2) — the Internet
//! Archive's public-domain files, and add-ons that answer with links. A torrent goes to
//! librqbit or qBittorrent; this backend takes only `http`/`https` sources.
//!
//! It writes to `<name>.part` beside the final file and renames it when the last byte is
//! in, so a half-written file is never filed into the library. A pause, a failure or an
//! app restart leaves the `.part`; the next start asks the server for the rest
//! (`Range: bytes=<have>-`), and starts again only if the server will not say.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use sinephile_persistence::repositories::Download;
use tokio::io::AsyncWriteExt;

use crate::backend::{folder_name, Backend, BackendError, Progress};

/// The name stored in `downloads.backend`.
pub const NAME: &str = "http";

/// Whether a download's source is a web address this backend fetches.
pub fn is_web(d: &Download) -> bool {
    d.torrent.is_none() && (d.source.starts_with("https://") || d.source.starts_with("http://"))
}

#[derive(Default)]
struct Job {
    url: String,
    path: PathBuf,
    done: u64,
    total: Option<u64>,
    speed: f64,
    finished: bool,
    error: Option<String>,
    task: Option<tokio::task::JoinHandle<()>>,
}

type Jobs = Arc<Mutex<HashMap<String, Job>>>;

pub struct Direct {
    client: reqwest::Client,
    jobs: Jobs,
}

impl Default for Direct {
    fn default() -> Self {
        Self::new()
    }
}

impl Direct {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(20))
                .build()
                .expect("an HTTP client with no special options always builds"),
            jobs: Arc::default(),
        }
    }

    fn start(&self, id: &str) {
        let (url, path) = {
            let jobs = self.jobs.lock().expect("jobs");
            let Some(j) = jobs.get(id) else { return };
            (j.url.clone(), j.path.clone())
        };
        let task = tokio::spawn(fetch(
            self.client.clone(),
            url,
            path,
            id.to_string(),
            self.jobs.clone(),
        ));
        if let Some(j) = self.jobs.lock().expect("jobs").get_mut(id) {
            j.error = None;
            j.task = Some(task);
        }
    }
}

/// `<path>.part`: where the bytes go until the last one is in.
pub fn part_of(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// The file's name from its address ("…/Nosferatu_1922.mp4"), made safe for Windows;
/// the title, as an `.mp4`, when the address gives none.
pub fn file_name(url: &str, title: &str) -> String {
    let last = url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut s| s.next_back().map(str::to_string))
        })
        .map(|s| {
            url::form_urlencoded::parse(format!("x={s}").as_bytes())
                .next()
                .map(|(_, v)| v.into_owned())
                .unwrap_or(s)
        })
        .unwrap_or_default();
    let clean: String = last
        .chars()
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string();
    if clean.contains('.') && clean.len() > 2 {
        clean
    } else {
        format!("{}.mp4", folder_name(title))
    }
}

fn set(jobs: &Jobs, id: &str, f: impl FnOnce(&mut Job)) {
    if let Some(j) = jobs.lock().expect("jobs").get_mut(id) {
        f(j);
    }
}

async fn fetch(client: reqwest::Client, url: String, path: PathBuf, id: String, jobs: Jobs) {
    if let Err(e) = fetch_inner(&client, &url, &path, &id, &jobs).await {
        set(&jobs, &id, |j| j.error = Some(e));
    }
}

async fn fetch_inner(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    id: &str,
    jobs: &Jobs,
) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("could not make the folder: {e}"))?;
    }
    let part = part_of(path);
    let mut have = tokio::fs::metadata(&part)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let mut request = client.get(url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request
        .send()
        .await
        .map_err(|e| format!("could not reach it: {e}"))?;
    let status = response.status().as_u16();
    let append = match status {
        206 => true,
        200 => {
            have = 0; // the server sends it all again: start the file again
            false
        }
        416 if have > 0 => {
            // Nothing past what is already here: it was complete.
            tokio::fs::rename(&part, path)
                .await
                .map_err(|e| format!("could not finish the file: {e}"))?;
            set(jobs, id, |j| {
                j.done = have;
                j.total = Some(have);
                j.finished = true;
            });
            return Ok(());
        }
        s => return Err(format!("the server answered {s}")),
    };
    let total = response.content_length().map(|n| n + have);
    set(jobs, id, |j| {
        j.done = have;
        j.total = total;
    });
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&part)
        .await
        .map_err(|e| format!("could not write the file: {e}"))?;
    let mut done = have;
    let (mut mark, mut mark_bytes) = (Instant::now(), done);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("the connection failed: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("could not write the file: {e}"))?;
        done += chunk.len() as u64;
        let elapsed = mark.elapsed();
        let speed = (elapsed >= Duration::from_millis(500))
            .then(|| (done - mark_bytes) as f64 / elapsed.as_secs_f64());
        if speed.is_some() {
            (mark, mark_bytes) = (Instant::now(), done);
        }
        set(jobs, id, |j| {
            j.done = done;
            if let Some(s) = speed {
                j.speed = s;
            }
        });
    }
    file.flush()
        .await
        .map_err(|e| format!("could not write the file: {e}"))?;
    drop(file);
    if total.is_some_and(|t| done < t) {
        return Err("the connection closed before the end; it will carry on from here".into());
    }
    tokio::fs::rename(&part, path)
        .await
        .map_err(|e| format!("could not finish the file: {e}"))?;
    set(jobs, id, |j| {
        j.total = Some(done);
        j.speed = 0.0;
        j.finished = true;
    });
    Ok(())
}

#[async_trait]
impl Backend for Direct {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn available(&self) -> bool {
        true
    }

    fn accepts(&self, d: &Download) -> bool {
        is_web(d)
    }

    async fn holds(&self, id: &str) -> bool {
        self.jobs.lock().expect("jobs").contains_key(id)
    }

    async fn add(&self, d: &Download, dir: &Path) -> Result<String, BackendError> {
        if !is_web(d) {
            return Err(BackendError::Refused(format!(
                "{} is not a web address",
                d.title
            )));
        }
        let id = format!("{NAME}-{}", d.id);
        let path = dir
            .join(folder_name(&d.title))
            .join(file_name(&d.source, &d.title));
        self.jobs.lock().expect("jobs").insert(
            id.clone(),
            Job {
                url: d.source.clone(),
                path,
                ..Job::default()
            },
        );
        self.start(&id);
        Ok(id)
    }

    async fn progress(&self, id: &str) -> Result<Progress, BackendError> {
        let jobs = self.jobs.lock().expect("jobs");
        let j = jobs
            .get(id)
            .ok_or_else(|| BackendError::Unknown(id.into()))?;
        Ok(Progress {
            done: j.done,
            total: j.total.unwrap_or(0),
            speed: j.speed,
            peers: 0,
            finished: j.finished,
            error: j.error.clone(),
        })
    }

    async fn pause(&self, id: &str) -> Result<(), BackendError> {
        let mut jobs = self.jobs.lock().expect("jobs");
        let j = jobs
            .get_mut(id)
            .ok_or_else(|| BackendError::Unknown(id.into()))?;
        if let Some(t) = j.task.take() {
            t.abort();
        }
        j.speed = 0.0;
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<(), BackendError> {
        if !self.holds(id).await {
            return Err(BackendError::Unknown(id.into()));
        }
        let running = self
            .jobs
            .lock()
            .expect("jobs")
            .get(id)
            .is_some_and(|j| j.task.as_ref().is_some_and(|t| !t.is_finished()));
        if !running {
            self.start(id);
        }
        Ok(())
    }

    async fn remove(&self, id: &str, delete_files: bool) -> Result<(), BackendError> {
        let job = self.jobs.lock().expect("jobs").remove(id);
        let Some(j) = job else { return Ok(()) };
        if let Some(t) = j.task {
            t.abort();
        }
        if delete_files {
            let _ = std::fs::remove_file(part_of(&j.path));
            let _ = std::fs::remove_file(&j.path);
        }
        Ok(())
    }

    async fn video_file(&self, id: &str, _file: Option<usize>) -> Result<PathBuf, BackendError> {
        let jobs = self.jobs.lock().expect("jobs");
        let j = jobs
            .get(id)
            .ok_or_else(|| BackendError::Unknown(id.into()))?;
        Ok(j.path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_named_from_its_address_or_its_title() {
        assert_eq!(
            file_name("https://cdn.example/a/Nosferatu%20(1922).mp4", "Nosferatu"),
            "Nosferatu (1922).mp4"
        );
        assert_eq!(
            file_name("https://cdn.example/watch?v=1", "Nosferatu"),
            "Nosferatu.mp4"
        );
        assert_eq!(
            file_name("https://cdn.example/a%3Ab%3F.mkv", "x"),
            "a b .mkv"
        );
        assert_eq!(
            part_of(Path::new("D:/x/a.mp4")),
            PathBuf::from("D:/x/a.mp4.part")
        );
    }
}
