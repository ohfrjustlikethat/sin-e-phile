//! The local library (SPEC.md Phase 12, migrations 0004 and 0021): the folders the user
//! added, the files found in them, what each file was identified as, the review queue
//! for the ones that were not certain, and the user's own decisions.

use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFolder {
    pub id: i64,
    pub path: String,
    pub added_at: String,
    pub scanned_at: Option<String>,
    /// Files present (not missing) under it.
    pub files: i64,
}

/// A file on disk, and what is known about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub path: String,
    pub size_bytes: i64,
    pub modified_at: Option<String>,
    pub content_hash: Option<String>,
    pub duration_seconds: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// ISO 639-1, from the probe.
    pub audio_languages: Vec<String>,
    pub container_title: Option<String>,
    pub probed: bool,
    pub missing_since: Option<String>,
}

/// A file as the scanner finds it on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLocalFile<'a> {
    pub folder_id: i64,
    pub path: &'a str,
    pub size_bytes: i64,
    pub modified_at: Option<&'a str>,
}

/// What FFprobe found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub duration_seconds: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub audio_languages: Vec<String>,
    pub container_title: Option<String>,
}

/// What `upsert_file` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upserted {
    New(i64),
    /// Same size and modification time: nothing to redo.
    Unchanged(i64),
    /// Different contents at the same path: probe and identify again.
    Changed(i64),
}

impl Upserted {
    pub fn id(self) -> i64 {
        match self {
            Upserted::New(id) | Upserted::Unchanged(id) | Upserted::Changed(id) => id,
        }
    }
}

/// How a match was made (0004's CHECK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMethod {
    Filename,
    Hash,
    Metadata,
    Manual,
}

impl MatchMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchMethod::Filename => "filename",
            MatchMethod::Hash => "hash",
            MatchMethod::Metadata => "metadata",
            MatchMethod::Manual => "manual",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "hash" => MatchMethod::Hash,
            "metadata" => MatchMethod::Metadata,
            "manual" => MatchMethod::Manual,
            _ => MatchMethod::Filename,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileMatch {
    pub media_item_id: i64,
    pub confidence: f64,
    pub method: MatchMethod,
    /// Episodes the file holds, from `media_item_id` on.
    pub covers: i64,
    /// A human decided. Never overwritten by a scan.
    pub confirmed: bool,
}

/// A candidate offered in the review queue.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub media_item_id: i64,
    pub kind: String,
    pub primary_title: String,
    pub release_year: Option<i64>,
    pub runtime_minutes: Option<i64>,
    pub score: f64,
}

/// A catalogue item as identification weighs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFacts {
    pub media_item_id: i64,
    pub kind: String,
    pub primary_title: String,
    pub release_year: Option<i64>,
    pub runtime_minutes: Option<i64>,
    pub rating_votes: Option<i64>,
    /// Every title variant, the primary included.
    pub titles: Vec<String>,
}

const FILE_COLUMNS: &str = "id, folder_id, path, size_bytes, modified_at, content_hash,
    duration_seconds, video_codec, audio_codec, width, height, audio_languages,
    container_title, probed_at, missing_since";

fn file(row: &SqliteRow) -> Result<LocalFile, sqlx::Error> {
    let langs: Option<String> = row.try_get("audio_languages")?;
    Ok(LocalFile {
        id: row.try_get("id")?,
        folder_id: row.try_get("folder_id")?,
        path: row.try_get("path")?,
        size_bytes: row.try_get("size_bytes")?,
        modified_at: row.try_get("modified_at")?,
        content_hash: row.try_get("content_hash")?,
        duration_seconds: row.try_get("duration_seconds")?,
        video_codec: row.try_get("video_codec")?,
        audio_codec: row.try_get("audio_codec")?,
        width: row.try_get("width")?,
        height: row.try_get("height")?,
        audio_languages: langs
            .filter(|l| !l.is_empty())
            .map(|l| l.split(',').map(str::to_string).collect())
            .unwrap_or_default(),
        container_title: row.try_get("container_title")?,
        probed: row.try_get::<Option<String>, _>("probed_at")?.is_some(),
        missing_since: row.try_get("missing_since")?,
    })
}

pub struct LibraryRepository<'a> {
    db: &'a Db,
}

