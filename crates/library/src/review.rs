//! The review queue: the files identification was not sure about, what each could be,
//! and the user's answer.
//!
//! An answer is remembered under the name's pattern (`identify::pattern`), and every other
//! file waiting under the same pattern is identified again at once. One decision about
//! "Shingeki no Kyojin - 05" settles episodes 01 to 25.

use std::path::{Path, PathBuf};

use sinephile_persistence::repositories::{
    Candidate, FileMatch, LibraryRepository, LocalFile, MatchMethod, Probe, SearchRepository,
};
use sinephile_persistence::{Db, DbError};

use crate::identify::{identify, read, Cache, Evidence, Reading, ACCEPT_AT};
use crate::scan::{folders_between, identify_file};

/// A candidate as the review screen shows it: enough to tell two films apart.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewOption {
    pub candidate: Candidate,
    /// Director, or a series' creator.
    pub made_by: Option<String>,
    pub synopsis: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewItem {
    pub file: LocalFile,
    pub reading: Reading,
    /// The folders between the library root and the file.
    pub folders: Vec<String>,
    pub options: Vec<ReviewOption>,
    /// Other waiting files named the same way, which one answer also settles.
    pub same_name: usize,
}

/// What an answer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    /// The episode or film the file was matched to.
    pub media_item_id: i64,
    /// Other waiting files the same answer settled.
    pub settled: usize,
}

fn no_probe(_: &Path) -> Option<Probe> {
    None
}

async fn root_of(db: &Db, file: &LocalFile) -> Result<PathBuf, DbError> {
    let folders = LibraryRepository::new(db).folders().await?;
    Ok(folders
        .into_iter()
        .find(|f| Some(f.id) == file.folder_id)
        .map(|f| PathBuf::from(f.path))
        .unwrap_or_default())
}

fn evidence(root: &Path, file: &LocalFile) -> Evidence {
    let path = PathBuf::from(&file.path);
    Evidence {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        folders: folders_between(root, &path),
        duration_minutes: file.duration_seconds.map(|s| s as f64 / 60.0),
    }
}

/// The waiting files, in path order (a series' episodes together), with what each could be.
pub async fn queue(db: &Db, limit: i64) -> Result<Vec<ReviewItem>, DbError> {
    let repo = LibraryRepository::new(db);
    let search = SearchRepository::new(db);
    let waiting = repo.review_queue(ACCEPT_AT, limit.max(1) * 4).await?;
    let mut roots = std::collections::HashMap::new();
    let mut readings = Vec::with_capacity(waiting.len());
    for f in &waiting {
        let root = match roots.get(&f.folder_id) {
            Some(r) => Clone::clone(r),
            None => {
                let r = root_of(db, f).await?;
                roots.insert(f.folder_id, r.clone());
                r
            }
        };
        readings.push((root.clone(), read(&evidence(&root, f))));
    }
    let mut out = Vec::new();
    for (i, f) in waiting.iter().enumerate().take(limit.max(0) as usize) {
        let (root, reading) = &readings[i];
        let candidates = repo.candidates(f.id).await?;
        let ids: Vec<i64> = candidates.iter().map(|c| c.media_item_id).collect();
        let details = search.details(&ids).await?;
        let options = candidates
            .into_iter()
            .map(|c| {
                let d = details.iter().find(|d| d.media_item_id == c.media_item_id);
                ReviewOption {
                    made_by: d.and_then(|d| d.made_by.clone()),
                    synopsis: d.and_then(|d| d.synopsis.clone()),
                    candidate: c,
                }
            })
            .collect();
        let same_name = readings
            .iter()
            .enumerate()
            .filter(|(j, (_, r))| *j != i && r.pattern == reading.pattern)
            .count();
        out.push(ReviewItem {
            folders: folders_between(root, Path::new(&f.path)),
            file: f.clone(),
            reading: reading.clone(),
            options,
            same_name,
        });
    }
    Ok(out)
}

