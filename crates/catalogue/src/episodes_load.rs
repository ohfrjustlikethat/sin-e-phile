//! Loading episodes: the seasonal skeleton (Phase 4, subtask 4.4).
//!
//! Three passes, because neither dataset alone has what an episode row needs:
//!
//! 1. `title.episode` gives `(episode tconst, parent tconst, season, episode)` — the
//!    numbering, but no title.
//! 2. `title.basics` gives the title, year and runtime — but says nothing about which
//!    series an episode belongs to. `load_titles` deliberately skipped every
//!    `tvEpisode` row for exactly that reason (see `imdb::KEPT_TYPES`).
//! 3. Only then can `media_items`, `episodes` and `episode_numbering` be written.
//!
//! Pass 1 is held in memory. It is bounded by the scope, not by the dataset: the
//! anime scope is 132,880 entries and the widest sensible one is a few million, at
//! about 24 bytes each.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use sinephile_persistence::Db;

use crate::job::{Batch, Job, JobError, SqliteTx};
use crate::{imdb, TsvReader};

/// Rows per transaction.
const BATCH: usize = 5_000;

/// Which episodes to load.
///
/// The core tier admits a series at 10 votes, and that is far too generous here:
/// 6,759,802 episodes hang off core series, against 670 MB of R4 headroom. Episodes
/// therefore get their own, tighter rule — a series earns them by being one someone
/// might actually sit down and watch through.
#[derive(Debug, Clone, Copy)]
pub struct EpisodeScope {
    /// Anime series always qualify regardless of votes. `SPEC.md` §6.2 singles out
    /// absolute-versus-seasonal reconciliation as a requirement, Phase 12's filename
    /// matching depends on it, and the whole anime catalogue is 132,880 episodes —
    /// small enough that a vote threshold would cost more than it saves.
    pub all_anime: bool,
    /// Minimum votes on the PARENT SERIES for a non-anime series to get episodes.
    pub min_votes: i64,
}

impl EpisodeScope {
    pub const ANIME_ONLY: Self = Self {
        all_anime: true,
        min_votes: i64::MAX,
    };

    pub fn admits(&self, parent: &SeriesInfo) -> bool {
        (self.all_anime && parent.is_anime) || parent.votes >= self.min_votes
    }
}

/// A catalogue series an episode can hang off.
#[derive(Debug, Clone)]
pub struct SeriesInfo {
    pub media_item_id: i64,
    pub is_anime: bool,
    pub votes: i64,
}

/// One episode the scope admits, as `title.episode` describes it.
#[derive(Debug, Clone, Copy)]
pub struct Wanted {
    /// Numeric part of the episode's own `tt` id.
    pub tconst: u32,
    pub series_id: i64,
    pub season: Option<i32>,
    pub episode: Option<i32>,
}

fn tconst_id(raw: &str) -> Option<u32> {
    raw.strip_prefix("tt")?.parse().ok()
}

/// Every series in the catalogue, by IMDb id.
pub async fn series(db: &Db) -> Result<HashMap<String, SeriesInfo>, JobError> {
    let rows: Vec<(String, i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT e.external_id, m.id, m.kind, m.rating_votes
           FROM external_ids e
           JOIN media_items m ON m.id = e.media_item_id
          WHERE e.source = 'imdb' AND m.kind IN ('series', 'anime_series')",
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|(tconst, id, kind, votes)| {
            (
                tconst,
                SeriesInfo {
                    media_item_id: id,
                    is_anime: kind == "anime_series",
                    votes: votes.unwrap_or(0),
                },
            )
        })
        .collect())
}

/// Pass 1 — which episodes the scope admits.
pub fn collect(
    episodes_path: &std::path::Path,
    series: &HashMap<String, SeriesInfo>,
    scope: EpisodeScope,
    skip: &HashSet<u32>,
) -> Result<Vec<Wanted>, JobError> {
    let mut reader = TsvReader::open(episodes_path)?;
    imdb::check_columns(&reader, &imdb::TITLE_EPISODE)?;

    let mut wanted = Vec::new();
    while let Some(row) = reader.next_row()? {
        let Some(parent) = row.get("parentTconst").and_then(|p| series.get(p)) else {
            continue;
        };
        if !scope.admits(parent) {
            continue;
        }
        let Some(tconst) = row.get("tconst").and_then(tconst_id) else {
            continue;
        };
        if skip.contains(&tconst) {
            continue;
        }
        wanted.push(Wanted {
            tconst,
            series_id: parent.media_item_id,
            // Stored as given. A `\N` season is a real thing in IMDb — a special that
            // never got placed — and inventing a 0 for it would be a fact we made up.
            season: row.get("seasonNumber").and_then(|v| v.parse::<i32>().ok()),
            episode: row.get("episodeNumber").and_then(|v| v.parse::<i32>().ok()),
        });
    }
    Ok(wanted)
}

