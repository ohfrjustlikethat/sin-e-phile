//! What the Sources settings screen shows: every backend, in order, in plain words.
//!
//! Here, not in TypeScript, because the wording is a product decision and the health
//! line is arithmetic over stored counts — both are things a test should hold still.

use std::sync::Arc;

use sinephile_metadata_api::Transport;
use sinephile_persistence::repositories::{BackendRow, DirectLinkRepository, SourceRepository};
use sinephile_persistence::{Db, DbError};
use source_protocol::{Manifest, SourceBackend};
use url::Url;

use crate::backends::{self, addon::backend_id, direct::DirectLink, Context};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub blurb: String,
    /// The host an addon sends queries to; `None` for a built-in.
    pub addon_host: Option<String>,
    pub enabled: bool,
    /// "Answered 12 of 14 searches, usually in 340 ms." and the like.
    pub health: String,
    /// The last problem, in the backend's own words, if the last answer was one.
    pub last_error: Option<String>,
}

/// Built-in backends' names and one-line descriptions. An id not listed here is an
/// addon, whose manifest supplies both.
fn built_in(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        backends::local::ID => (
            "Your files",
            "Films already on this computer. Folders are added in a later version.",
        ),
        backends::archive::ID => (
            "Internet Archive",
            "Public-domain feature films from the Internet Archive's own collection.",
        ),
        backends::direct::ID => (
            "Your links",
            "Web addresses and playlists you add yourself.",
        ),
        _ => return None,
    })
}

/// What to call a backend in a sentence: "Found in Internet Archive".
pub fn display_name(id: &str, addons: &[(Url, Manifest)]) -> String {
    if let Some((name, _)) = built_in(id) {
        return name.to_string();
    }
    addons
        .iter()
        .find(|(_, m)| backend_id(m) == id)
        .map(|(_, m)| m.name.clone())
        .unwrap_or_else(|| id.to_string())
}

/// The health line. Counts, phrased: no score, because a score would need a meaning
/// Phase 9 has not given it yet.
pub fn health_line(row: &BackendRow) -> String {
    let asked = row.successes + row.failures + row.timeouts;
    if asked == 0 {
        return "Not asked yet.".into();
    }
    let mut line = format!(
        "Answered {} of {} {}",
        row.successes,
        asked,
        if asked == 1 { "search" } else { "searches" }
    );
    if let Some(ms) = row.latency_ms.filter(|_| row.successes > 0) {
        line.push_str(&format!(", usually in {}", human_ms(ms)));
    }
    line.push('.');
    if row.timeouts > 0 {
        line.push_str(&format!(
            " Ran out of time {}.",
            if row.timeouts == 1 {
                "once".to_string()
            } else {
                format!("{} times", row.timeouts)
            }
        ));
    }
    line
}

fn human_ms(ms: f64) -> String {
    if ms < 1000.0 {
        format!("{} ms", (ms / 10.0).round() as i64 * 10)
    } else {
        format!("{:.1} s", ms / 1000.0)
    }
}

pub async fn overview(
    db: &Db,
    backends: &[Arc<dyn SourceBackend>],
    addons: &[(Url, Manifest)],
) -> Result<Vec<SourceSummary>, DbError> {
    let repo = SourceRepository::new(db);
    let ids: Vec<&str> = backends.iter().map(|b| b.id()).collect();
    repo.ensure(&ids).await?;
    Ok(repo
        .all()
        .await?
        .into_iter()
        // A row with no backend behind it — an addon whose manifest no longer passes —
        // is not offered as if it worked.
        .filter(|row| ids.contains(&row.id.as_str()))
        .map(|row| {
            let (name, blurb, host) = match built_in(&row.id) {
                Some((n, b)) => (n.to_string(), b.to_string(), None),
                None => addons
                    .iter()
                    .find(|(_, m)| backend_id(m) == row.id)
                    .map(|(url, m)| {
                        (
                            m.name.clone(),
                            m.description.clone(),
                            url.host_str().map(String::from),
                        )
                    })
                    .unwrap_or_else(|| (row.id.clone(), String::new(), None)),
            };
            SourceSummary {
                health: health_line(&row),
                last_error: row
                    .last_error
                    .clone()
                    .filter(|_| row.last_outcome_at.is_some()),
                id: row.id,
                name,
                blurb,
                addon_host: host,
                enabled: row.enabled,
            }
        })
        .collect())
}

/// Everything backends draw on, read fresh from the database: the user's links and
/// the installed addons, re-checked. Returns the ids of stored addons that were
/// refused, so the app can say so rather than silently lose one.
pub async fn context_from(
    db: &Db,
    http: Arc<dyn Transport>,
    untrusted_http: Arc<dyn Transport>,
) -> Result<(Context, Vec<String>), DbError> {
    let direct_links = DirectLinkRepository::new(db)
        .all()
        .await?
        .into_iter()
        .map(|r| DirectLink {
            title: r.title,
            year: r.year.and_then(|y| u16::try_from(y).ok()),
            url: r.url,
        })
        .collect();
    let (addons, refused) = crate::addons::installed(db).await?;
    Ok((
        Context {
            http,
            untrusted_http,
            direct_links,
            addons,
        },
        refused,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(s: i64, f: i64, t: i64, ms: Option<f64>) -> BackendRow {
        BackendRow {
            id: "x".into(),
            enabled: true,
            position: 0,
            successes: s,
            failures: f,
            timeouts: t,
            latency_ms: ms,
            last_error: None,
            last_outcome_at: None,
        }
    }

    #[test]
    fn the_health_line_reads_like_a_sentence() {
        assert_eq!(health_line(&row(0, 0, 0, None)), "Not asked yet.");
        assert_eq!(
            health_line(&row(1, 0, 0, Some(342.0))),
            "Answered 1 of 1 search, usually in 340 ms."
        );
        assert_eq!(
            health_line(&row(12, 1, 1, Some(2450.0))),
            "Answered 12 of 14 searches, usually in 2.5 s. Ran out of time once."
        );
        assert_eq!(
            health_line(&row(0, 0, 3, Some(9.0))),
            "Answered 0 of 3 searches. Ran out of time 3 times.",
            "no latency is quoted for a backend that never answered"
        );
    }
}
