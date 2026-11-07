//! The local library (SPEC.md Phase 12). **Thin**: scanning, watching and applying
//! changes are `sinephile_library`'s; this starts them, keeps the watchers alive, and
//! tells the page how a scan is going.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use sinephile_library::identify::Cache;
use sinephile_library::probe::ffprobe;
use sinephile_library::scan::{scan, Progress};
use sinephile_library::watch::{apply, watch, Change, Quieten};
use sinephile_persistence::repositories::LibraryRepository;
use sinephile_persistence::Db;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LibraryFolderView {
    pub id: i32,
    pub path: String,
    pub files: i32,
    pub scanned_at: Option<String>,
    pub scanning: bool,
}

/// Sent as `library-scan` while a folder is scanned.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LibraryScanEvent {
    pub folder_id: i32,
    /// "walking" | "identifying" | "done" | "failed"
    pub phase: String,
    pub done: i32,
    pub total: i32,
    /// When done: files, accepted, asked.
    pub files: i32,
    pub accepted: i32,
    pub asked: i32,
}

/// Folders being scanned now.
static SCANNING: Mutex<Option<HashSet<i64>>> = Mutex::new(None);
/// One watcher per folder; dropping one stops it.
static WATCHERS: Mutex<Option<HashMap<i64, notify::RecommendedWatcher>>> = Mutex::new(None);
/// Every watcher's changes, to the one task that applies them.
static CHANGES: OnceLock<tokio::sync::mpsc::UnboundedSender<(i64, Change)>> = OnceLock::new();

fn scanning(id: i64) -> bool {
    SCANNING
        .lock()
        .expect("scanning")
        .as_ref()
        .is_some_and(|s| s.contains(&id))
}

