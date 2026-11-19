//! The discovery engine's memory (Phase 18, migration 0034): which arm first showed each title
//! to a profile, and in which part of the day; what the profile said of a recommendation; and
//! everything that happened to a shown title afterwards, which the bandit learns from.
//!
//! Arms, parts of the day and events are names here (`"deep_cut"`, `"evening"`,
//! `"completed"`): the recommender's types are not this crate's to know.

use crate::db::{Db, DbError};

/// One thing that happened to a title an arm showed, after it was shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Happened {
    pub media_item_id: i64,
    /// The arm that first showed it.
    pub arm: String,
    /// The part of the day it was shown in, or for a dismissal, the part it was said in.
    pub context: String,
    /// `"completed"`, `"played"`, `"abandoned"` (stopped within the first fifth),
    /// `"watchlisted"`, `"not_for_me"`, `"not_right_now"` or `"already_seen"`. A rating counts
    /// as `"completed"` from four stars, `"not_for_me"` at two or fewer, `"played"` at three.
    pub event: String,
    /// When, as stored.
    pub at: String,
}

pub struct DiscoveryRepository<'a> {
    db: &'a Db,
}

impl<'a> DiscoveryRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Record that each of `shown` (title, arm) was put in front of the profile in `context`.
    /// A title already shown keeps its first arm: the arm that found it is the one credited.
    pub async fn record_shown(
        &self,
        profile_id: i64,
        shown: &[(i64, &str)],
        context: &str,
    ) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;
        for chunk in shown.chunks(200) {
            let marks = vec!["(?, ?, ?, ?)"; chunk.len()].join(", ");
            let sql = format!(
                "INSERT OR IGNORE INTO discovery_shown (profile_id, media_item_id, arm, context) \
                 VALUES {marks}"
            );
            let mut q = sqlx::query(&sql);
            for (id, arm) in chunk {
                q = q.bind(profile_id).bind(id).bind(*arm).bind(context);
            }
            q.execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Everything that happened to the titles shown to a profile, after each was shown: its
    /// viewings (an episode counting for its series), its watchlist entry, its rating and
    /// its dismissal.
    pub async fn happened(&self, profile_id: i64) -> Result<Vec<Happened>, DbError> {
        let rows: Vec<(i64, String, String, String, String)> = sqlx::query_as(
            "SELECT s.media_item_id, s.arm, s.context,
                    CASE WHEN w.completed = 1 THEN 'completed'
                         WHEN m.runtime_minutes > 0
                              AND COALESCE(w.stopped_at_seconds, w.watched_seconds) * 5
                                  < m.runtime_minutes * 60 THEN 'abandoned'
                         ELSE 'played' END,
                    w.started_at
             FROM discovery_shown s
             JOIN watch_events w ON w.profile_id = s.profile_id
              AND (w.media_item_id = s.media_item_id
                   OR w.media_item_id IN (SELECT e.media_item_id FROM episodes e
                                          WHERE e.series_id = s.media_item_id))
              AND datetime(w.started_at) >= datetime(s.first_shown)
             JOIN media_items m ON m.id = w.media_item_id
             WHERE s.profile_id = ?1
             UNION ALL
             SELECT s.media_item_id, s.arm, s.context, 'watchlisted', l.added_at
             FROM discovery_shown s
             JOIN watchlist_items l ON l.profile_id = s.profile_id
              AND l.media_item_id = s.media_item_id
              AND datetime(l.added_at) >= datetime(s.first_shown)
             WHERE s.profile_id = ?1
             UNION ALL
             SELECT s.media_item_id, s.arm, s.context,
                    CASE WHEN r.stars >= 4 THEN 'completed'
                         WHEN r.stars <= 2 THEN 'not_for_me'
                         ELSE 'played' END,
                    r.rated_at
             FROM discovery_shown s
             JOIN ratings r ON r.profile_id = s.profile_id
              AND r.media_item_id = s.media_item_id
              AND datetime(r.rated_at) >= datetime(s.first_shown)
             WHERE s.profile_id = ?1
             UNION ALL
             SELECT s.media_item_id, s.arm, d.context, d.kind, d.at
             FROM discovery_shown s
             JOIN dismissals d ON d.profile_id = s.profile_id
              AND d.media_item_id = s.media_item_id
             WHERE s.profile_id = ?1
             ORDER BY 1, 5",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(media_item_id, arm, context, event, at)| Happened {
                media_item_id,
                arm,
                context,
                event,
                at,
            })
            .collect())
    }

    /// Record a dismissal (`"not_for_me"`, `"not_right_now"` or `"already_seen"`), said in
    /// `context`, replacing any earlier one for the title.
    pub async fn dismiss(
        &self,
        profile_id: i64,
        media_item_id: i64,
        kind: &str,
        context: &str,
    ) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO dismissals (profile_id, media_item_id, kind, context) VALUES (?, ?, ?, ?)
             ON CONFLICT (profile_id, media_item_id) DO UPDATE
                 SET kind = excluded.kind, context = excluded.context, at = excluded.at",
        )
        .bind(profile_id)
        .bind(media_item_id)
        .bind(kind)
        .bind(context)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Take a dismissal back (the undo on the card). True if there was one.
    pub async fn undismiss(&self, profile_id: i64, media_item_id: i64) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM dismissals WHERE profile_id = ? AND media_item_id = ?")
            .bind(profile_id)
            .bind(media_item_id)
            .execute(self.db.pool())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// A profile's dismissals: (title, kind, when).
    pub async fn dismissed(&self, profile_id: i64) -> Result<Vec<(i64, String, String)>, DbError> {
        Ok(sqlx::query_as(
            "SELECT media_item_id, kind, at FROM dismissals WHERE profile_id = ?
             ORDER BY media_item_id",
        )
        .bind(profile_id)
        .fetch_all(self.db.pool())
        .await?)
    }
}
