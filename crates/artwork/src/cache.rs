//! The artwork cache: content-addressed files under `./data/`, with a size budget.
//!
//! # Content-addressed, not URL-addressed
//!
//! The filename is a hash of the *source URL*, so the same poster requested twice is
//! stored once and no path traversal is possible from a remote string. A URL can
//! contain anything at all — `../`, a colon, four hundred characters — and turning one
//! into a filename directly is how a cache becomes a way to write anywhere on disk.
//!
//! # The budget, and why eviction is by access time
//!
//! An unbounded image cache on a portable install eventually fills the drive it is
//! carried on. The budget is a soft ceiling: writes are never refused, and eviction
//! runs after a write that takes the cache over.
//!
//! Least-recently-*used*, not least-recently-written. A poster fetched months ago and
//! looked at yesterday is a poster on the user's home screen; one fetched yesterday and
//! never looked at again was a search result they scrolled past. Write order would evict
//! exactly the wrong one.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("artwork cache at {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error(transparent)]
    Encode(#[from] crate::encode::EncodeError),
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> CacheError + '_ {
    move |e| CacheError::Io(path.to_path_buf(), e)
}

/// What a stored image is, from the caller's point of view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub path: PathBuf,
    pub blurhash: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

/// FNV-1a. A cache filename needs to be short, stable and collision-resistant enough
/// that two posters do not share one; it is not a security boundary, and saying so is
/// better than reaching for SHA-256 and implying otherwise.
fn key_for(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub struct ArtworkCache {
    root: PathBuf,
    budget_bytes: u64,
}

impl ArtworkCache {
    /// 500 MB by default.
    ///
    /// Chosen rather than measured, and worth saying so: TMDB's `w500` posters run
    /// 30–60 KB as WebP, so this is roughly ten thousand of them — far more than a
    /// browsing session touches, and small beside the catalogue database. It is a
    /// constant here so the settings screen can move it.
    pub const DEFAULT_BUDGET: u64 = 500 * 1024 * 1024;

    pub fn new(root: impl Into<PathBuf>, budget_bytes: u64) -> Self {
        Self {
            root: root.into(),
            budget_bytes,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, url: &str) -> PathBuf {
        // Two hex characters of prefix directory. Ten thousand files in one directory
        // is slow to enumerate on NTFS, and the eviction sweep enumerates.
        let key = key_for(url);
        self.root.join(&key[0..2]).join(format!("{key}.webp"))
    }

    /// Is this already cached? Touches the file so eviction sees it as recently used.
    ///
    /// **Lazy fetch lives here**: a caller asks the cache first and only reaches the
    /// network on `None`. Nothing in this crate fetches anything.
    pub fn get(&self, url: &str) -> Option<PathBuf> {
        let path = self.path_for(url);
        if !path.is_file() {
            return None;
        }
        // Best-effort. A cache that failed a read because it could not update an
        // access time would be worse than one with slightly stale eviction order.
        let _ = filetime_now(&path);
        Some(path)
    }

    /// Store an image fetched from `url`, re-encoding it and computing its blurhash.
    pub fn put(&self, url: &str, bytes: &[u8]) -> Result<Stored, CacheError> {
        let prepared = crate::encode::prepare(bytes)?;
        let path = self.path_for(url);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io(parent))?;
        }

        // Write beside, then rename. A half-written poster that a later run treats as
        // cached is a permanently broken image with no way to notice — and this is
        // exactly the kind of write that gets interrupted, because it happens while
        // the user is scrolling.
        let temporary = path.with_extension("webp.part");
        fs::write(&temporary, &prepared.webp).map_err(io(&temporary))?;
        fs::rename(&temporary, &path).map_err(io(&path))?;

        let stored = Stored {
            path,
            blurhash: prepared.blurhash,
            width: prepared.width,
            height: prepared.height,
            bytes: prepared.webp.len() as u64,
        };
        self.evict_if_over_budget()?;
        Ok(stored)
    }

    /// Total bytes currently held.
    pub fn size(&self) -> Result<u64, CacheError> {
        Ok(self.entries()?.iter().map(|e| e.bytes).sum())
    }

    /// Remove one image — for "forget this", and for the key-removal path in
    /// ADR-0027, which discards artwork obtained under a key that is being deleted.
    pub fn remove(&self, url: &str) -> Result<bool, CacheError> {
        let path = self.path_for(url);
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(CacheError::Io(path, e)),
        }
    }

    /// Discard everything.
    pub fn clear(&self) -> Result<u64, CacheError> {
        let entries = self.entries()?;
        let mut removed = 0;
        for entry in &entries {
            if fs::remove_file(&entry.path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn entries(&self) -> Result<Vec<Entry>, CacheError> {
        let mut out = Vec::new();
        if !self.root.is_dir() {
            return Ok(out);
        }
        for shard in fs::read_dir(&self.root).map_err(io(&self.root))? {
            let shard = shard.map_err(io(&self.root))?.path();
            if !shard.is_dir() {
                continue;
            }
            for file in fs::read_dir(&shard).map_err(io(&shard))? {
                let file = file.map_err(io(&shard))?;
                let path = file.path();
                if path.extension().and_then(|e| e.to_str()) != Some("webp") {
                    // Skips `.part` files from an interrupted write, which are not
                    // cache entries and must not be counted against the budget.
                    continue;
                }
                let meta = file.metadata().map_err(io(&path))?;
                out.push(Entry {
                    path,
                    bytes: meta.len(),
                    // MODIFIED, not accessed. Windows disables last-access-time
                    // updates by default, so `accessed()` returns the creation time
                    // forever and eviction silently becomes least-recently-WRITTEN
                    // wearing a disguise. `touch` below rewrites the modified time on
                    // every read, which makes mtime the only honest "used" signal
                    // available without a database. Preferring `accessed()` here made
                    // this cache evict the poster on the user's home screen.
                    used: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                });
            }
        }
        Ok(out)
    }

    /// Evict least-recently-used entries until the cache is back inside its budget.
    fn evict_if_over_budget(&self) -> Result<u64, CacheError> {
        let mut entries = self.entries()?;
        let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
        if total <= self.budget_bytes {
            return Ok(0);
        }

        entries.sort_by_key(|e| e.used);
        let mut evicted = 0;
        for entry in entries {
            if total <= self.budget_bytes {
                break;
            }
            if fs::remove_file(&entry.path).is_ok() {
                total = total.saturating_sub(entry.bytes);
                evicted += 1;
            }
        }
        Ok(evicted)
    }
}

struct Entry {
    path: PathBuf,
    bytes: u64,
    used: SystemTime,
}

/// Mark a file as used now.
///
/// Windows disables last-access-time updates by default
/// (`NtfsDisableLastAccessUpdate`), so the filesystem will not record that a file was
/// read. Rewriting the modified time is crude, and it is the only signal available
/// without keeping a database of read times — which is a lot of machinery for an
/// eviction order.
fn filetime_now(path: &Path) -> std::io::Result<()> {
    let file = fs::OpenOptions::new().append(true).open(path)?;
    file.set_modified(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_cannot_escape_the_cache_directory() {
        // The whole reason filenames are hashed. A URL is a remote string and may
        // contain anything at all.
        let cache = ArtworkCache::new("/cache/root", ArtworkCache::DEFAULT_BUDGET);
        for hostile in [
            "../../../../etc/passwd",
            "https://example.test/../../secret",
            "C:\\Windows\\System32\\config\\SAM",
        ] {
            let path = cache.path_for(hostile);
            assert!(
                path.starts_with("/cache/root"),
                "{hostile:?} escaped to {path:?}"
            );
            assert_eq!(path.extension().and_then(|e| e.to_str()), Some("webp"));
        }
    }

    #[test]
    fn the_same_url_always_maps_to_the_same_file() {
        let cache = ArtworkCache::new("/root", 1);
        assert_eq!(
            cache.path_for("https://example.test/a.jpg"),
            cache.path_for("https://example.test/a.jpg")
        );
        assert_ne!(
            cache.path_for("https://example.test/a.jpg"),
            cache.path_for("https://example.test/b.jpg")
        );
    }
}