/// The user's answer: this file is `media_item_id` (a film or a series). Remembered for
/// every name read the same way, and applied to the others waiting now.
pub async fn resolve(db: &Db, file_id: i64, media_item_id: i64) -> Result<Resolved, DbError> {
    let repo = LibraryRepository::new(db);
    let Some(file) = repo.file(file_id).await? else {
        return Ok(Resolved {
            media_item_id,
            settled: 0,
        });
    };
    let root = root_of(db, &file).await?;
    let ev = evidence(&root, &file);
    let reading = read(&ev);
    repo.remember(&reading.pattern, media_item_id).await?;

    // Identified again with the answer remembered: a series answer finds the episode.
    let id = identify(db, &ev).await?;
    let (item, covers) = match (id.best(), id.episode) {
        (Some(b), Some(e)) if b.media_item_id == media_item_id => (e.episode_id, e.covers),
        _ => (media_item_id, 1),
    };
    repo.set_match(
        file_id,
        &FileMatch {
            media_item_id: item,
            confidence: 1.0,
            method: MatchMethod::Manual,
            covers,
            confirmed: true,
        },
    )
    .await?;
    repo.set_candidates(file_id, &[]).await?;

    // Everything else waiting under the same name.
    let cache = Cache::default();
    let mut settled = 0;
    for other in repo.review_queue(ACCEPT_AT, 100_000).await? {
        if other.id == file_id {
            continue;
        }
        let other_root = root_of(db, &other).await?;
        if read(&evidence(&other_root, &other)).pattern != reading.pattern {
            continue;
        }
        let (accepted, _) = identify_file(db, &other_root, &other, no_probe, &cache).await?;
        settled += usize::from(accepted);
    }
    Ok(Resolved {
        media_item_id: item,
        settled,
    })
}

/// "Not a film or episode": never asked about again.
pub async fn not_a_title(db: &Db, file_id: i64) -> Result<(), DbError> {
    let repo = LibraryRepository::new(db);
    repo.clear_match(file_id).await?;
    repo.set_ignored(file_id, true).await
}

/// A synopsis cut to its first sentence, at most `max` characters, for a card.
pub fn first_sentence(text: &str, max: usize) -> String {
    let text = text.trim();
    // "Dr.", "St.", "J." do not end a sentence.
    let abbreviation = |before: &str| {
        let word = before.rsplit(' ').next().unwrap_or("");
        word.chars().count() == 1
            || matches!(
                word,
                "Dr" | "Mr" | "Mrs" | "Ms" | "St" | "Jr" | "Sr" | "vs" | "No" | "Mt"
            )
    };
    let end = text
        .char_indices()
        .find(|(i, c)| {
            matches!(c, '.' | '!' | '?')
                && text[i + c.len_utf8()..].starts_with(' ')
                && !(*c == '.' && abbreviation(&text[..*i]))
        })
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(text.len());
    let sentence = &text[..end];
    if sentence.chars().count() <= max {
        return sentence.to_string();
    }
    let cut: String = sentence.chars().take(max).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", cut.trim_end_matches([',', ';', ':']))
}

/// How the name reads, in words: "Episode 5 of season 2", "Episode 59", "".
pub fn episode_words(r: &Reading) -> String {
    match (
        r.season,
        r.episodes.first(),
        r.parsed.absolute_episodes.first(),
    ) {
        (Some(s), Some(e), _) if r.episodes.len() > 1 => format!(
            "Episodes {e}–{} of season {s}",
            r.episodes.last().unwrap_or(e)
        ),
        (Some(s), Some(e), _) => format!("Episode {e} of season {s}"),
        (Some(s), None, None) => format!("Season {s}"),
        (_, _, Some(a)) => format!("Episode {a}"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identify::{read, Evidence};

    #[test]
    fn a_synopsis_is_cut_at_its_first_sentence_and_a_word() {
        assert_eq!(
            first_sentence("A thief plans one last job. The police close in.", 80),
            "A thief plans one last job."
        );
        assert_eq!(
            first_sentence("Dr. Who travels. Then more.", 80),
            "Dr. Who travels."
        );
        assert_eq!(
            first_sentence("J. Edgar Hoover rises. He falls.", 80),
            "J. Edgar Hoover rises."
        );
        assert_eq!(first_sentence("one two three four five", 12), "one two…");
        assert_eq!(first_sentence("  ", 10), "");
    }

    #[test]
    fn the_episode_reads_in_words() {
        let r = |name: &str| {
            read(&Evidence {
                name: name.into(),
                ..Default::default()
            })
        };
        assert_eq!(
            episode_words(&r("Show.S02E05.mkv")),
            "Episode 5 of season 2"
        );
        assert_eq!(
            episode_words(&r("Show.S02E05E06.mkv")),
            "Episodes 5–6 of season 2"
        );
        assert_eq!(episode_words(&r("[Grp] Show - 059.mkv")), "Episode 59");
        assert_eq!(episode_words(&r("Film.1995.mkv")), "");
    }
}
