//! The stream cache (ADR-0057, the author's answers after 524 GB of streams were found kept).
//!
//! A streamed torrent lives in the engine's own folder until it is downloaded (which moves
//! it to the downloads folder) or evicted. The cache holds at most a cap, 20 GB unless the
//! viewer chose another: past it, the streams least recently watched are deleted first,
//! never one playing. When each stream was last watched is kept in a small file beside the
//! session, `stream-cache.json`, written each time a stream is opened.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The cap when the viewer has not chosen one.
pub const DEFAULT_CAP_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// One streamed torrent as the cache sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    /// What it holds on disk.
    pub bytes: u64,
    /// When it was last opened, seconds since 1970; 0 when never recorded (kept before the
    /// cache existed: the first to go).
    pub last_used: u64,
}

/// Which streams to delete so the rest fit within `cap`: the least recently watched first,
/// never one in `playing`. If what is playing alone is over the cap, it stays, and the
/// rest still go.
pub fn evictions(entries: &[Entry], cap: u64, playing: &[String]) -> Vec<String> {
    let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
    let mut oldest: Vec<&Entry> = entries.iter().collect();
    oldest.sort_by(|a, b| a.last_used.cmp(&b.last_used).then(a.id.cmp(&b.id)));
    let mut out = Vec::new();
    for e in oldest {
        if total <= cap {
            break;
        }
        if playing.iter().any(|p| p.eq_ignore_ascii_case(&e.id)) {
            continue;
        }
        total -= e.bytes;
        out.push(e.id.clone());
    }
    out
}

/// When each stream was last opened, as kept on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LastUsed(pub HashMap<String, u64>);

impl LastUsed {
    pub fn path(session_dir: &Path) -> PathBuf {
        session_dir.join("stream-cache.json")
    }

    /// What was recorded; nothing if the file is missing or unreadable.
    pub fn load(session_dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(session_dir))
            .ok()
            .map(|text| Self(parse(&text)))
            .unwrap_or_default()
    }

    pub fn save(&self, session_dir: &Path) -> std::io::Result<()> {
        let mut ids: Vec<(&String, &u64)> = self.0.iter().collect();
        ids.sort();
        let body = ids
            .iter()
            .map(|(id, at)| format!("  \"{id}\": {at}"))
            .collect::<Vec<_>>()
            .join(",\n");
        let tmp = Self::path(session_dir).with_extension("json.tmp");
        std::fs::write(&tmp, format!("{{\n{body}\n}}\n"))?;
        std::fs::rename(tmp, Self::path(session_dir))
    }
}

/// `{"<id>": <seconds>, ...}`: written by `save`, so a flat map of hex ids to integers.
fn parse(text: &str) -> HashMap<String, u64> {
    text.trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split(',')
        .filter_map(|pair| {
            let (id, at) = pair.split_once(':')?;
            let id = id.trim().trim_matches('"');
            let at = at.trim().parse().ok()?;
            (!id.is_empty()).then(|| (id.to_ascii_lowercase(), at))
        })
        .collect()
}

/// Seconds since 1970, now.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    fn e(id: &str, gb: u64, at: u64) -> Entry {
        Entry {
            id: id.into(),
            bytes: gb * GB,
            last_used: at,
        }
    }

    #[test]
    fn the_least_recently_watched_go_first_until_the_rest_fit() {
        let all = [
            e("a", 8, 300),
            e("b", 8, 100),
            e("c", 8, 200),
            e("d", 8, 400),
        ];
        assert_eq!(evictions(&all, 20 * GB, &[]), vec!["b", "c"]);
        assert!(evictions(&all, 40 * GB, &[]).is_empty(), "within the cap");
    }

    #[test]
    fn what_is_playing_stays_even_when_it_is_the_oldest_or_too_big() {
        let all = [e("old", 8, 1), e("new", 8, 9)];
        assert_eq!(evictions(&all, 10 * GB, &["OLD".into()]), vec!["new"]);
        let huge = [e("film", 50, 5), e("other", 1, 1)];
        assert_eq!(evictions(&huge, 20 * GB, &["film".into()]), vec!["other"]);
    }

    #[test]
    fn a_stream_never_recorded_is_the_first_to_go() {
        let all = [e("recorded", 15, 1_800_000_000), e("from-before", 15, 0)];
        assert_eq!(evictions(&all, 20 * GB, &[]), vec!["from-before"]);
    }

    #[test]
    fn last_used_survives_a_save_and_a_load() {
        let dir = std::env::temp_dir().join(format!("sinephile-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut seen = LastUsed::default();
        seen.0.insert("abc123".into(), 42);
        seen.0.insert("def456".into(), 1_800_000_000);
        seen.save(&dir).unwrap();
        assert_eq!(LastUsed::load(&dir), seen);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            LastUsed::load(&dir),
            LastUsed::default(),
            "missing: nothing"
        );
    }
}
