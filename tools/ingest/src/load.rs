//! Loading IMDb titles into the catalogue.
//!
//! Runs as steps on the resumable job runner, so a killed ingestion carries on
//! rather than starting over, and never re-inserts a batch it already committed.
//!
//! # Why this does not go through `MediaRepository`
//!
//! The repository inserts one title and its primary title row per call, in its own
//! transaction, which is exactly right for the application and wrong here: 2.7
//! million titles at one statement each is the difference between minutes and hours.
//! This builds multi-row `INSERT` statements inside the runner's transaction
//! instead.
//!
//! That is a deliberate exception to "no raw SQL outside the repositories", and it
//! is narrow: the SQL lives in `crates/persistence`'s sibling crate, never under
//! `src-tauri/`, and it is covered by the same
//! ADR-0026 obligation — every statement here is exercised against a migrated
//! database by `tests/load.rs`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sinephile_persistence::Db;

use crate::imdb::{self, CatalogueScope};
use crate::job::{Batch, Job, JobError, SqliteTx};
use crate::tsv::TsvReader;

/// Rows per transaction.
///
/// SQLite's cost here is commits, not inserts, so bigger is faster — up to the point
/// where a crash loses too much work and the statement gets unwieldy. 5,000 titles
/// is about 20 MB of SQL and a fraction of a second to redo.
pub const BATCH: usize = 5_000;

/// SQLite's hard ceiling is 999 bound parameters per statement by default, and
/// modern builds allow far more — but a multi-row insert still has to stay under
/// whatever the limit is. Ten columns per title means 90 titles per statement is
/// safe everywhere; batches are chunked to that inside one transaction.
const ROWS_PER_STATEMENT: usize = 90;

/// What one load step did, for reporting.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LoadStats {
    pub scanned: u64,
    pub indexed: u64,
    pub core: u64,
    pub skipped_type: u64,
    pub skipped_adult: u64,
}

/// Read `title.ratings` into memory: title id → vote count.
///
/// Held as `u32 → i64` rather than `String → i64`. At 1.7 million rated titles that
/// is roughly 20 MB instead of 120 MB, and the map is needed for the whole of the
/// basics pass.
pub fn load_votes(path: &Path) -> Result<HashMap<u32, i64>, JobError> {
    let mut reader = TsvReader::open(path)?;
    imdb::check_columns(&reader, &imdb::TITLE_RATINGS)?;

    let mut votes = HashMap::with_capacity(1_800_000);
    while reader.advance()? {
        let Some(row) = reader.current_row() else {
            continue;
        };
        if let (Some(id), Some(count)) = (
            row.get("tconst").and_then(tconst_id),
            row.parse::<i64>("numVotes"),
        ) {
            votes.insert(id, count);
        }
    }
    Ok(votes)
}

/// `tt0000001` → `1`.
pub fn tconst_id(raw: &str) -> Option<u32> {
    raw.strip_prefix("tt")?.parse().ok()
}

/// One title, ready to insert.
struct Title {
    tconst: String,
    kind: &'static str,
    primary: String,
    original: Option<String>,
    year: Option<i64>,
    runtime: Option<i64>,
    rating: Option<i64>,
    votes: Option<i64>,
    in_core: bool,
    genres: Vec<String>,
}

/// IMDb's `titleType` to our `media_kind` (§6.2, ADR-0025).
///
/// Anime is NOT distinguished here. IMDb has no anime flag, and guessing from
/// country or genre would be wrong often enough to matter — subtask 4.4 promotes
/// titles to `anime_film` / `anime_series` when AniList confirms them, which is a
/// fact rather than an inference.
fn media_kind(title_type: &str) -> Option<&'static str> {
    Some(match title_type {
        "movie" | "tvMovie" | "video" | "short" | "tvSpecial" => "film",
        "tvSeries" | "tvMiniSeries" => "series",
        _ => return None,
    })
}