impl<'a> LibraryRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    // ---- folders ----------------------------------------------------------------

    /// Add a folder, or return the one already added at that path.
    pub async fn add_folder(&self, path: &str) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO library_folders (path) VALUES (?1)
             ON CONFLICT (path) DO UPDATE SET path = excluded.path
             RETURNING id",
        )
        .bind(path)
        .fetch_one(self.db.pool())
        .await?)
    }

    pub async fn folders(&self) -> Result<Vec<LibraryFolder>, DbError> {
        let rows = sqlx::query(
            "SELECT f.id, f.path, f.added_at, f.scanned_at,
                    (SELECT COUNT(*) FROM local_files l
                      WHERE l.folder_id = f.id AND l.missing_since IS NULL) AS files
             FROM library_folders f ORDER BY f.path",
        )
        .fetch_all(self.db.pool())
        .await?;
        rows.iter()
            .map(|r| {
                Ok(LibraryFolder {
                    id: r.try_get("id")?,
                    path: r.try_get("path")?,
                    added_at: r.try_get("added_at")?,
                    scanned_at: r.try_get("scanned_at")?,
                    files: r.try_get("files")?,
                })
            })
            .collect::<Result<_, sqlx::Error>>()
            .map_err(Into::into)
    }

    /// Remove a folder and, by 0021's trigger, its files and their matches.
    pub async fn remove_folder(&self, id: i64) -> Result<(), DbError> {
        sqlx::query("DELETE FROM library_folders WHERE id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    pub async fn mark_folder_scanned(&self, id: i64) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE library_folders SET scanned_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1",
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    // ---- files ------------------------------------------------------------------

    /// Record a file found on disk. A file whose size or modification time changed is
    /// reset: probed and identified again, unless a human confirmed its match.
    pub async fn upsert_file(&self, f: &NewLocalFile<'_>) -> Result<Upserted, DbError> {
        let mut tx = self.db.pool().begin().await?;
        let existing =
            sqlx::query("SELECT id, size_bytes, modified_at FROM local_files WHERE path = ?1")
                .bind(f.path)
                .fetch_optional(&mut *tx)
                .await?;
        let outcome = match existing {
            None => {
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO local_files (folder_id, path, size_bytes, modified_at)
                     VALUES (?1, ?2, ?3, ?4) RETURNING id",
                )
                .bind(f.folder_id)
                .bind(f.path)
                .bind(f.size_bytes)
                .bind(f.modified_at)
                .fetch_one(&mut *tx)
                .await?;
                Upserted::New(id)
            }
            Some(row) => {
                let id: i64 = row.try_get("id")?;
                let size: i64 = row.try_get("size_bytes")?;
                let modified: Option<String> = row.try_get("modified_at")?;
                if size == f.size_bytes && modified.as_deref() == f.modified_at {
                    sqlx::query(
                        "UPDATE local_files SET missing_since = NULL, folder_id = ?2 WHERE id = ?1",
                    )
                    .bind(id)
                    .bind(f.folder_id)
                    .execute(&mut *tx)
                    .await?;
                    Upserted::Unchanged(id)
                } else {
                    sqlx::query(
                        "UPDATE local_files
                         SET size_bytes = ?2, modified_at = ?3, folder_id = ?4, missing_since = NULL,
                             content_hash = NULL, duration_seconds = NULL, video_codec = NULL,
                             audio_codec = NULL, width = NULL, height = NULL,
                             audio_languages = NULL, container_title = NULL, probed_at = NULL,
                             scanned_at = datetime('now')
                         WHERE id = ?1",
                    )
                    .bind(id)
                    .bind(f.size_bytes)
                    .bind(f.modified_at)
                    .bind(f.folder_id)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "DELETE FROM local_file_matches WHERE local_file_id = ?1 AND is_confirmed = 0",
                    )
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query("DELETE FROM review_candidates WHERE local_file_id = ?1")
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                    Upserted::Changed(id)
                }
            }
        };
        tx.commit().await?;
        Ok(outcome)
    }

    pub async fn file(&self, id: i64) -> Result<Option<LocalFile>, DbError> {
        let row = sqlx::query(&format!(
            "SELECT {FILE_COLUMNS} FROM local_files WHERE id = ?1"
        ))
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.as_ref().map(file).transpose()?)
    }

    pub async fn file_by_path(&self, path: &str) -> Result<Option<LocalFile>, DbError> {
        let row = sqlx::query(&format!(
            "SELECT {FILE_COLUMNS} FROM local_files WHERE path = ?1"
        ))
        .bind(path)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.as_ref().map(file).transpose()?)
    }

    /// Every file under a folder, missing ones included (the scanner diffs against it).
    pub async fn files_in_folder(&self, folder_id: i64) -> Result<Vec<LocalFile>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {FILE_COLUMNS} FROM local_files WHERE folder_id = ?1 ORDER BY path"
        ))
        .bind(folder_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.iter().map(file).collect::<Result<_, _>>()?)
    }

    pub async fn set_probe(&self, id: i64, p: &Probe) -> Result<(), DbError> {
        let langs = (!p.audio_languages.is_empty()).then(|| p.audio_languages.join(","));
        sqlx::query(
            "UPDATE local_files
             SET duration_seconds = ?2, video_codec = ?3, audio_codec = ?4, width = ?5,
                 height = ?6, audio_languages = ?7, container_title = ?8,
                 probed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1",
        )
        .bind(id)
        .bind(p.duration_seconds)
        .bind(&p.video_codec)
        .bind(&p.audio_codec)
        .bind(p.width)
        .bind(p.height)
        .bind(langs)
        .bind(&p.container_title)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn set_content_hash(&self, id: i64, hash: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE local_files SET content_hash = ?2 WHERE id = ?1")
            .bind(id)
            .bind(hash)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// The file is gone from where it was. It stays (with its match) until the scan
    /// ends, in case it turns up elsewhere.
    pub async fn mark_missing(&self, id: i64) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE local_files SET missing_since = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1 AND missing_since IS NULL",
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Missing files of this size: what a new file might have been before it moved.
    pub async fn missing_with_size(&self, size_bytes: i64) -> Result<Vec<LocalFile>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {FILE_COLUMNS} FROM local_files
             WHERE size_bytes = ?1 AND missing_since IS NOT NULL ORDER BY id"
        ))
        .bind(size_bytes)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.iter().map(file).collect::<Result<_, _>>()?)
    }

    /// A rename or a move: the same row, so its probe and its match survive (E4). A row
    /// already made for the new path (the scan saw it before it knew) is the same file,
    /// and is removed.
    pub async fn move_file(&self, id: i64, new_path: &str, folder_id: i64) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM local_files WHERE path = ?1 AND id <> ?2")
            .bind(new_path)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE local_files SET path = ?2, folder_id = ?3, missing_since = NULL WHERE id = ?1",
        )
        .bind(id)
        .bind(new_path)
        .bind(folder_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Remove a folder's files that are still missing at the end of a scan. Returns how
    /// many.
    pub async fn purge_missing(&self, folder_id: i64) -> Result<u64, DbError> {
        Ok(sqlx::query(
            "DELETE FROM local_files WHERE folder_id = ?1 AND missing_since IS NOT NULL",
        )
        .bind(folder_id)
        .execute(self.db.pool())
        .await?
        .rows_affected())
    }

    pub async fn delete_file(&self, id: i64) -> Result<(), DbError> {
        sqlx::query("DELETE FROM local_files WHERE id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    // ---- matches ----------------------------------------------------------------

    /// Record what a file is. A confirmed match is never replaced by an unconfirmed one:
    /// a human decision outranks the matcher permanently (0004).
    pub async fn set_match(&self, id: i64, m: &FileMatch) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO local_file_matches
                 (local_file_id, media_item_id, confidence, method, covers, is_confirmed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (local_file_id) DO UPDATE
               SET media_item_id = excluded.media_item_id, confidence = excluded.confidence,
                   method = excluded.method, covers = excluded.covers,
                   is_confirmed = excluded.is_confirmed, matched_at = datetime('now')
               WHERE local_file_matches.is_confirmed = 0 OR excluded.is_confirmed = 1",
        )
        .bind(id)
        .bind(m.media_item_id)
        .bind(m.confidence.clamp(0.0, 1.0))
        .bind(m.method.as_str())
        .bind(m.covers.max(1))
        .bind(m.confirmed)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn match_of(&self, id: i64) -> Result<Option<FileMatch>, DbError> {
        let row = sqlx::query(
            "SELECT media_item_id, confidence, method, covers, is_confirmed
             FROM local_file_matches WHERE local_file_id = ?1",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        let Some(r) = row else { return Ok(None) };
        Ok(Some(FileMatch {
            media_item_id: r.try_get("media_item_id")?,
            confidence: r.try_get("confidence")?,
            method: MatchMethod::parse(&r.try_get::<String, _>("method")?),
            covers: r.try_get("covers")?,
            confirmed: r.try_get::<i64, _>("is_confirmed")? == 1,
        }))
    }

    /// "Not this": forget the match, confirmed or not.
    pub async fn clear_match(&self, id: i64) -> Result<(), DbError> {
        sqlx::query("DELETE FROM local_file_matches WHERE local_file_id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Replace what the review queue offers for a file: (media item, score), best first.
    pub async fn set_candidates(&self, id: i64, candidates: &[(i64, f64)]) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM review_candidates WHERE local_file_id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for (rank, (item, score)) in candidates.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO review_candidates (local_file_id, media_item_id, score, rank)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(id)
            .bind(item)
            .bind(score.clamp(0.0, 1.0))
            .bind(rank as i64)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn candidates(&self, id: i64) -> Result<Vec<Candidate>, DbError> {
        let rows = sqlx::query(
            "SELECT c.media_item_id, m.kind, m.primary_title, m.release_year,
                    m.runtime_minutes, c.score
             FROM review_candidates c JOIN media_items m ON m.id = c.media_item_id
             WHERE c.local_file_id = ?1 ORDER BY c.rank",
        )
        .bind(id)
        .fetch_all(self.db.pool())
        .await?;
        rows.iter()
            .map(|r| {
                Ok(Candidate {
                    media_item_id: r.try_get("media_item_id")?,
                    kind: r.try_get("kind")?,
                    primary_title: r.try_get("primary_title")?,
                    release_year: r.try_get("release_year")?,
                    runtime_minutes: r.try_get("runtime_minutes")?,
                    score: r.try_get("score")?,
                })
            })
            .collect::<Result<_, sqlx::Error>>()
            .map_err(Into::into)
    }

    /// Files that need a human: present, and either unmatched or matched below
    /// `threshold` and not confirmed. In path order, so a series' episodes sit together.
    pub async fn review_queue(
        &self,
        threshold: f64,
        limit: i64,
    ) -> Result<Vec<LocalFile>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {FILE_COLUMNS} FROM local_files l
             WHERE l.missing_since IS NULL AND NOT EXISTS (
                 SELECT 1 FROM local_file_matches m
                 WHERE m.local_file_id = l.id AND (m.is_confirmed = 1 OR m.confidence >= ?1))
             ORDER BY l.path LIMIT ?2"
        ))
        .bind(threshold)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.iter().map(file).collect::<Result<_, _>>()?)
    }

    pub async fn review_count(&self, threshold: f64) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM local_files l
             WHERE l.missing_since IS NULL AND NOT EXISTS (
                 SELECT 1 FROM local_file_matches m
                 WHERE m.local_file_id = l.id AND (m.is_confirmed = 1 OR m.confidence >= ?1))",
        )
        .bind(threshold)
        .fetch_one(self.db.pool())
        .await?)
    }

    /// Present files matched to this item: the copies a film's page can play.
    pub async fn files_for_item(&self, media_item_id: i64) -> Result<Vec<LocalFile>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {FILE_COLUMNS} FROM local_files l
             WHERE l.missing_since IS NULL AND l.id IN (
                 SELECT local_file_id FROM local_file_matches WHERE media_item_id = ?1)
             ORDER BY l.size_bytes DESC"
        ))
        .bind(media_item_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.iter().map(file).collect::<Result<_, _>>()?)
    }

    /// What identification weighs for each candidate, in two queries however many there
    /// are: its facts, and every title it is known by.
    pub async fn candidate_facts(&self, ids: &[i64]) -> Result<Vec<CandidateFacts>, DbError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let rows = sqlx::query(&format!(
            "SELECT id, kind, primary_title, release_year, runtime_minutes, rating_votes
             FROM media_items WHERE id IN ({list})"
        ))
        .fetch_all(self.db.pool())
        .await?;
        let titles: Vec<(i64, String)> = sqlx::query_as(&format!(
            "SELECT media_item_id, title FROM titles WHERE media_item_id IN ({list})"
        ))
        .fetch_all(self.db.pool())
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            let id: i64 = r.try_get("id")?;
            let primary: String = r.try_get("primary_title")?;
            let mut known: Vec<String> = titles
                .iter()
                .filter(|(m, _)| *m == id)
                .map(|(_, t)| t.clone())
                .collect();
            if !known.contains(&primary) {
                known.push(primary.clone());
            }
            out.push(CandidateFacts {
                media_item_id: id,
                kind: r.try_get("kind")?,
                primary_title: primary,
                release_year: r.try_get("release_year")?,
                runtime_minutes: r.try_get("runtime_minutes")?,
                rating_votes: r.try_get("rating_votes")?,
                titles: known,
            });
        }
        // In the order asked for.
        out.sort_by_key(|f| ids.iter().position(|i| *i == f.media_item_id));
        Ok(out)
    }

    // ---- the user's decisions ---------------------------------------------------

    /// Remember that names reading `pattern` are `media_item_id`.
    pub async fn remember(&self, pattern: &str, media_item_id: i64) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO match_memory (pattern, media_item_id) VALUES (?1, ?2)
             ON CONFLICT (pattern) DO UPDATE
               SET media_item_id = excluded.media_item_id,
                   decided_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        )
        .bind(pattern)
        .bind(media_item_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn recall(&self, pattern: &str) -> Result<Option<i64>, DbError> {
        Ok(
            sqlx::query_scalar("SELECT media_item_id FROM match_memory WHERE pattern = ?1")
                .bind(pattern)
                .fetch_optional(self.db.pool())
                .await?,
        )
    }
}
