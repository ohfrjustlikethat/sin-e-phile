//! Scanning a library folder: find the video files, reconcile them with what is stored,
//! and identify what is new.
//!
//! - **Walk** the folder (no probing, no hashing: a name, a size and a modification time
//!   each), so 10,000 files are listed in seconds.
//! - **Reconcile.** A file at a known path with the same size and time is unchanged. A file
//!   that vanished is marked missing; a new path with the size and time of a missing file
//!   is that file, moved or renamed, and keeps its row and its match (Phase 12 E4).
//!   Windows keeps a file's modification time across a rename or a move within a volume.
//! - **Identify** what is new or changed, several at once, sharing catalogue searches
//!   between a series' episodes. FFprobe runs only when the name alone was not enough.
//! - Whatever is still missing at the end is gone, and is removed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures::stream::{self, StreamExt};
use sinephile_persistence::repositories::{
    FileMatch, LibraryRepository, LocalFile, MatchMethod, NewLocalFile, Probe, Upserted,
};
use sinephile_persistence::{Db, DbError};

use crate::identify::{identify_with, Cache, Evidence, Identification, ACCEPT_AT};

/// Video containers a library holds.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "wmv", "mpg", "mpeg", "ts", "m2ts", "webm", "flv", "ogv",
    "vob", "divx", "rmvb", "ogm",
];

/// What runs FFprobe: `probe::ffprobe` in the app, a stand-in in tests.
pub type Prober = fn(&Path) -> Option<Probe>;

/// How many files are identified at once. The pool has eight connections; one is left
/// for the app.
pub const PARALLEL: usize = 7;

/// A file as the walk finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnDisk {
    pub path: PathBuf,
    pub size: i64,
    /// Seconds and nanoseconds since 1970, as text: exact, and comparable as a string.
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Walking { found: usize },
    Identifying { done: usize, total: usize },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub files: usize,
    pub new: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub moved: usize,
    pub removed: u64,
    /// Identified and accepted without asking.
    pub accepted: usize,
    /// Identified, but below the threshold: in the review queue.
    pub asked: usize,
    pub probed: usize,
    pub elapsed_ms: u128,
}

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// A release's sample clip, not the film: "sample" in the name and small.
fn is_sample(path: &Path, size: i64) -> bool {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .map(filename_parser::comparable)
        .unwrap_or_default();
    size < 300 * 1024 * 1024 && name.split(' ').any(|w| w == "sample")
}

pub fn modified_text(time: SystemTime) -> Option<String> {
    let d = time.duration_since(UNIX_EPOCH).ok()?;
    Some(format!("{}.{:09}", d.as_secs(), d.subsec_nanos()))
}

/// One file's size and time, if it is a video worth listing.
pub fn on_disk(path: &Path) -> Option<OnDisk> {
    if !is_video(path) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let size = meta.len() as i64;
    if is_sample(path, size) {
        return None;
    }
    Some(OnDisk {
        path: path.to_path_buf(),
        size,
        modified: meta.modified().ok().and_then(modified_text),
    })
}

/// Folders Windows keeps for itself, or that the user has hidden. Decided by the
/// folder's attributes and two system names, never by its first character: "$9.99 (2008)"
/// and ".45 (2006)" are films (found by `eval scan`, which lost both).
fn skipped(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if name.eq_ignore_ascii_case("$RECYCLE.BIN")
        || name.eq_ignore_ascii_case("System Volume Information")
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & (HIDDEN | SYSTEM) != 0)
    }
    #[cfg(not(windows))]
    {
        name.starts_with('.')
    }
}

/// Every video under `root`. System and hidden folders are skipped; links are not
/// followed (a loop would never end). A release's extras (featurettes, interviews, menus)
/// are not the library's: left out, so none is ever searched for as a film (Phase 16).
pub fn walk(root: &Path, progress: &(dyn Fn(Progress) + Sync)) -> Vec<OnDisk> {
    let mut out = Vec::new();
    let entries = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !skipped(e));
    for entry in entries.flatten() {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if crate::folders::is_extra(&folders_between(root, entry.path()), &name) {
                continue;
            }
            if let Some(f) = on_disk(entry.path()) {
                out.push(f);
                if out.len() % 500 == 0 {
                    progress(Progress::Walking { found: out.len() });
                }
            }
        }
    }
    progress(Progress::Walking { found: out.len() });
    out
}

