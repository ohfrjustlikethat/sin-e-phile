//! Episodes, and the absolute-vs-seasonal numbering reconciliation (`SPEC.md` §6.2).
//!
//! The problem is set out in `migrations/0003_series.up.sql`. In short: a
//! long-running anime has no single correct episode number, the conversions
//! between schemes are not arithmetic, and Phase 12 will be handed a filename
//! containing exactly one number and asked which episode it is.
//!
//! So resolution is a lookup, never a calculation, and it says which scheme it
//! matched on. A caller that cannot tell "TVDB S03E07" from "AniList 59" cannot
//! report a false-confident match, and Phase 12's exit criterion is a
//! false-confident rate below 1%.

use sqlx::Row;

use crate::db::{Db, DbError};
use crate::model::{EpisodeNumbering, IdSource};

/// How an episode was identified from a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberingMatch {
    /// A source's own numbering table matched exactly. The strongest signal.
    SourceExact,
    /// Matched the item's own seasonal numbering.
    Seasonal,
    /// Matched the item's own absolute numbering.
    Absolute,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEpisode {
    pub episode_id: i64,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub absolute_number: Option<i64>,
    pub matched_by: NumberingMatch,
}

pub struct EpisodeRepository<'a> {
    db: &'a Db,
}

impl<'a> EpisodeRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Create the `series` extension row for an existing media item.
    pub async fn create_series(
        &self,
        media_item_id: i64,
        is_cour_based: bool,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO series (media_item_id, is_cour_based) VALUES (?, ?)
             ON CONFLICT (media_item_id) DO UPDATE SET is_cour_based = excluded.is_cour_based",
        )
        .bind(media_item_id)
        .bind(i64::from(is_cour_based))
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn create_season(
        &self,
        series_id: i64,
        season_number: i64,
        name: Option<&str>,
    ) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO seasons (series_id, season_number, name) VALUES (?, ?, ?)
             ON CONFLICT (series_id, season_number) DO UPDATE SET name = excluded.name
             RETURNING id",
        )
        .bind(series_id)
        .bind(season_number)
        .bind(name)
        .fetch_one(self.db.pool())
        .await?)
    }

    /// Attach an existing media item (kind `episode`) to a series.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_episode(
        &self,
        media_item_id: i64,
        series_id: i64,
        season_id: Option<i64>,
        season_number: Option<i64>,
        episode_number: Option<i64>,
        absolute_number: Option<i64>,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO episodes (
                 media_item_id, series_id, season_id,
                 season_number, episode_number, absolute_number
             ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(media_item_id)
        .bind(series_id)
        .bind(season_id)
        .bind(season_number)
        .bind(episode_number)
        .bind(absolute_number)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Record what one source calls this episode.
    pub async fn set_numbering(&self, numbering: &EpisodeNumbering) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO episode_numbering
                 (episode_id, source, season_number, episode_number, absolute_number)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (episode_id, source) DO UPDATE
                 SET season_number   = excluded.season_number,
                     episode_number  = excluded.episode_number,
                     absolute_number = excluded.absolute_number",
        )
        .bind(numbering.episode_id)
        .bind(numbering.source.as_str())
        .bind(numbering.season_number)
        .bind(numbering.episode_number)
        .bind(numbering.absolute_number)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// "Source X calls this S`season`E`episode` — which episode is that?"
    pub async fn resolve_seasonal(
        &self,
        series_id: i64,
        source: IdSource,
        season: i64,
        episode: i64,
    ) -> Result<Option<ResolvedEpisode>, DbError> {
        // The source's own table first: it is the only record that is asserted
        // rather than inferred.
        let row = sqlx::query(
            "SELECT e.media_item_id, e.season_number, e.episode_number, e.absolute_number
             FROM episode_numbering n
             JOIN episodes e ON e.media_item_id = n.episode_id
             WHERE e.series_id = ? AND n.source = ?
               AND n.season_number = ? AND n.episode_number = ?",
        )
        .bind(series_id)
        .bind(source.as_str())
        .bind(season)
        .bind(episode)
        .fetch_optional(self.db.pool())
        .await?;

        if let Some(row) = row {
            return Ok(Some(resolved(row, NumberingMatch::SourceExact)?));
        }

        // Fall back to the episode's own seasonal numbering.
        let row = sqlx::query(
            "SELECT media_item_id, season_number, episode_number, absolute_number
             FROM episodes
             WHERE series_id = ? AND season_number = ? AND episode_number = ?",
        )
        .bind(series_id)
        .bind(season)
        .bind(episode)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(|r| resolved(r, NumberingMatch::Seasonal))
            .transpose()
    }

    /// "A file says episode 59 — which episode is that?"
    ///
    /// The anime case. A release group numbers absolutely across a run that TVDB
    /// splits into three seasons, so this cannot be answered by arithmetic.
    pub async fn resolve_absolute(
        &self,
        series_id: i64,
        source: Option<IdSource>,
        absolute: i64,
    ) -> Result<Option<ResolvedEpisode>, DbError> {
        if let Some(source) = source {
            let row = sqlx::query(
                "SELECT e.media_item_id, e.season_number, e.episode_number, e.absolute_number
                 FROM episode_numbering n
                 JOIN episodes e ON e.media_item_id = n.episode_id
                 WHERE e.series_id = ? AND n.source = ? AND n.absolute_number = ?",
            )
            .bind(series_id)
            .bind(source.as_str())
            .bind(absolute)
            .fetch_optional(self.db.pool())
            .await?;

            if let Some(row) = row {
                return Ok(Some(resolved(row, NumberingMatch::SourceExact)?));
            }
        }

        let row = sqlx::query(
            "SELECT media_item_id, season_number, episode_number, absolute_number
             FROM episodes WHERE series_id = ? AND absolute_number = ?",
        )
        .bind(series_id)
        .bind(absolute)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(|r| resolved(r, NumberingMatch::Absolute))
            .transpose()
    }

    /// Every numbering recorded for an episode — what a "why did it match?"
    /// explanation is built from.
    pub async fn numberings_for(&self, episode_id: i64) -> Result<Vec<EpisodeNumbering>, DbError> {
        let rows = sqlx::query(
            "SELECT episode_id, source, season_number, episode_number, absolute_number
             FROM episode_numbering WHERE episode_id = ? ORDER BY source",
        )
        .bind(episode_id)
        .fetch_all(self.db.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                let source: String = row.try_get("source")?;
                Ok(EpisodeNumbering {
                    episode_id: row.try_get("episode_id")?,
                    source: IdSource::ALL
                        .into_iter()
                        .find(|s| s.as_str() == source)
                        .unwrap_or(IdSource::Imdb),
                    season_number: row.try_get("season_number")?,
                    episode_number: row.try_get("episode_number")?,
                    absolute_number: row.try_get("absolute_number")?,
                })
            })
            .collect()
    }
}

fn resolved(
    row: sqlx::sqlite::SqliteRow,
    matched_by: NumberingMatch,
) -> Result<ResolvedEpisode, DbError> {
    Ok(ResolvedEpisode {
        episode_id: row.try_get("media_item_id")?,
        season_number: row.try_get("season_number")?,
        episode_number: row.try_get("episode_number")?,
        absolute_number: row.try_get("absolute_number")?,
        matched_by,
    })
}
