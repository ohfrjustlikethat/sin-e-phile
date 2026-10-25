//! Episodes: the seasonal half of the numbering problem (Phase 4, subtask 4.4).
//!
//! # Where each number comes from
//!
//! Migration 0003 stores both a seasonal and an absolute number per episode, and
//! **neither source supplies both**:
//!
//! - **IMDb `title.episode`** gives `(parentTconst, seasonNumber, episodeNumber)` —
//!   the seasonal numbering, for every series it knows, anime included.
//! - **AniList** gives an episode count and an airing schedule per entry, and an
//!   AniList entry is one cour — so its numbering is effectively absolute within a
//!   run, and restarts where IMDb would say "season 2".
//!
//! That is exactly the reconciliation `episode_numbering` exists for, and it is why
//! this module loads IMDb first: the seasonal skeleton has to exist before anything
//! can be reconciled against it.
//!
//! # Why this is measured before it is loaded
//!
//! `title.episode` is roughly 8.5 million rows, and R4's headroom after the AniList
//! ingestion is 670 MB. Loading every episode of every series would be a second
//! catalogue. So the scope is chosen from counts, the same way the two-tier title
//! scope was — and the counts are produced by this module before anything is written.

use std::collections::HashMap;
use std::path::Path;

use sinephile_persistence::Db;

use crate::imdb;
use crate::job::JobError;
use crate::tsv::TsvReader;

/// What the episode dataset actually contains, relative to the catalogue we have.
#[derive(Debug, Default, Clone)]
pub struct Measurement {
    pub rows: i64,
    /// Rows whose `parentTconst` is not a series we hold at all.
    pub parent_unknown: i64,
    /// Rows whose parent is a series we hold but did not put in the core tier.
    pub parent_indexed_only: i64,
    /// Rows whose parent is a core-tier series. The candidate scope.
    pub parent_core: i64,
    /// Rows whose parent is an anime series, core or not. A subset of the two above.
    pub parent_anime: i64,
    /// Rows with a usable seasonal number. The rest are `\N` on one or both columns
    /// and cannot contribute seasonal numbering at all.
    pub with_season_and_episode: i64,
    /// Distinct series that would gain episodes under the core scope.
    pub core_series_with_episodes: i64,
    /// `(vote threshold, episodes whose parent series has at least that many votes)`.
    ///
    /// The core tier admits a series at 10 votes, which turns out to be far too
    /// generous for episodes: 6.9 million of them do not fit in R4's headroom. This
    /// is the curve the tighter threshold gets chosen from — measured, because four
    /// storage predictions in this phase have been wrong in both directions.
    pub by_vote_threshold: Vec<(i64, i64, i64)>,
}

/// Thresholds worth reporting. Wide apart, because the question is which order of
/// magnitude fits, not which exact number.
const VOTE_THRESHOLDS: &[i64] = &[10, 100, 1_000, 5_000, 10_000, 50_000];

impl Measurement {
    pub fn report(&self) {
        let pct = |n: i64| {
            if self.rows == 0 {
                0.0
            } else {
                100.0 * n as f64 / self.rows as f64
            }
        };
        println!();
        println!("  title.episode: {} rows", self.rows);
        println!(
            "    {:>9}  ({:>4.1}%)  parent is a CORE series      <- the candidate scope",
            self.parent_core,
            pct(self.parent_core)
        );
        println!(
            "    {:>9}  ({:>4.1}%)  parent is indexed, not core",
            self.parent_indexed_only,
            pct(self.parent_indexed_only)
        );
        println!(
            "    {:>9}  ({:>4.1}%)  parent is not in the catalogue",
            self.parent_unknown,
            pct(self.parent_unknown)
        );
        println!(
            "    {:>9}  ({:>4.1}%)  parent is an anime series (subset of the above)",
            self.parent_anime,
            pct(self.parent_anime)
        );
        println!(
            "    {:>9}  ({:>4.1}%)  have BOTH a season and an episode number",
            self.with_season_and_episode,
            pct(self.with_season_and_episode)
        );
        println!(
            "    {:>9}            distinct core series that would gain episodes",
            self.core_series_with_episodes
        );
        println!();
        println!("  by parent series vote count:");
        println!("    {:>8}  {:>10}  {:>8}", "votes >=", "episodes", "series");
        for (threshold, episodes, series) in &self.by_vote_threshold {
            println!("    {threshold:>8}  {episodes:>10}  {series:>8}");
        }
        println!();
    }
}

/// A series we hold, and which tier it is in.
struct Parent {
    in_core: bool,
    is_anime: bool,
    votes: i64,
}

/// Count what would be loaded, without loading it.
///
/// Deliberately writes nothing — a measurement that mutates the thing it measures
/// cannot be re-run to check itself.
pub async fn measure(db: &Db, datasets: &Path) -> Result<Measurement, JobError> {
    let parents = series_parents(db).await?;
    tracing::info!("{} series in the catalogue", parents.len());

    let path = datasets.join(imdb::TITLE_EPISODE.filename);
    let mut reader = TsvReader::open(&path)?;
    imdb::check_columns(&reader, &imdb::TITLE_EPISODE)?;

    let mut m = Measurement::default();
    let mut core_series = std::collections::HashSet::new();
    let mut by_threshold: Vec<(i64, i64, std::collections::HashSet<String>)> = VOTE_THRESHOLDS
        .iter()
        .map(|t| (*t, 0, std::collections::HashSet::new()))
        .collect();

    while let Some(row) = reader.next_row()? {
        m.rows += 1;
        let parent = row.get("parentTconst").unwrap_or_default();
        let season = row.get("seasonNumber").and_then(|v| v.parse::<i64>().ok());
        let episode = row.get("episodeNumber").and_then(|v| v.parse::<i64>().ok());
        if season.is_some() && episode.is_some() {
            m.with_season_and_episode += 1;
        }

        match parents.get(parent) {
            None => m.parent_unknown += 1,
            Some(p) => {
                if p.is_anime {
                    m.parent_anime += 1;
                }
                for (threshold, episodes, series) in by_threshold.iter_mut() {
                    if p.votes >= *threshold {
                        *episodes += 1;
                        series.insert(parent.to_string());
                    }
                }
                if p.in_core {
                    m.parent_core += 1;
                    core_series.insert(parent.to_string());
                } else {
                    m.parent_indexed_only += 1;
                }
            }
        }
    }

    m.core_series_with_episodes = core_series.len() as i64;
    m.by_vote_threshold = by_threshold
        .into_iter()
        .map(|(t, episodes, series)| (t, episodes, series.len() as i64))
        .collect();
    Ok(m)
}

/// Every series in the catalogue, by IMDb id.
async fn series_parents(db: &Db) -> Result<HashMap<String, Parent>, JobError> {
    let rows: Vec<(String, i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT e.external_id, m.in_core, m.kind, m.rating_votes
           FROM external_ids e
           JOIN media_items m ON m.id = e.media_item_id
          WHERE e.source = 'imdb' AND m.kind IN ('series', 'anime_series')",
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|(tconst, in_core, kind, votes)| {
            (
                tconst,
                Parent {
                    in_core: in_core == 1,
                    is_anime: kind == "anime_series",
                    votes: votes.unwrap_or(0),
                },
            )
        })
        .collect())
}