/// The folders between `root` and the file, outermost first: what identification reads.
pub fn folders_between(root: &Path, file: &Path) -> Vec<String> {
    file.parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .map(|rel| {
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Record a file found on disk: unchanged, changed, a moved file found again, or new.
/// Returns the row and whether it needs identifying.
pub async fn record(
    db: &Db,
    folder_id: i64,
    f: &OnDisk,
) -> Result<(Upserted, Option<LocalFile>), DbError> {
    let repo = LibraryRepository::new(db);
    let path = path_text(&f.path);
    if repo.file_by_path(&path).await?.is_none() {
        // A moved or renamed file keeps its size and time: the same file, found again.
        let missing = repo.missing_with_size(f.size).await?;
        let same: Vec<&LocalFile> = missing
            .iter()
            .filter(|m| m.modified_at.is_some() && m.modified_at == f.modified)
            .collect();
        if let [one] = same.as_slice() {
            repo.move_file(one.id, &path, folder_id).await?;
            let moved = repo.file(one.id).await?;
            return Ok((Upserted::Unchanged(one.id), moved));
        }
    }
    let up = repo
        .upsert_file(&NewLocalFile {
            folder_id,
            path: &path,
            size_bytes: f.size,
            modified_at: f.modified.as_deref(),
        })
        .await?;
    Ok((up, None))
}

/// What to store for an identification: a match with its confidence (the best guess even
/// when unsure, so the queue can show it), and the candidates when it must be asked.
pub async fn store(
    db: &Db,
    file_id: i64,
    id: &Identification,
    probed: bool,
) -> Result<bool, DbError> {
    let repo = LibraryRepository::new(db);
    let Some(best) = id.best() else {
        repo.set_candidates(file_id, &[]).await?;
        return Ok(false);
    };
    let accepted = id.confidence >= ACCEPT_AT;
    let (item, covers) = match id.episode {
        Some(e) => (e.episode_id, e.covers),
        None => (best.media_item_id, 1),
    };
    repo.set_match(
        file_id,
        &FileMatch {
            media_item_id: item,
            confidence: id.confidence,
            method: if probed {
                MatchMethod::Metadata
            } else {
                MatchMethod::Filename
            },
            covers,
            confirmed: false,
        },
    )
    .await?;
    let offered: Vec<(i64, f64)> = if accepted {
        Vec::new()
    } else {
        id.candidates
            .iter()
            .take(5)
            .map(|c| (c.media_item_id, c.score))
            .collect()
    };
    repo.set_candidates(file_id, &offered).await?;
    Ok(accepted)
}

/// Identify one stored file: by name and folders first, and with FFprobe's duration only
/// if that was not enough. Returns (accepted, probed).
pub async fn identify_file(
    db: &Db,
    root: &Path,
    file: &LocalFile,
    probe: Prober,
    cache: &Cache,
) -> Result<(bool, bool), DbError> {
    let path = PathBuf::from(&file.path);
    let mut evidence = Evidence {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        folders: folders_between(root, &path),
        duration_minutes: file.duration_seconds.map(|s| s as f64 / 60.0),
    };
    let mut id = identify_with(db, &evidence, Some(cache)).await?;
    let mut probed = false;
    if id.confidence < ACCEPT_AT && !file.probed && id.best().is_some() {
        // On the blocking pool: FFprobe is a process, and waiting on it here would hold up
        // every identification running alongside.
        let target = path.clone();
        let found = tokio::task::spawn_blocking(move || probe(&target))
            .await
            .ok()
            .flatten();
        if let Some(p) = found {
            LibraryRepository::new(db).set_probe(file.id, &p).await?;
            probed = true;
            if let Some(secs) = p.duration_seconds {
                evidence.duration_minutes = Some(secs as f64 / 60.0);
                id = identify_with(db, &evidence, Some(cache)).await?;
            }
        }
    }
    let accepted = store(db, file.id, &id, probed).await?;
    Ok((accepted, probed))
}

/// Scan one library folder. `probe` is FFprobe in the app and a stand-in in tests.
pub async fn scan(
    db: &Db,
    folder_id: i64,
    root: &Path,
    probe: Prober,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<ScanReport, DbError> {
    let started = Instant::now();
    let repo = LibraryRepository::new(db);
    let found = walk(root, progress);
    let mut report = ScanReport {
        files: found.len(),
        ..Default::default()
    };

    // What vanished: missing until the end, in case it turns up elsewhere.
    let here: std::collections::HashSet<String> =
        found.iter().map(|f| path_text(&f.path)).collect();
    for known in repo.files_in_folder(folder_id).await? {
        if known.missing_since.is_none() && !here.contains(&known.path) {
            repo.mark_missing(known.id).await?;
        }
    }

    let mut to_identify: Vec<i64> = Vec::new();
    for f in &found {
        let (up, moved) = record(db, folder_id, f).await?;
        match (up, moved) {
            (_, Some(_)) => report.moved += 1,
            (Upserted::New(id), _) => {
                report.new += 1;
                to_identify.push(id);
            }
            (Upserted::Changed(id), _) => {
                report.changed += 1;
                to_identify.push(id);
            }
            (Upserted::Unchanged(id), _) => {
                report.unchanged += 1;
                // Never identified (or forgotten): try again, e.g. after a catalogue update.
                if repo.match_of(id).await?.is_none() {
                    to_identify.push(id);
                }
            }
        }
    }
    report.removed = repo.purge_missing(folder_id).await?;

    let total = to_identify.len();
    let done = AtomicUsize::new(0);
    let cache = Cache::default();
    let results: Vec<Result<(bool, bool), DbError>> = stream::iter(to_identify)
        .map(|id| {
            let (done, cache) = (&done, &cache);
            async move {
                let file = LibraryRepository::new(db).file(id).await?;
                let r = match file {
                    Some(f) => identify_file(db, root, &f, probe, cache).await,
                    None => Ok((false, false)),
                };
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 50 == 0 || n == total {
                    progress(Progress::Identifying { done: n, total });
                }
                r
            }
        })
        .buffer_unordered(PARALLEL)
        .collect()
        .await;
    for r in results {
        let (accepted, probed) = r?;
        if accepted {
            report.accepted += 1;
        } else {
            report.asked += 1;
        }
        report.probed += usize::from(probed);
    }

    repo.mark_folder_scanned(folder_id).await?;
    report.elapsed_ms = started.elapsed().as_millis();
    Ok(report)
}
