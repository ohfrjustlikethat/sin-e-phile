//! Watching a library folder: ReadDirectoryChangesW (through `notify`) turned into
//! changes, quietened, and applied without a rescan.
//!
//! A rename may arrive whole (from and to together) or as a removal and an arrival.
//! Either way it is the same row afterwards: a whole rename moves the row directly, and an
//! arrival with the size and time of a just-removed file is recognised as that file
//! (`scan::record`). Removals are therefore applied before arrivals.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sinephile_persistence::repositories::{LibraryRepository, Upserted};
use sinephile_persistence::{Db, DbError};

use crate::identify::Cache;
use crate::scan::{identify_file, is_video, on_disk, record, Prober};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Arrived or changed.
    Upsert(PathBuf),
    Remove(PathBuf),
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
}

impl Change {
    fn key(&self) -> &Path {
        match self {
            Change::Upsert(p) | Change::Remove(p) => p,
            Change::Rename { to, .. } => to,
        }
    }

    /// Removals first, so an arrival can be recognised as a move.
    fn order(&self) -> u8 {
        match self {
            Change::Remove(_) => 0,
            Change::Rename { .. } => 1,
            Change::Upsert(_) => 2,
        }
    }
}

/// What a filesystem event means for the library.
pub fn changes_from(event: &Event) -> Vec<Change> {
    let paths = &event.paths;
    match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if paths.len() == 2 => {
            vec![Change::Rename {
                from: paths[0].clone(),
                to: paths[1].clone(),
            }]
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) | EventKind::Remove(_) => {
            paths.iter().cloned().map(Change::Remove).collect()
        }
        EventKind::Create(_) | EventKind::Modify(_) => {
            paths.iter().cloned().map(Change::Upsert).collect()
        }
        _ => Vec::new(),
    }
}

/// Changes held until their path has been quiet for a while: a film being copied in
/// fires hundreds of events, and is handled once, when the copy is done.
#[derive(Debug, Default)]
pub struct Quieten {
    pending: HashMap<PathBuf, (Change, Instant)>,
}

impl Quieten {
    pub fn push(&mut self, change: Change, now: Instant) {
        self.pending
            .insert(change.key().to_path_buf(), (change, now));
    }

    /// The changes quiet for at least `quiet`, removals first.
    pub fn ready(&mut self, now: Instant, quiet: Duration) -> Vec<Change> {
        let due: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, (_, at))| now.duration_since(*at) >= quiet)
            .map(|(p, _)| p.clone())
            .collect();
        let mut out: Vec<Change> = due
            .into_iter()
            .filter_map(|p| self.pending.remove(&p).map(|(c, _)| c))
            .collect();
        out.sort_by_key(|c| (c.order(), c.key().to_path_buf()));
        out
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Watch `root` and everything under it. The watcher stops when dropped.
pub fn watch(
    root: &Path,
    on_change: impl Fn(Change) + Send + 'static,
) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        if let Ok(event) = event {
            for change in changes_from(&event) {
                on_change(change);
            }
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    Ok(watcher)
}

/// What applying a change did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Ignored,
    Added { accepted: bool },
    Updated,
    Moved,
    Missing(usize),
}

fn under(path: &str, dir: &Path) -> bool {
    Path::new(path).starts_with(dir) && Path::new(path) != dir
}

/// Apply one change to the library.
pub async fn apply(
    db: &Db,
    folder_id: i64,
    root: &Path,
    change: &Change,
    probe: Prober,
    cache: &Cache,
) -> Result<Applied, DbError> {
    let repo = LibraryRepository::new(db);
    match change {
        Change::Upsert(path) => {
            let Some(found) = on_disk(path) else {
                return Ok(Applied::Ignored);
            };
            let (up, moved) = record(db, folder_id, &found).await?;
            if moved.is_some() {
                return Ok(Applied::Moved);
            }
            let id = up.id();
            let needs = !matches!(up, Upserted::Unchanged(_)) || repo.match_of(id).await?.is_none();
            if !needs {
                return Ok(Applied::Updated);
            }
            let Some(file) = repo.file(id).await? else {
                return Ok(Applied::Ignored);
            };
            let (accepted, _) = identify_file(db, root, &file, probe, cache).await?;
            Ok(Applied::Added { accepted })
        }
        Change::Remove(path) => {
            // A file, or a whole folder of them.
            let mut gone = 0;
            for f in repo.files_in_folder(folder_id).await? {
                if f.missing_since.is_none() && (Path::new(&f.path) == path || under(&f.path, path))
                {
                    repo.mark_missing(f.id).await?;
                    gone += 1;
                }
            }
            Ok(if gone == 0 {
                Applied::Ignored
            } else {
                Applied::Missing(gone)
            })
        }
        Change::Rename { from, to } => {
            let mut moved = 0;
            for f in repo.files_in_folder(folder_id).await? {
                let old = Path::new(&f.path);
                let new = if old == from {
                    Some(to.clone())
                } else if under(&f.path, from) {
                    old.strip_prefix(from).ok().map(|rest| to.join(rest))
                } else {
                    None
                };
                if let Some(new) = new {
                    if is_video(&new) {
                        repo.move_file(f.id, &new.to_string_lossy(), folder_id)
                            .await?;
                    } else {
                        repo.mark_missing(f.id).await?;
                    }
                    moved += 1;
                }
            }
            if moved == 0 {
                // Renamed into view from something we did not know: an arrival.
                return Box::pin(apply(
                    db,
                    folder_id,
                    root,
                    &Change::Upsert(to.clone()),
                    probe,
                    cache,
                ))
                .await;
            }
            Ok(Applied::Moved)
        }
    }
}
