//! A taste's axes (SPEC.md Phase 15): what it leans towards and away from, in terms a
//! person would use. From the weighted signals: each liked title adds its weight to its
//! decade, its length, its genres and its directors; each disliked one, to the same things
//! on the other side. The taste statements are priors: a profile that said "older films"
//! starts there until what it does says otherwise.
//!
//! Countries and original languages are not in the offline catalogue (IMDb's datasets do not
//! carry them), so those axes, and subtitle tolerance learned from behaviour, wait for
//! enrichment (D84); the statements stand in for them meanwhile.

use std::collections::HashMap;

use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::{Db, DbError};

use crate::Statements;

/// A length band, as a person says it.
pub fn length_band(minutes: i64) -> &'static str {
    match minutes {
        0..=89 => "short",
        90..=129 => "standard",
        _ => "long",
    }
}

/// A decade, as a person says it: "the 1970s".
pub fn decade(year: i64) -> i64 {
    year.div_euclid(10) * 10
}

/// Weighted shares of the values, strongest first, those of at least `floor` only.
pub fn shares(values: &[(String, f64)], floor: f64) -> Vec<(String, f64)> {
    let total: f64 = values.iter().map(|(_, w)| w.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut by: HashMap<&str, f64> = HashMap::new();
    for (v, w) in values {
        *by.entry(v.as_str()).or_default() += w.max(0.0);
    }
    let mut out: Vec<(String, f64)> = by
        .into_iter()
        .map(|(v, w)| (v.to_string(), w / total))
        .filter(|(_, s)| *s >= floor)
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Axes {
    /// Decades liked, with their share: [("1970", 0.31), …].
    pub decades: Vec<(String, f64)>,
    /// "short" | "standard" | "long", with shares.
    pub lengths: Vec<(String, f64)>,
    pub genres: Vec<(String, f64)>,
    pub directors: Vec<(String, f64)>,
    /// What it turns away from: genres and directors of what was abandoned early or rated low.
    pub avoids: Vec<(String, f64)>,
}

/// The statements as priors: a statement adds this much weight to its side.
pub const PRIOR: f64 = 3.0;

/// Axes from weighed titles (`modes::weigh`) and the profile's statements.
pub async fn axes(db: &Db, weighed: &[(i64, f64)], said: &Statements) -> Result<Axes, DbError> {
    let media = MediaRepository::new(db);
    let (mut decades, mut lengths, mut genres, mut directors, mut avoids) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (id, w) in weighed {
        let Some(m) = media.by_id(*id).await? else {
            continue;
        };
        let names: Vec<String> = media
            .credits(*id, 8)
            .await?
            .into_iter()
            .filter(|c| c.role == "director")
            .map(|c| c.name)
            .collect();
        let kinds = media.genres(*id).await?;
        if *w > 0.0 {
            if let Some(y) = m.release_year {
                decades.push((decade(y).to_string(), *w));
            }
            if let Some(r) = m.runtime_minutes {
                lengths.push((length_band(r).to_string(), *w));
            }
            genres.extend(kinds.into_iter().map(|g| (g, *w)));
            directors.extend(names.into_iter().map(|d| (d, *w)));
        } else if *w < 0.0 {
            avoids.extend(kinds.into_iter().chain(names).map(|g| (g, -*w)));
        }
    }
    // The statements: "older films" or "recent films", "short" or "long is welcome".
    match said.era {
        Some(-1) => decades.extend(
            (1920..=1970)
                .step_by(10)
                .map(|d| (d.to_string(), PRIOR / 6.0)),
        ),
        Some(1) => decades.extend(
            (2000..=2020)
                .step_by(10)
                .map(|d| (d.to_string(), PRIOR / 3.0)),
        ),
        _ => {}
    }
    match said.runtime {
        Some(-1) => lengths.push(("short".into(), PRIOR)),
        Some(1) => lengths.push(("long".into(), PRIOR)),
        _ => {}
    }
    Ok(Axes {
        decades: shares(&decades, 0.08),
        lengths: shares(&lengths, 0.1),
        genres: shares(&genres, 0.06),
        directors: shares(&directors, 0.03),
        avoids: shares(&avoids, 0.1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_are_weighted_and_the_small_left_out() {
        let s = shares(
            &[
                ("Horror".into(), 3.0),
                ("Drama".into(), 1.0),
                ("Horror".into(), 1.0),
                ("War".into(), 0.1),
            ],
            0.05,
        );
        assert_eq!(s[0].0, "Horror");
        assert!((s[0].1 - 4.0 / 5.1).abs() < 1e-9);
        assert!(s.iter().all(|(v, _)| v != "War"), "under the floor");
        assert!(shares(&[], 0.1).is_empty());
    }

    #[test]
    fn decades_and_lengths_read_as_a_person_says_them() {
        assert_eq!(decade(1977), 1970);
        assert_eq!(decade(2009), 2000);
        assert_eq!(
            (length_band(85), length_band(110), length_band(170)),
            ("short", "standard", "long")
        );
    }
}