fn db(state: &AppState) -> Result<Arc<Db>, String> {
    state
        .db()
        .ok_or_else(|| "the catalogue is still opening".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn library_folders(state: State<'_, AppState>) -> Result<Vec<LibraryFolderView>, String> {
    let db = db(&state)?;
    let folders = LibraryRepository::new(&db)
        .folders()
        .await
        .map_err(|e| e.to_string())?;
    Ok(folders
        .into_iter()
        .map(|f| LibraryFolderView {
            id: f.id as i32,
            scanning: scanning(f.id),
            path: f.path,
            files: f.files as i32,
            scanned_at: f.scanned_at,
        })
        .collect())
}

/// Add a folder, scan it, and watch it.
#[tauri::command]
#[specta::specta]
pub async fn add_library_folder(app: AppHandle, path: String) -> Result<i32, String> {
    let state = app.state::<AppState>();
    let db = db(&state)?;
    let dir = PathBuf::from(path.trim());
    if !dir.is_dir() {
        return Err(format!("{} is not a folder on this PC", dir.display()));
    }
    // One spelling of the path everywhere: the folder row, the walk, and the watcher's
    // events. The canonical form's `\\?\` prefix leaked into file rows once while the
    // folder row had none, so the next launch's scan saw every file as moved.
    let dir = dir.canonicalize().map_err(|e| e.to_string())?;
    let text = plain(&dir);
    let id = LibraryRepository::new(&db)
        .add_folder(&text)
        .await
        .map_err(|e| e.to_string())?;
    start_scan(app.clone(), id, PathBuf::from(text));
    Ok(id as i32)
}

#[tauri::command]
#[specta::specta]
pub async fn remove_library_folder(state: State<'_, AppState>, id: i32) -> Result<(), String> {
    let db = db(&state)?;
    if let Some(w) = WATCHERS.lock().expect("watchers").as_mut() {
        w.remove(&i64::from(id));
    }
    LibraryRepository::new(&db)
        .remove_folder(i64::from(id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn rescan_library_folder(app: AppHandle, id: i32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = db(&state)?;
    let folders = LibraryRepository::new(&db)
        .folders()
        .await
        .map_err(|e| e.to_string())?;
    let folder = folders
        .into_iter()
        .find(|f| f.id == i64::from(id))
        .ok_or("no such folder")?;
    start_scan(app.clone(), folder.id, PathBuf::from(folder.path));
    Ok(())
}

/// How many files are waiting for a human decision.
#[tauri::command]
#[specta::specta]
pub async fn library_review_count(state: State<'_, AppState>) -> Result<i32, String> {
    let db = db(&state)?;
    LibraryRepository::new(&db)
        .review_count(sinephile_library::ACCEPT_AT)
        .await
        .map(|n| n as i32)
        .map_err(|e| e.to_string())
}

/// Windows' own folder picker. `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn pick_library_folder(app: AppHandle) -> Result<Option<String>, String> {
    let owner = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);
    tauri::async_runtime::spawn_blocking(move || {
        // The dialog needs a single-threaded COM apartment of its own.
        std::thread::spawn(move || picker::folder(owner))
            .join()
            .unwrap_or(None)
    })
    .await
    .map_err(|e| e.to_string())
}

/// A canonical path without Windows' `\\?\` prefix, which nobody wants to read.
fn plain(p: &Path) -> String {
    let s = p.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

fn emit(app: &AppHandle, e: LibraryScanEvent) {
    let _ = app.emit("library-scan", e);
}

/// Scan a folder in the background, then make sure it is watched.
pub fn start_scan(app: AppHandle, id: i64, root: PathBuf) {
    {
        let mut s = SCANNING.lock().expect("scanning");
        let set = s.get_or_insert_with(HashSet::new);
        if !set.insert(id) {
            return; // already scanning
        }
    }
    tauri::async_runtime::spawn(async move {
        let Some(db) = app.state::<AppState>().db() else {
            return;
        };
        let event = |phase: &str, done: usize, total: usize| LibraryScanEvent {
            folder_id: id as i32,
            phase: phase.into(),
            done: done as i32,
            total: total as i32,
            files: 0,
            accepted: 0,
            asked: 0,
        };
        let progress_app = app.clone();
        let progress = move |p: Progress| {
            let e = match p {
                Progress::Walking { found } => event("walking", found, 0),
                Progress::Identifying { done, total } => event("identifying", done, total),
            };
            emit(&progress_app, e);
        };
        let outcome = scan(&db, id, &root, ffprobe, &progress).await;
        if let Some(s) = SCANNING.lock().expect("scanning").as_mut() {
            s.remove(&id);
        }
        match outcome {
            Ok(r) => {
                tracing::info!(folder = id, ?r, "library scanned");
                emit(
                    &app,
                    LibraryScanEvent {
                        files: r.files as i32,
                        accepted: r.accepted as i32,
                        asked: r.asked as i32,
                        ..event("done", r.files, r.files)
                    },
                );
                ensure_watched(id, &root);
            }
            Err(error) => {
                tracing::error!(folder = id, %error, "library scan failed");
                emit(&app, event("failed", 0, 0));
            }
        }
    });
}

fn ensure_watched(id: i64, root: &Path) {
    let Some(tx) = CHANGES.get().cloned() else {
        return;
    };
    let mut w = WATCHERS.lock().expect("watchers");
    let map = w.get_or_insert_with(HashMap::new);
    if map.contains_key(&id) {
        return;
    }
    match watch(root, move |c| {
        let _ = tx.send((id, c));
    }) {
        Ok(watcher) => {
            map.insert(id, watcher);
        }
        Err(error) => tracing::warn!(folder = id, %error, "could not watch the folder"),
    }
}

/// At launch: scan every folder (catching what changed while the app was closed), then
/// watch it; and apply what the watchers report, once each path has been quiet for two
/// seconds.
pub async fn library_loop(app: AppHandle) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, Change)>();
    let _ = CHANGES.set(tx);

    // Wait for the database, which opens off the startup path.
    let db = loop {
        if let Some(db) = app.state::<AppState>().db() {
            break db;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    if let Ok(folders) = LibraryRepository::new(&db).folders().await {
        for f in folders {
            start_scan(app.clone(), f.id, PathBuf::from(f.path));
        }
    }

    let mut pending: HashMap<i64, Quieten> = HashMap::new();
    let cache = Cache::default();
    loop {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Some((id, change))) => pending.entry(id).or_default().push(change, Instant::now()),
            Ok(None) => return,
            Err(_) => {}
        }
        let ready: Vec<(i64, Vec<Change>)> = pending
            .iter_mut()
            .map(|(id, q)| (*id, q.ready(Instant::now(), Duration::from_secs(2))))
            .filter(|(_, c)| !c.is_empty())
            .collect();
        if ready.is_empty() {
            continue;
        }
        let Ok(folders) = LibraryRepository::new(&db).folders().await else {
            continue;
        };
        for (id, changes) in ready {
            // A folder being scanned will see the change itself.
            if scanning(id) {
                continue;
            }
            let Some(folder) = folders.iter().find(|f| f.id == id) else {
                continue;
            };
            let root = PathBuf::from(&folder.path);
            for c in &changes {
                match apply(&db, id, &root, c, ffprobe, &cache).await {
                    Ok(done) => tracing::info!(folder = id, ?c, ?done, "library change"),
                    Err(error) => tracing::warn!(folder = id, %error, "library change failed"),
                }
            }
            let _ = app.emit("library-changed", id as i32);
        }
    }
}

#[cfg(windows)]
pub(crate) mod picker {
    use windows::core::w;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH,
    };

    /// Windows' file picker, titled `title`, owned by the app's window (Phase 14 imports).
    pub fn file(owner: Option<isize>, title: &str) -> Option<String> {
        // SAFETY: as `folder`: COM for exactly the dialog's lifetime, the one COM string
        // freed with CoTaskMemFree.
        unsafe {
            if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
                return None;
            }
            let picked = (|| {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
                let options = dialog.GetOptions().ok()?;
                dialog
                    .SetOptions(options | FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM)
                    .ok()?;
                dialog.SetTitle(&HSTRING::from(title)).ok()?;
                dialog.Show(owner.map(|h| HWND(h as *mut _))).ok()?;
                let item = dialog.GetResult().ok()?;
                let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
                let path = name.to_string().ok();
                CoTaskMemFree(Some(name.0 as *const _));
                path
            })();
            CoUninitialize();
            picked
        }
    }

    /// Windows' folder picker, owned by the app's window so it sits above it.
    pub fn folder(owner: Option<isize>) -> Option<String> {
        // SAFETY: COM is initialised on this thread for exactly the dialog's lifetime;
        // every interface is released (dropped) before CoUninitialize, and the one string
        // COM allocates is freed with CoTaskMemFree.
        unsafe {
            if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
                return None;
            }
            let picked = (|| {
                let dialog: IFileOpenDialog =
                    CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
                let options = dialog.GetOptions().ok()?;
                dialog
                    .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
                    .ok()?;
                dialog.SetTitle(w!("Add a folder to your library")).ok()?;
                dialog.Show(owner.map(|h| HWND(h as *mut _))).ok()?;
                let item = dialog.GetResult().ok()?;
                let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
                let path = name.to_string().ok();
                CoTaskMemFree(Some(name.0 as *const _));
                path
            })();
            CoUninitialize();
            picked
        }
    }
}

