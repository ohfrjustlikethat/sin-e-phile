//! Continue watching, by title (Phase 16, the author's round 8).
//!
//! A position is saved per thing played, so a show used to appear as its lone episode
//! ("Episode #1.1"), opening that episode. Now a show is one card, titled with the show,
//! saying which episode is next for this profile and how far into it they are; it opens
//! the show's page, where that episode is ready to resume.
//!
//! The episode to watch is the latest one started, if it is unfinished; if it was
//! finished, the one after it; if there is none after it, the show is done and leaves the
//! rail. "Finished" is the same 95% the rest of the app uses (credits).

use std::collections::HashSet;

use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{EpisodeRepository, MediaRepository, ProfileRepository};
use sinephile_persistence::{Db, DbError};

/// Positions read to fill one rail: enough for twenty cards after a long binge of one show.
const READ: i64 = 400;

/// One card of Continue watching.
#[derive(Debug, Clone, PartialEq)]
pub struct Continuing {
    /// What the card is: the film, or the series of the episode.
    pub card: i64,
    /// What plays: the film itself, or the episode to watch.
    pub plays: i64,
    /// A show's card (the TV Shows page), not a film's (the Films page).
    pub is_show: bool,
    /// For a show: "S1 E2 · Traces to Nowhere", or "Next: S1 E3" when the last was finished.
    pub note: Option<String>,
    /// How far into what plays, from 0 to 1, when it has been started.
    pub progress: Option<f64>,
    /// Where to resume what plays, in seconds; `None` from the start.
    pub resume_at: Option<i64>,
    /// Its season and number, for a show.
    pub season: Option<i64>,
    pub episode: Option<i64>,
    /// The episode's own name, when it has one (see [`episode_name`]).
    pub episode_title: Option<String>,
}

/// Finished, as the rest of the app counts it: 95% of the way, because of the credits.
fn finished(p: &PlaybackPosition) -> bool {
    p.duration_seconds
        .is_some_and(|d| d > 0 && p.position_seconds >= d * 95 / 100)
}

/// An episode's own name, or `None` when the catalogue has only a placeholder for it:
/// IMDb lists many episodes as "Episode #1.2", which says nothing the numbers do not.
pub fn episode_name(title: &str) -> Option<&str> {
    let t = title.trim();
    let rest = t
        .strip_prefix("Episode")
        .map(|r| r.trim_start_matches([' ', '#']));
    match rest {
        Some(r) if !r.is_empty() && r.chars().all(|c| c.is_ascii_digit() || c == '.') => None,
        _ if t.is_empty() => None,
        _ => Some(t),
    }
}

/// "S1 E2", or as much of it as is known.
pub fn numbering(season: Option<i64>, episode: Option<i64>) -> String {
    match (season, episode) {
        (Some(s), Some(e)) => format!("S{s} E{e}"),
        (None, Some(e)) => format!("E{e}"),
        (Some(s), None) => format!("S{s}"),
        (None, None) => String::new(),
    }
}

fn note(next: bool, season: Option<i64>, episode: Option<i64>, title: Option<&str>) -> String {
    let place = numbering(season, episode);
    let line = match title {
        Some(t) if !place.is_empty() => format!("{place} · {t}"),
        Some(t) => t.to_string(),
        None => place,
    };
    if next {
        format!("Next: {line}")
    } else {
        line
    }
}

/// What one saved position means for its card, given where it sits (`placement`).
async fn resolve(
    db: &Db,
    p: &PlaybackPosition,
    placement: Option<(i64, Option<i64>, Option<i64>)>,
) -> Result<Option<Continuing>, DbError> {
    let media = MediaRepository::new(db);
    let Some((series, season, episode)) = placement else {
        // A film (or anything that is not an episode): unfinished, or not on the rail.
        if finished(p) {
            return Ok(None);
        }
        return Ok(Some(Continuing {
            card: p.media_item_id,
            plays: p.media_item_id,
            is_show: false,
            note: None,
            progress: progress(p),
            resume_at: Some(p.position_seconds).filter(|s| *s > 0),
            season: None,
            episode: None,
            episode_title: None,
        }));
    };
    if !finished(p) {
        let title = media.by_id(p.media_item_id).await?.map(|m| m.primary_title);
        let named = title.as_deref().and_then(episode_name);
        return Ok(Some(Continuing {
            card: series,
            plays: p.media_item_id,
            is_show: true,
            note: Some(note(false, season, episode, named)),
            progress: progress(p),
            resume_at: Some(p.position_seconds).filter(|s| *s > 0),
            season,
            episode,
            episode_title: named.map(str::to_string),
        }));
    }
    // Finished: the next episode, from its start; none after it, and the show is done.
    let Some(next) = EpisodeRepository::new(db)
        .next_after(p.media_item_id)
        .await?
    else {
        return Ok(None);
    };
    let named = episode_name(&next.title);
    Ok(Some(Continuing {
        card: series,
        plays: next.media_item_id,
        is_show: true,
        note: Some(note(true, next.season_number, next.episode_number, named)),
        progress: None,
        resume_at: None,
        season: next.season_number,
        episode: next.episode_number,
        episode_title: named.map(str::to_string),
    }))
}