/// Pass 2 — a `series` row for every parent that gained episodes.
///
/// `episodes.series_id` has a foreign key onto `series.media_item_id`, so these must
/// exist before any episode does. The one-to-one extension table is what keeps a
/// dozen always-NULL series columns off 2.7 million film rows.
pub async fn load_series_rows(job: &mut Job<'_>, wanted: Arc<Vec<Wanted>>) -> Result<(), JobError> {
    let mut parents: Vec<i64> = wanted.iter().map(|w| w.series_id).collect();
    parents.sort_unstable();
    parents.dedup();
    let parents = Arc::new(parents);

    job.run_step("episodes.series", move |tx, cursor| {
        let parents = Arc::clone(&parents);
        Box::pin(async move {
            let from: usize = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
            let end = (from + BATCH).min(parents.len());
            if from >= parents.len() {
                return Ok(Batch::finished(0));
            }
            let chunk = &parents[from..end];

            // Counts come from the episodes themselves rather than from IMDb, which
            // publishes no per-series episode total.
            let sql = format!(
                "INSERT INTO series (media_item_id, total_episodes)
                 SELECT id, NULL FROM media_items WHERE id IN ({})
                 ON CONFLICT (media_item_id) DO NOTHING",
                vec!["?"; chunk.len()].join(", ")
            );
            let mut query = sqlx::query(&sql);
            for id in chunk {
                query = query.bind(id);
            }
            query
                .execute(&mut **tx)
                .await
                .map_err(|e| JobError::step("episodes.series", e.to_string()))?;

            Ok(if end >= parents.len() {
                Batch::finished(chunk.len() as i64)
            } else {
                Batch::more(end.to_string(), chunk.len() as i64)
            })
        })
    })
    .await?;
    Ok(())
}

/// Pass 3 — the episodes themselves, with titles read from `title.basics`.
///
/// The cursor is the `title.basics` tconst, so a resume seeks rather than re-reading
/// eleven million rows.
pub async fn load_episodes(
    job: &mut Job<'_>,
    basics: PathBuf,
    wanted: Arc<HashMap<u32, Wanted>>,
) -> Result<(), JobError> {
    job.run_step("episodes.rows", move |tx, cursor| {
        let basics = basics.clone();
        let wanted = Arc::clone(&wanted);
        Box::pin(async move {
            let mut reader = TsvReader::open(&basics)?;
            imdb::check_columns(&reader, &imdb::TITLE_BASICS)?;

            let mut pending: Vec<Episode> = Vec::new();
            if let Some(cursor) = cursor.as_deref() {
                reader.seek_past("tconst", cursor)?;
                if let Some(episode) = episode_from(&reader, &wanted) {
                    pending.push(episode);
                }
            }

            let mut last_id = None;
            while pending.len() < BATCH {
                if !reader.advance()? {
                    break;
                }
                if let Some(episode) = episode_from(&reader, &wanted) {
                    pending.push(episode);
                }
                if let Some(row) = reader.current_row() {
                    if let Some(id) = row.get("tconst") {
                        last_id = Some(id.to_string());
                    }
                }
            }

            if pending.is_empty() {
                return Ok(Batch::finished(0));
            }
            let count = pending.len() as i64;
            insert(tx, &pending).await?;

            Ok(match last_id {
                Some(id) if count as usize >= BATCH => Batch::more(id, count),
                _ => Batch::finished(count),
            })
        })
    })
    .await?;
    Ok(())
}

struct Episode {
    wanted: Wanted,
    title: String,
    year: Option<i64>,
    runtime: Option<i64>,
}

fn episode_from(reader: &TsvReader, wanted: &HashMap<u32, Wanted>) -> Option<Episode> {
    let row = reader.current_row()?;
    if row.get("titleType") != Some("tvEpisode") {
        return None;
    }
    let id = row.get("tconst").and_then(tconst_id)?;
    let w = wanted.get(&id)?;
    Some(Episode {
        wanted: *w,
        title: row.get("primaryTitle").unwrap_or_default().to_string(),
        year: row.get("startYear").and_then(|v| v.parse().ok()),
        runtime: row.get("runtimeMinutes").and_then(|v| v.parse().ok()),
    })
}