#[cfg(not(windows))]
pub(crate) mod picker {
    pub fn folder(_: Option<isize>) -> Option<String> {
        None
    }
    pub fn file(_: Option<isize>, _: &str) -> Option<String> {
        None
    }
}

// ---- the review queue -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ReviewOptionView {
    pub media_item_id: i32,
    /// "film" | "series" | "anime_film" | "anime_series"
    pub kind: String,
    pub title: String,
    pub year: Option<i32>,
    pub runtime_minutes: Option<i32>,
    pub made_by: Option<String>,
    /// The first sentence, for a card.
    pub synopsis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ReviewItemView {
    pub file_id: i32,
    /// The file's own name.
    pub name: String,
    /// The folders it sits in, below the library folder.
    pub folders: String,
    pub minutes: Option<i32>,
    pub height: Option<i32>,
    /// What the name was read as: the title, the year, and the episode in words.
    pub read_title: String,
    pub read_year: Option<i32>,
    pub read_episode: String,
    /// Other waiting files named the same way, which one answer also settles.
    pub same_name: i32,
    pub options: Vec<ReviewOptionView>,
}

#[tauri::command]
#[specta::specta]
pub async fn library_review(
    state: State<'_, AppState>,
    limit: i32,
) -> Result<Vec<ReviewItemView>, String> {
    use sinephile_library::review::{episode_words, first_sentence, queue};
    let db = db(&state)?;
    let items = queue(&db, i64::from(limit))
        .await
        .map_err(|e| e.to_string())?;
    Ok(items
        .into_iter()
        .map(|i| ReviewItemView {
            file_id: i.file.id as i32,
            name: Path::new(&i.file.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            folders: i.folders.join(" / "),
            minutes: i.file.duration_seconds.map(|s| (s / 60) as i32),
            height: i.file.height.map(|h| h as i32),
            read_episode: episode_words(&i.reading),
            read_title: i.reading.title.clone(),
            read_year: i.reading.year.map(i32::from),
            same_name: i.same_name as i32,
            options: i
                .options
                .into_iter()
                .map(|o| ReviewOptionView {
                    media_item_id: o.candidate.media_item_id as i32,
                    kind: o.candidate.kind,
                    title: o.candidate.primary_title,
                    year: o.candidate.release_year.map(|y| y as i32),
                    runtime_minutes: o.candidate.runtime_minutes.map(|m| m as i32),
                    made_by: o.made_by,
                    synopsis: o
                        .synopsis
                        .map(|s| first_sentence(&s, 140))
                        .filter(|s| !s.is_empty()),
                })
                .collect(),
        })
        .collect())
}

/// The answer: this file is that film or series. Returns how many other waiting files
/// the same answer settled.
#[tauri::command]
#[specta::specta]
pub async fn resolve_review(
    app: AppHandle,
    file_id: i32,
    media_item_id: i32,
) -> Result<i32, String> {
    let state = app.state::<AppState>();
    let db = db(&state)?;
    let done =
        sinephile_library::review::resolve(&db, i64::from(file_id), i64::from(media_item_id))
            .await
            .map_err(|e| e.to_string())?;
    let _ = app.emit("library-changed", 0);
    Ok(done.settled as i32)
}

/// "Not a film or episode": never asked about again.
#[tauri::command]
#[specta::specta]
pub async fn not_a_title(state: State<'_, AppState>, file_id: i32) -> Result<(), String> {
    let db = db(&state)?;
    sinephile_library::review::not_a_title(&db, i64::from(file_id))
        .await
        .map_err(|e| e.to_string())
}