fn progress(p: &PlaybackPosition) -> Option<f64> {
    p.duration_seconds
        .filter(|d| *d > 0)
        .map(|d| (p.position_seconds as f64 / d as f64).clamp(0.0, 1.0))
}

/// Continue watching for a profile, most recent first, one card per film or show, at most
/// `limit`. Only a show's latest episode counts: an older half-watched one is behind it.
pub async fn continuing(db: &Db, profile: i64, limit: usize) -> Result<Vec<Continuing>, DbError> {
    let positions = ProfileRepository::new(db)
        .recent_positions(profile, READ)
        .await?;
    let episodes = EpisodeRepository::new(db);
    // Removed by the viewer (ADR-0056), until played again.
    let hidden: HashSet<i64> = ProfileRepository::new(db)
        .hidden_from_continue(profile)
        .await?
        .into_iter()
        .collect();
    let mut cards = HashSet::new();
    let mut out = Vec::new();
    for p in positions {
        if out.len() >= limit {
            break;
        }
        let placement = episodes.placement(p.media_item_id).await?;
        let card = placement.map_or(p.media_item_id, |(series, _, _)| series);
        if !cards.insert(card) || hidden.contains(&card) {
            continue;
        }
        if let Some(c) = resolve(db, &p, placement).await? {
            out.push(c);
        }
    }
    Ok(out)
}

/// Continue watching for the Films page (`shows` false) or the TV Shows page (Phase 16, the
/// author: "filter out what doesn't belong there"); a kids profile sees only what suits
/// children, as on Home.
pub async fn of_kind(
    db: &Db,
    profile: i64,
    shows: bool,
    kids: bool,
    limit: usize,
) -> Result<Vec<Continuing>, DbError> {
    let mut picked: Vec<Continuing> = continuing(db, profile, 100)
        .await?
        .into_iter()
        .filter(|c| c.is_show == shows)
        .collect();
    if kids {
        let ids: Vec<i64> = picked.iter().map(|c| c.card).collect();
        let ok = MediaRepository::new(db).suitable_for_children(&ids).await?;
        picked.retain(|c| ok.contains(&c.card));
    }
    picked.truncate(limit);
    Ok(picked)
}

/// The episode of one show to watch next for a profile: its page's Resume (Phase 16).
/// `None` when nothing of it was started, or the last episode was finished.
pub async fn for_series(db: &Db, profile: i64, series: i64) -> Result<Option<Continuing>, DbError> {
    let Some(p) = ProfileRepository::new(db)
        .latest_in_series(profile, series)
        .await?
    else {
        return Ok(None);
    };
    let placement = EpisodeRepository::new(db)
        .placement(p.media_item_id)
        .await?;
    resolve(db, &p, placement).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placeholder_is_not_a_name() {
        assert_eq!(episode_name("Episode #1.2"), None);
        assert_eq!(episode_name("Episode 7"), None);
        assert_eq!(episode_name("  "), None);
        assert_eq!(episode_name("Traces to Nowhere"), Some("Traces to Nowhere"));
        assert_eq!(episode_name("Episode IV"), Some("Episode IV"));
        assert_eq!(episode_name("Episode"), Some("Episode"));
    }

    #[test]
    fn the_note_says_where_and_what() {
        assert_eq!(
            note(false, Some(1), Some(2), Some("Traces to Nowhere")),
            "S1 E2 · Traces to Nowhere"
        );
        assert_eq!(note(true, Some(1), Some(3), None), "Next: S1 E3");
        assert_eq!(note(false, None, Some(4), None), "E4");
    }
}