async fn insert(tx: &mut SqliteTx<'_>, episodes: &[Episode]) -> Result<(), JobError> {
    let step = "episodes.rows";

    for chunk in episodes.chunks(200) {
        // RETURNING id, for the same reason load.rs uses it: `last_insert_rowid()`
        // arithmetic is correct only while the table has no gaps, and after any delete
        // every episode row would attach to the WRONG media item. Silently.
        let sql = format!(
            "INSERT INTO media_items (kind, primary_title, release_year, runtime_minutes)
             VALUES {} RETURNING id",
            vec!["('episode', ?, ?, ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for e in chunk {
            query = query.bind(&e.title).bind(e.year).bind(e.runtime);
        }
        let ids: Vec<i64> = query
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| JobError::step(step, format!("media_items: {e}")))?;

        if ids.len() != chunk.len() {
            return Err(JobError::step(
                step,
                format!(
                    "inserted {} media items for {} episodes",
                    ids.len(),
                    chunk.len()
                ),
            ));
        }

        let sql = format!(
            "INSERT INTO episodes (media_item_id, series_id, season_number, episode_number)
             VALUES {}",
            vec!["(?, ?, ?, ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (id, e) in ids.iter().zip(chunk) {
            query = query
                .bind(id)
                .bind(e.wanted.series_id)
                .bind(e.wanted.season)
                .bind(e.wanted.episode);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step(step, format!("episodes: {e}")))?;

        // What IMDb calls this episode. The absolute number is left NULL: IMDb does
        // not publish one, and computing it by counting within a season would be an
        // invention — recaps and specials interleave, which is the single largest
        // cause of off-by-N drift between numbering schemes (migration 0003).
        let sql = format!(
            "INSERT INTO episode_numbering
                 (episode_id, source, season_number, episode_number, absolute_number)
             VALUES {}",
            vec!["(?, 'imdb', ?, ?, NULL)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (id, e) in ids.iter().zip(chunk) {
            query = query.bind(id).bind(e.wanted.season).bind(e.wanted.episode);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step(step, format!("episode_numbering: {e}")))?;

        // The episode's own IMDb id. Without it a re-run duplicates every episode —
        // `episodes` has no natural key to collide on — and Phase 12 has nothing to
        // reconcile a filename against once a second source arrives.
        let sql = format!(
            "INSERT INTO external_ids (media_item_id, source, external_id)
             VALUES {} ON CONFLICT DO NOTHING",
            vec!["(?, 'imdb', ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (id, e) in ids.iter().zip(chunk) {
            query = query.bind(id).bind(format!("tt{:07}", e.wanted.tconst));
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step(step, format!("external_ids: {e}")))?;
    }
    Ok(())
}

/// Seasons, derived from the episodes that were loaded.
///
/// IMDb publishes no season list, so this is the only way to have one — and a TV view
/// that cannot list seasons is not a TV view.
pub async fn load_seasons(job: &mut Job<'_>) -> Result<(), JobError> {
    job.run_step("episodes.seasons", move |tx, _| {
        Box::pin(async move {
            let inserted = sqlx::query(
                "INSERT INTO seasons (series_id, season_number, episode_count)
                 SELECT series_id, season_number, COUNT(*)
                   FROM episodes
                  WHERE season_number IS NOT NULL
                  GROUP BY series_id, season_number
                 ON CONFLICT (series_id, season_number)
                 DO UPDATE SET episode_count = excluded.episode_count",
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("episodes.seasons", e.to_string()))?
            .rows_affected() as i64;

            // Now that seasons exist, point each episode at its own.
            sqlx::query(
                "UPDATE episodes
                    SET season_id = (SELECT s.id FROM seasons s
                                      WHERE s.series_id = episodes.series_id
                                        AND s.season_number = episodes.season_number)
                  WHERE season_number IS NOT NULL AND season_id IS NULL",
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("episodes.seasons", e.to_string()))?;

            // One statement, one batch: this is a grouped aggregate over a table we
            // just wrote, not a stream, so there is nothing to checkpoint partway.
            Ok(Batch::finished(inserted))
        })
    })
    .await?;
    Ok(())
}

/// Fill in `series.total_episodes` from what was actually loaded.
pub async fn count_episodes(db: &Db) -> Result<(i64, i64), JobError> {
    sqlx::query(
        "UPDATE series
            SET total_episodes = (SELECT COUNT(*) FROM episodes e
                                   WHERE e.series_id = series.media_item_id)",
    )
    .execute(db.pool())
    .await?;

    let episodes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM episodes")
        .fetch_one(db.pool())
        .await?;
    let seasons: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seasons")
        .fetch_one(db.pool())
        .await?;
    Ok((episodes, seasons))
}

/// Episode IMDb ids already in the database.
///
/// The job checkpoint protects a RESUMED run. It does nothing for a re-run after
/// `ingest reset`, or for a second pass that widens the scope — and an episode has no
/// natural key in `episodes`, so a duplicate would simply appear twice with no
/// constraint to stop it. Excluding these from `wanted` is what makes widening the
/// scope later a safe operation rather than a corrupting one.
pub async fn already_loaded(db: &Db) -> Result<HashSet<u32>, JobError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT e.external_id
           FROM external_ids e
           JOIN media_items m ON m.id = e.media_item_id
          WHERE e.source = 'imdb' AND m.kind = 'episode'",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(rows.iter().filter_map(|t| tconst_id(t)).collect())
}
