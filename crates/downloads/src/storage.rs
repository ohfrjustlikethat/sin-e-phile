//! What downloads keep on disk, and clearing it (SPEC.md Phase 13: a size budget, and a
//! cleanup view of watched downloads by size and age).

use std::path::{Path, PathBuf};

use sinephile_persistence::repositories::{
    Download, DownloadRepository, DownloadState, LibraryRepository, ProfileRepository,
};
use sinephile_persistence::Db;

use crate::manager::ManagerError;

/// One finished download, as the cleanup view lists it.
#[derive(Debug, Clone)]
pub struct Stored {
    pub download: Download,
    /// What it takes on disk now (the file's size; the recorded size if it is gone).
    pub bytes: u64,
    /// Days since it finished.
    pub age_days: f64,
    /// Seen through by this profile: the first candidates for clearing.
    pub watched: bool,
}

/// Seconds since 1970 for `"2026-11-07T22:30:00Z"`, the form `finished_at` is stored in.
pub fn unix_of_iso(text: &str) -> Option<i64> {
    let (date, time) = text.trim_end_matches('Z').split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next()??);
    Some(crate::measure::days_from_civil(y, m, day) * 86_400 + h * 3600 + min * 60 + s)
}

/// Finished downloads, the best to clear first: watched before unwatched, then the
/// largest, then the oldest.
pub async fn stored(db: &Db, profile_id: i64, now_unix: i64) -> Result<Vec<Stored>, ManagerError> {
    let profiles = ProfileRepository::new(db);
    let mut out = Vec::new();
    for d in DownloadRepository::new(db).all().await? {
        if d.state != DownloadState::Done {
            continue;
        }
        let on_disk = d
            .file_path
            .as_deref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());
        let bytes = on_disk.unwrap_or(d.total_bytes.unwrap_or(d.done_bytes).max(0) as u64);
        let age_days = d
            .finished_at
            .as_deref()
            .and_then(unix_of_iso)
            .map_or(0.0, |t| (now_unix - t).max(0) as f64 / 86_400.0);
        let watched = match d.media_item_id {
            Some(item) => profiles.finished(profile_id, item).await?,
            None => false,
        };
        out.push(Stored {
            download: d,
            bytes,
            age_days,
            watched,
        });
    }
    out.sort_by(|a, b| {
        b.watched
            .cmp(&a.watched)
            .then(b.bytes.cmp(&a.bytes))
            .then(b.age_days.total_cmp(&a.age_days))
    });
    Ok(out)
}

/// The room a new download has: the free disk, and within the budget if one is set.
pub fn room(free_disk: u64, used: u64, budget: Option<u64>) -> u64 {
    match budget {
        Some(b) => free_disk.min(b.saturating_sub(used)),
        None => free_disk,
    }
}

/// What to delete for a download's file: the download's own folder (its first folder
/// inside the downloads folder, with any subtitles beside it), or the file alone when it
/// is not inside the downloads folder. Never the downloads folder itself.
pub fn to_delete(file: &Path, dir: &Path) -> PathBuf {
    match file
        .strip_prefix(dir)
        .ok()
        .and_then(|r| r.components().next())
    {
        Some(first) if file.parent() != Some(dir) => dir.join(first),
        _ => file.to_path_buf(),
    }
}

/// Clear a finished download: its files from disk, its file from the library, its row.
pub async fn clear(db: &Db, id: i64, dir: &Path) -> Result<(), ManagerError> {
    let repo = DownloadRepository::new(db);
    let Some(d) = repo.get(id).await? else {
        return Ok(());
    };
    if let Some(path) = d.file_path.as_deref() {
        let target = to_delete(Path::new(path), dir);
        let gone = if target.is_dir() {
            std::fs::remove_dir_all(&target)
        } else {
            std::fs::remove_file(&target)
        };
        if let Err(e) = gone {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(ManagerError::Io(e.to_string()));
            }
        }
        let library = LibraryRepository::new(db);
        if let Some(f) = library.file_by_path(path).await? {
            library.delete_file(f.id).await?;
        }
    }
    repo.remove(id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_time_reads_back() {
        assert_eq!(unix_of_iso("2026-11-07T22:30:00Z"), Some(1_794_090_600));
        assert_eq!(unix_of_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_of_iso("yesterday"), None);
    }

    #[test]
    fn the_budget_caps_the_room_and_no_budget_is_the_disk() {
        assert_eq!(room(100, 30, Some(50)), 20);
        assert_eq!(room(10, 30, Some(50)), 10, "the disk is still the limit");
        assert_eq!(room(100, 80, Some(50)), 0, "over budget");
        assert_eq!(room(100, 80, None), 100);
    }

    #[test]
    fn clearing_takes_the_downloads_own_folder_and_never_the_downloads_folder() {
        let dir = Path::new(r"D:\data\Downloads");
        assert_eq!(
            to_delete(Path::new(r"D:\data\Downloads\Film\Film\film.mkv"), dir),
            Path::new(r"D:\data\Downloads\Film")
        );
        assert_eq!(
            to_delete(Path::new(r"D:\data\Downloads\film.mkv"), dir),
            Path::new(r"D:\data\Downloads\film.mkv"),
            "a file directly in the folder is deleted alone"
        );
        assert_eq!(
            to_delete(Path::new(r"E:\elsewhere\film.mkv"), dir),
            Path::new(r"E:\elsewhere\film.mkv"),
            "outside the folder (a qBittorrent save path): the file alone"
        );
    }
}
