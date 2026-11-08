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

/// An episode as a series page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRow {
    pub media_item_id: i64,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub title: String,
    pub air_date: Option<String>,
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

    /// New episodes of the series a profile follows (Phase 16, Home): episodes the
    /// catalogue gained in the last `days` days, dated `from_year` or later, not watched by
    /// the profile, of a series in its library, on its watchlist, or with an episode it
    /// watched. One row per series, its newest such episode:
    /// (series, episode, season, number), newest first. The catalogue has no air dates
    /// (IMDb publishes years), so "new" is "new to the catalogue, and this year's"; the year
    /// keeps out the back catalogue of a series that was only just followed.
    pub async fn new_episodes(
        &self,
        profile_id: i64,
        days: i64,
        from_year: i64,
        limit: i64,
    ) -> Result<Vec<(i64, i64, Option<i64>, Option<i64>)>, DbError> {
        Ok(sqlx::query_as(
            "WITH followed(series_id) AS (
                 SELECT COALESCE(e.series_id, l.media_item_id) FROM local_file_matches l
                   LEFT JOIN episodes e ON e.media_item_id = l.media_item_id
                 UNION SELECT media_item_id FROM watchlist_items WHERE profile_id = ?1
                 UNION SELECT e.series_id FROM watch_events w
                   JOIN episodes e ON e.media_item_id = w.media_item_id WHERE w.profile_id = ?1),
             fresh AS (
                 SELECT e.series_id, e.media_item_id, e.season_number, e.episode_number,
                        ROW_NUMBER() OVER (PARTITION BY e.series_id
                            ORDER BY e.season_number DESC, e.episode_number DESC) AS n,
                        m.created_at
                   FROM episodes e JOIN media_items m ON m.id = e.media_item_id
                  WHERE e.series_id IN (SELECT series_id FROM followed)
                    AND m.created_at >= datetime('now', '-' || ?2 || ' days')
                    -- No year said is no evidence of being old: it arrived this month.
                    AND COALESCE(m.release_year, ?3) >= ?3
                    AND e.media_item_id NOT IN
                        (SELECT media_item_id FROM watch_events WHERE profile_id = ?1))
             SELECT series_id, media_item_id, season_number, episode_number FROM fresh
              WHERE n = 1 ORDER BY created_at DESC, season_number DESC, episode_number DESC
              LIMIT ?4",
        )
        .bind(profile_id)
        .bind(days)
        .bind(from_year)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// A series' seasons, by number, with how many episodes the catalogue holds for each
    /// (Phase 11's series page). Episodes with no season number are season `None`.
    pub async fn seasons(&self, series_id: i64) -> Result<Vec<(Option<i64>, i64)>, DbError> {
        let rows = sqlx::query(
            "SELECT season_number, COUNT(*) FROM episodes WHERE series_id = ?
             GROUP BY season_number ORDER BY season_number IS NULL, season_number",
        )
        .bind(series_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
    }

    /// One season's episodes in order, with their titles and air dates. `None` is the
    /// episodes without a season number.
    pub async fn episodes_of(
        &self,
        series_id: i64,
        season: Option<i64>,
    ) -> Result<Vec<EpisodeRow>, DbError> {
        let rows = sqlx::query(
            "SELECT e.media_item_id, e.season_number, e.episode_number, m.primary_title, e.air_date
             FROM episodes e JOIN media_items m ON m.id = e.media_item_id
             WHERE e.series_id = ?1 AND e.season_number IS ?2
             ORDER BY e.episode_number IS NULL, e.episode_number, e.media_item_id",
        )
        .bind(series_id)
        .bind(season)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| EpisodeRow {
                media_item_id: r.get(0),
                season_number: r.get(1),
                episode_number: r.get(2),
                title: r.get(3),
                air_date: r.get(4),
            })
            .collect())
    }

    /// The episode after this one in the series: the next in its season, else the first
    /// of the next season that has any (Phase 11's next-episode card).
    pub async fn next_after(&self, episode_id: i64) -> Result<Option<EpisodeRow>, DbError> {
        let row = sqlx::query(
            "SELECT n.media_item_id, n.season_number, n.episode_number, m.primary_title, n.air_date
             FROM episodes e
             JOIN episodes n ON n.series_id = e.series_id
             JOIN media_items m ON m.id = n.media_item_id
             WHERE e.media_item_id = ?1
               AND n.season_number IS NOT NULL AND n.episode_number IS NOT NULL
               AND (n.season_number > e.season_number
                    OR (n.season_number = e.season_number AND n.episode_number > e.episode_number))
             ORDER BY n.season_number, n.episode_number
             LIMIT 1",
        )
        .bind(episode_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| EpisodeRow {
            media_item_id: r.get(0),
            season_number: r.get(1),
            episode_number: r.get(2),
            title: r.get(3),
            air_date: r.get(4),
        }))
    }

    /// The one before it, the same way backwards.
    pub async fn previous_before(&self, episode_id: i64) -> Result<Option<EpisodeRow>, DbError> {
        let row = sqlx::query(
            "SELECT p.media_item_id, p.season_number, p.episode_number, m.primary_title, p.air_date
             FROM episodes e
             JOIN episodes p ON p.series_id = e.series_id
             JOIN media_items m ON m.id = p.media_item_id
             WHERE e.media_item_id = ?1
               AND p.season_number IS NOT NULL AND p.episode_number IS NOT NULL
               AND (p.season_number < e.season_number
                    OR (p.season_number = e.season_number AND p.episode_number < e.episode_number))
             ORDER BY p.season_number DESC, p.episode_number DESC
             LIMIT 1",
        )
        .bind(episode_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| EpisodeRow {
            media_item_id: r.get(0),
            season_number: r.get(1),
            episode_number: r.get(2),
            title: r.get(3),
            air_date: r.get(4),
        }))
    }

    /// Where an episode sits: (series, season, episode).
    pub async fn placement(
        &self,
        episode_id: i64,
    ) -> Result<Option<(i64, Option<i64>, Option<i64>)>, DbError> {
        let row = sqlx::query(
            "SELECT series_id, season_number, episode_number FROM episodes WHERE media_item_id = ?",
        )
        .bind(episode_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
    }

    /// Record when the series' next episode airs (ADR-0030): one row per series, the
    /// newest schedule replacing the last.
    pub async fn set_next_airing(
        &self,
        series_id: i64,
        episode: i64,
        airs_at: i64,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO airing_schedule (series_id, episode_number, airs_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (series_id) DO UPDATE
               SET episode_number = excluded.episode_number, airs_at = excluded.airs_at,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        )
        .bind(series_id)
        .bind(episode)
        .bind(airs_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// (episode, airs at, Unix seconds), if one is scheduled — whether or not an
    /// `episodes` row exists for it yet.
    pub async fn next_airing(&self, series_id: i64) -> Result<Option<(i64, i64)>, DbError> {
        let row =
            sqlx::query("SELECT episode_number, airs_at FROM airing_schedule WHERE series_id = ?")
                .bind(series_id)
                .fetch_optional(self.db.pool())
                .await?;
        Ok(row.map(|r| (r.get(0), r.get(1))))
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