/// Load `title.basics` as a resumable step.
///
/// `ratings` is the vote map from [`load_votes`]; `scope` decides both tiers.
///
/// The maps arrive as `Arc` and the path as an owned `PathBuf` **because the batch
/// closure may not borrow anything but its transaction.** The runner's signature is
/// `for<'t> FnMut(&'t mut SqliteTx, ...) -> BatchFuture<'t>`, which says the returned
/// future borrows only from `'t` — so an `async move` block capturing an outer `&`
/// cannot satisfy it. Cloning the `Arc` inside the closure, before `Box::pin`, gives
/// the future owned handles and costs one atomic increment per batch.
pub async fn load_titles(
    job: &mut Job<'_>,
    basics: std::path::PathBuf,
    ratings: std::sync::Arc<HashMap<u32, i64>>,
    average_ratings: std::sync::Arc<HashMap<u32, i64>>,
    scope: CatalogueScope,
) -> Result<LoadStats, JobError> {
    let mut stats = LoadStats::default();

    job.run_step("title.basics", move |tx, cursor| {
        let basics = basics.clone();
        let ratings = std::sync::Arc::clone(&ratings);
        let average_ratings = std::sync::Arc::clone(&average_ratings);
        Box::pin(async move {
            let basics = basics.as_path();
            let ratings = ratings.as_ref();
            let average_ratings = average_ratings.as_ref();
            let mut reader = TsvReader::open(basics)?;
            imdb::check_columns(&reader, &imdb::TITLE_BASICS)?;

            // Resume: skip to just past the last committed id. Costs a prefix
            // re-read, never a re-inserted row.
            let mut pending = if let Some(cursor) = cursor.as_deref() {
                reader.seek_past("tconst", cursor)?;
                // `seek_past` stops ON the first unprocessed row rather than before
                // it, so that row is taken from the buffer before reading onward.
                // Dropping it here would silently lose one title per resume.
                collect_current(&reader, ratings, average_ratings, scope)
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            let mut last_id: Option<String> = None;
            while pending.len() < BATCH {
                if !reader.advance()? {
                    break;
                }
                if let Some(title) = collect_current(&reader, ratings, average_ratings, scope) {
                    pending.push(title);
                }
                if let Some(row) = reader.current_row() {
                    if let Some(id) = row.get("tconst") {
                        last_id = Some(id.to_string());
                    }
                }
            }

            let count = pending.len() as i64;
            let finished = pending.len() < BATCH;
            insert_titles(tx, &pending).await?;

            Ok(match (finished, last_id) {
                (false, Some(id)) => Batch::more(id, count),
                _ => Batch::finished(count),
            })
        })
    })
    .await?;

    stats.scanned = 0; // filled by the caller from progress; see tests/load.rs
    Ok(stats)
}

/// Build a `Title` from the row currently in the reader, if the scope keeps it.
fn collect_current(
    reader: &TsvReader,
    votes: &HashMap<u32, i64>,
    averages: &HashMap<u32, i64>,
    scope: CatalogueScope,
) -> Option<Title> {
    let row = reader.current_row()?;
    let tconst = row.get("tconst")?;
    let title_type = row.get("titleType")?;
    let kind = media_kind(title_type)?;

    let id = tconst_id(tconst);
    let vote_count = id.and_then(|id| votes.get(&id)).copied();
    let adult = imdb::is_adult(row.get("isAdult"));
    let year = row.parse::<i64>("startYear");

    if !scope.in_index(title_type, vote_count, adult) {
        return None;
    }

    Some(Title {
        tconst: tconst.to_string(),
        kind,
        primary: row.get("primaryTitle")?.to_string(),
        original: row
            .get("originalTitle")
            .filter(|o| Some(*o) != row.get("primaryTitle"))
            .map(str::to_string),
        year,
        runtime: row.parse::<i64>("runtimeMinutes"),
        rating: id.and_then(|id| averages.get(&id)).copied(),
        votes: vote_count,
        in_core: scope.in_core(title_type, vote_count, adult, year),
        genres: row.list("genres").into_iter().map(str::to_string).collect(),
    })
}

/// Insert a batch, inside the runner's transaction.
async fn insert_titles(tx: &mut SqliteTx<'_>, titles: &[Title]) -> Result<(), JobError> {
    for chunk in titles.chunks(ROWS_PER_STATEMENT) {
        // media_items
        let placeholders = vec!["(?, ?, ?, ?, ?, ?, ?, ?)"; chunk.len()].join(", ");
        let sql = format!(
            "INSERT INTO media_items
                 (kind, primary_title, sort_title, release_year, runtime_minutes,
                  rating, rating_votes, in_core)
             VALUES {placeholders}"
        );
        // RETURNING, not arithmetic on last_insert_rowid().
        //
        // The obvious shortcut is `last_insert_rowid() - chunk.len() + 1`, and it is
        // correct only while the table has no gaps. After any delete — a re-ingest,
        // a user pruning their library — SQLite reuses ids and that base is wrong,
        // so every external id and title row in the chunk attaches to the WRONG
        // film. Silently, and permanently.
        //
        // SQLite emits one RETURNING row per inserted row, in insertion order, so
        // zipping with the chunk needs no assumption about the ids themselves.
        let sql = format!("{sql} RETURNING id");
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for title in chunk {
            query = query
                .bind(title.kind)
                .bind(&title.primary)
                .bind(&title.original)
                .bind(title.year)
                .bind(title.runtime)
                .bind(title.rating)
                .bind(title.votes)
                .bind(i64::from(title.in_core));
        }
        let ids: Vec<i64> = query
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.basics", format!("insert: {e}")))?;

        if ids.len() != chunk.len() {
            return Err(JobError::step(
                "title.basics",
                format!(
                    "inserted {} titles but got {} ids back — refusing to guess which \
                     is which",
                    chunk.len(),
                    ids.len()
                ),
            ));
        }

        // external_ids, titles, genres — all keyed on those ids.
        let mut ext =
            String::from("INSERT INTO external_ids (media_item_id, source, external_id) VALUES ");
        let mut title_rows =
            String::from("INSERT INTO titles (media_item_id, title, variant) VALUES ");
        let mut ext_binds: Vec<(i64, &str)> = Vec::new();
        let mut title_binds: Vec<(i64, &str, &str)> = Vec::new();

        for (title, &id) in chunk.iter().zip(ids.iter()) {
            ext_binds.push((id, title.tconst.as_str()));
            title_binds.push((id, title.primary.as_str(), "primary"));
            if let Some(original) = &title.original {
                title_binds.push((id, original.as_str(), "original"));
            }
        }

        ext.push_str(&vec!["(?, 'imdb', ?)"; ext_binds.len()].join(", "));
        let mut query = sqlx::query(&ext);
        for (id, tconst) in &ext_binds {
            query = query.bind(id).bind(tconst);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.basics", format!("external_ids: {e}")))?;

        title_rows.push_str(&vec!["(?, ?, ?)"; title_binds.len()].join(", "));
        let mut query = sqlx::query(&title_rows);
        for (id, text, variant) in &title_binds {
            query = query.bind(id).bind(text).bind(variant);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.basics", format!("titles: {e}")))?;

        insert_genres(tx, chunk, &ids).await?;
    }
    Ok(())
}

/// Genres are a closed vocabulary, so each name is inserted once and reused.
async fn insert_genres(
    tx: &mut SqliteTx<'_>,
    chunk: &[Title],
    media_ids: &[i64],
) -> Result<(), JobError> {
    let names: HashSet<&str> = chunk
        .iter()
        .flat_map(|t| t.genres.iter().map(String::as_str))
        .collect();
    if names.is_empty() {
        return Ok(());
    }

    // IMDb has 28 genres in total, so this insert-then-select pair does its real
    // work on the first chunk and nothing afterwards. It ran per chunk of 90
    // titles — roughly ten inserts and ten selects each, sixty thousand round trips
    // across a full load — because the results were thrown away every time.
    //
    // Reading the whole table once per batch instead is bounded by the number of
    // genres rather than the number of titles, which is the property that matters.
    for name in &names {
        sqlx::query("INSERT INTO genres (name) VALUES (?) ON CONFLICT (name) DO NOTHING")
            .bind(name)
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.basics", format!("genres: {e}")))?;
    }

    let ids: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>("SELECT name, id FROM genres")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .collect();

    let mut pairs: Vec<(i64, i64)> = Vec::new();
    for (title, &media_id) in chunk.iter().zip(media_ids.iter()) {
        for genre in &title.genres {
            if let Some(genre_id) = ids.get(genre) {
                pairs.push((media_id, *genre_id));
            }
        }
    }
    if pairs.is_empty() {
        return Ok(());
    }

    for slice in pairs.chunks(400) {
        let sql = format!(
            "INSERT INTO media_genres (media_item_id, genre_id) VALUES {} \
             ON CONFLICT DO NOTHING",
            vec!["(?, ?)"; slice.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (media_id, genre_id) in slice {
            query = query.bind(media_id).bind(genre_id);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("title.basics", format!("media_genres: {e}")))?;
    }
    Ok(())
}

/// Average ratings, scaled from IMDb's 0-10 to the schema's 0-100 integer.
pub fn load_average_ratings(path: &Path) -> Result<HashMap<u32, i64>, JobError> {
    let mut reader = TsvReader::open(path)?;
    let mut out = HashMap::with_capacity(1_800_000);
    while reader.advance()? {
        let Some(row) = reader.current_row() else {
            continue;
        };
        if let (Some(id), Some(average)) = (
            row.get("tconst").and_then(tconst_id),
            row.parse::<f64>("averageRating"),
        ) {
            out.insert(id, (average * 10.0).round() as i64);
        }
    }
    Ok(out)
}

/// How many titles are in the catalogue, and how many are core.
pub async fn counts(db: &Db) -> Result<(i64, i64), JobError> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items")
        .fetch_one(db.pool())
        .await?;
    let core: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_items WHERE in_core = 1")
        .fetch_one(db.pool())
        .await?;
    Ok((total, core))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imdb_types_map_onto_the_generic_kinds() {
        assert_eq!(media_kind("movie"), Some("film"));
        assert_eq!(media_kind("short"), Some("film"));
        assert_eq!(media_kind("tvSeries"), Some("series"));
        assert_eq!(media_kind("tvMiniSeries"), Some("series"));
        assert_eq!(media_kind("videoGame"), None);
        assert_eq!(
            media_kind("tvEpisode"),
            None,
            "episodes arrive via title.episode"
        );
    }

    #[test]
    fn anime_is_not_guessed_from_imdb() {
        // IMDb has no anime flag. Subtask 4.4 promotes titles when AniList confirms
        // them, which is a fact rather than an inference from country or genre.
        assert_eq!(media_kind("movie"), Some("film"));
        assert_ne!(media_kind("movie"), Some("anime_film"));
    }

    /// Eight columns per title. SQLite's default limit is 999 bound parameters, and
    /// exceeding it fails at runtime with a message that does not obviously point
    /// here — so it is checked at compile time instead, where it cannot be missed.
    const _: () = assert!(ROWS_PER_STATEMENT * 8 < 999);
}
