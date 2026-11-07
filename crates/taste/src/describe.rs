//! A taste in plain language (SPEC.md Phase 15): "You gravitate toward horror, mostly from
//! the 1970s and 80s (Halloween, Alien, The Shining)". The model is shown so it can be
//! trusted and corrected, so it is said the way a person would say it, from what it holds.

use std::collections::HashMap;

use sinephile_persistence::repositories::{CredentialRepository, MediaRepository};
use sinephile_persistence::{Db, DbError};

use crate::axes::{decade, shares, Axes};
use crate::modes::Mode;

/// What one mode holds, for describing it.
#[derive(Debug, Clone, Default)]
pub struct ModeFacts {
    /// Weighted shares, strongest first.
    pub genres: Vec<(String, f64)>,
    pub decades: Vec<(String, f64)>,
    pub directors: Vec<(String, f64)>,
    /// Its best-known members, most-voted first.
    pub examples: Vec<String>,
}

/// A genre as said in a sentence; a name (an avoided director) as it is.
fn said(g: &str) -> String {
    const GENRES: &[&str] = &[
        "Action",
        "Adventure",
        "Animation",
        "Biography",
        "Comedy",
        "Crime",
        "Documentary",
        "Drama",
        "Family",
        "Fantasy",
        "History",
        "Horror",
        "Music",
        "Musical",
        "Mystery",
        "Romance",
        "Sport",
        "Thriller",
        "War",
        "Western",
        "Children",
        "Adult",
        "News",
        "Reality-TV",
        "Talk-Show",
        "Game-Show",
        "Short",
    ];
    match g {
        "Sci-Fi" => "science fiction".to_string(),
        "Film-Noir" => "film noir".to_string(),
        g if GENRES.contains(&g) => g.to_lowercase(),
        other => other.to_string(),
    }
}

/// "horror", "drama and romance": the genres that make up at least a fifth, at most two.
fn genre_words(genres: &[(String, f64)]) -> Option<String> {
    let picked: Vec<String> = genres
        .iter()
        .filter(|(_, s)| *s >= 0.2)
        .take(2)
        .map(|(g, _)| said(g))
        .collect();
    match picked.as_slice() {
        [] => None,
        [one] => Some(one.clone()),
        [a, b] => Some(format!("{a} and {b}")),
        _ => None,
    }
}

/// "from the 1970s", "from the 1970s and 80s": decades that make up at least a quarter.
fn decade_words(decades: &[(String, f64)]) -> Option<String> {
    let mut picked: Vec<i64> = decades
        .iter()
        .filter(|(_, s)| *s >= 0.25)
        .take(2)
        .filter_map(|(d, _)| d.parse().ok())
        .collect();
    picked.sort_unstable();
    match picked.as_slice() {
        [] => None,
        [d] => Some(format!("from the {d}s")),
        // "the 1970s and 80s", but "the 2000s and 2010s": "the 2000s and 10s" reads oddly.
        [a, b] if a / 100 == b / 100 && a % 100 != 0 => {
            Some(format!("from the {a}s and {}s", b % 100))
        }
        [a, b] => Some(format!("from the {a}s and {b}s")),
        _ => None,
    }
}

/// One mode in a sentence. `first`: the strongest mode, said as what you gravitate toward;
/// the others as what you also love.
pub fn mode_sentence(f: &ModeFacts, first: bool) -> String {
    let what = genre_words(&f.genres).unwrap_or_else(|| "a mix of things".to_string());
    let mut s = if first {
        format!("You gravitate toward {what}")
    } else {
        format!("You also love {what}")
    };
    if let Some(when) = decade_words(&f.decades) {
        s.push_str(&format!(", mostly {when}"));
    }
    if let Some((who, share)) = f.directors.first() {
        if *share >= 0.15 {
            s.push_str(&format!(", {who} above all"));
        }
    }
    if !f.examples.is_empty() {
        s.push_str(&format!(
            " ({})",
            f.examples
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    s.push('.');
    s
}

/// The axes in sentences: length, and what is turned away from.
pub fn axes_sentences(lengths: &[(String, f64)], avoids: &[(String, f64)]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some((band, share)) = lengths.first() {
        if *share >= 0.5 {
            out.push(match band.as_str() {
                "short" => "You like films under an hour and a half.".to_string(),
                "long" => "Long films don't put you off.".to_string(),
                _ => "Most of what you love runs between an hour and a half and two hours."
                    .to_string(),
            });
        }
    }
    let away: Vec<String> = avoids.iter().take(2).map(|(a, _)| said(a)).collect();
    match away.as_slice() {
        [] => {}
        [one] => out.push(format!("You tend to walk out of {one}.")),
        [a, b] => out.push(format!("You tend to walk out of {a} and {b}.")),
        _ => {}
    }
    out
}

/// A correction to a mode, keyed by its anchor (its strongest member, which stays put while
/// the mode does): "more", "less" or "not_me".
pub type Corrections = HashMap<i64, String>;

/// How a correction changes a mode's weight.
pub fn correction_factor(c: Option<&str>) -> f64 {
    match c {
        Some("more") => 2.0,
        Some("less") => 0.5,
        Some("not_me") => 0.0,
        _ => 1.0,
    }
}

/// Apply corrections to modes (weights rescaled to sum to one; a mode set to "not me" goes).
pub fn apply(modes: &mut Vec<Mode>, corrections: &Corrections) {
    for m in modes.iter_mut() {
        if let Some(anchor) = m.members.first() {
            m.weight *= correction_factor(corrections.get(anchor).map(String::as_str));
        }
    }
    modes.retain(|m| m.weight > 0.0);
    let total: f64 = modes.iter().map(|m| m.weight).sum();
    if total > 0.0 {
        for m in modes.iter_mut() {
            m.weight /= total;
        }
    }
}

/// Where a profile's corrections are kept.
pub const CORRECTIONS: &str = "taste.corrections";

/// A profile's corrections; none when unset or unreadable.
pub async fn corrections(db: &Db, profile: i64) -> Result<Corrections, DbError> {
    let raw = CredentialRepository::new(db)
        .get(profile, CORRECTIONS)
        .await?;
    Ok(raw
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default())
}

/// Set or clear (`None`) the correction to the mode anchored at `anchor`.
pub async fn correct(
    db: &Db,
    profile: i64,
    anchor: i64,
    correction: Option<&str>,
) -> Result<(), DbError> {
    let mut all = corrections(db, profile).await?;
    match correction.filter(|c| matches!(*c, "more" | "less" | "not_me")) {
        Some(c) => {
            all.insert(anchor, c.to_string());
        }
        None => {
            all.remove(&anchor);
        }
    }
    let json = serde_json::to_string(&all).unwrap_or_else(|_| "{}".into());
    CredentialRepository::new(db)
        .set(profile, CORRECTIONS, &json)
        .await
}

/// One mode, described.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeDescription {
    /// Its strongest member: what a correction is keyed by.
    pub anchor: i64,
    /// Its share of the taste before corrections, 0 to 1.
    pub share: f64,
    pub sentence: String,
    /// Its three best-known members.
    pub examples: Vec<i64>,
    pub correction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Description {
    pub modes: Vec<ModeDescription>,
    pub axes: Vec<String>,
}

/// How many of a mode's members are read to describe it.
const READ: usize = 40;

/// What a mode holds, from its members' rows.
pub async fn facts(db: &Db, mode: &Mode) -> Result<(ModeFacts, Vec<i64>), DbError> {
    let media = MediaRepository::new(db);
    let (mut genres, mut decades, mut directors, mut known) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for id in mode.members.iter().take(READ) {
        let Some(m) = media.by_id(*id).await? else {
            continue;
        };
        if let Some(y) = m.release_year {
            decades.push((decade(y).to_string(), 1.0));
        }
        genres.extend(media.genres(*id).await?.into_iter().map(|g| (g, 1.0)));
        directors.extend(
            media
                .credits(*id, 8)
                .await?
                .into_iter()
                .filter(|c| c.role == "director")
                .map(|c| (c.name, 1.0)),
        );
        known.push((*id, m.rating_votes.unwrap_or(0), m.primary_title));
    }
    // A genre's share is of the titles, not of all the genre labels: three labels a title.
    let n = known.len().max(1) as f64;
    let mut g = shares(&genres, 0.0);
    let labels = genres.len() as f64;
    for (_, s) in g.iter_mut() {
        *s *= labels / n;
    }
    let mut d = shares(&directors, 0.0);
    let credited = directors.len() as f64;
    for (_, s) in d.iter_mut() {
        *s *= credited / n;
    }
    known.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let examples: Vec<i64> = known.iter().take(3).map(|k| k.0).collect();
    Ok((
        ModeFacts {
            genres: g,
            decades: shares(&decades, 0.0),
            directors: d,
            examples: known.into_iter().take(3).map(|k| k.2).collect(),
        },
        examples,
    ))
}

/// A taste, described: each mode (strongest first) in a sentence, then the axes.
pub async fn describe(
    db: &Db,
    modes: &[Mode],
    axes: &Axes,
    corrections: &Corrections,
) -> Result<Description, DbError> {
    let mut out = Description::default();
    for (i, m) in modes.iter().enumerate() {
        let Some(anchor) = m.members.first().copied() else {
            continue;
        };
        let (f, examples) = facts(db, m).await?;
        out.modes.push(ModeDescription {
            anchor,
            share: m.weight,
            sentence: mode_sentence(&f, i == 0),
            examples,
            correction: corrections.get(&anchor).cloned(),
        });
    }
    out.axes = axes_sentences(&axes.lengths, &axes.avoids);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[(&str, f64)]) -> Vec<(String, f64)> {
        v.iter().map(|(a, b)| (a.to_string(), *b)).collect()
    }

    #[test]
    fn a_mode_is_said_as_a_person_would_say_it() {
        let f = ModeFacts {
            genres: s(&[("Horror", 0.7), ("Thriller", 0.2), ("Drama", 0.1)]),
            decades: s(&[("1970", 0.4), ("1980", 0.35), ("2000", 0.25)]),
            directors: s(&[("John Carpenter", 0.3)]),
            examples: vec![
                "Halloween".into(),
                "The Thing".into(),
                "Alien".into(),
                "Jaws".into(),
            ],
        };
        assert_eq!(
            mode_sentence(&f, true),
            "You gravitate toward horror and thriller, mostly from the 1970s and 80s, John Carpenter above all (Halloween, The Thing, Alien)."
        );
        let g = ModeFacts {
            genres: s(&[("Documentary", 0.9)]),
            decades: s(&[("2010", 0.8)]),
            ..Default::default()
        };
        assert_eq!(
            mode_sentence(&g, false),
            "You also love documentary, mostly from the 2010s."
        );
        assert_eq!(
            mode_sentence(&ModeFacts::default(), false),
            "You also love a mix of things."
        );
        assert_eq!(
            decade_words(&s(&[("1990", 0.5), ("2000", 0.5)])),
            Some("from the 1990s and 2000s".into())
        );
        assert_eq!(
            decade_words(&s(&[("2010", 0.5), ("2000", 0.5)])),
            Some("from the 2000s and 2010s".into())
        );
    }

    #[test]
    fn the_axes_are_sentences_and_only_when_clear() {
        assert_eq!(
            axes_sentences(&s(&[("short", 0.6)]), &s(&[("Horror", 0.5), ("War", 0.3)])),
            vec![
                "You like films under an hour and a half.",
                "You tend to walk out of horror and war."
            ]
        );
        assert_eq!(
            axes_sentences(&[], &s(&[("Sci-Fi", 0.5), ("Ann Director", 0.3)])),
            vec!["You tend to walk out of science fiction and Ann Director."]
        );
        assert!(
            axes_sentences(&s(&[("long", 0.4)]), &[]).is_empty(),
            "not clear enough to say"
        );
    }

    #[test]
    fn corrections_reweigh_and_not_me_removes() {
        let mode = |anchor: i64, w: f64| Mode {
            direction: vec![1.0],
            weight: w,
            members: vec![anchor],
        };
        let mut m = vec![mode(1, 0.5), mode(2, 0.3), mode(3, 0.2)];
        let c: Corrections = [(1, "not_me".to_string()), (3, "more".to_string())]
            .into_iter()
            .collect();
        apply(&mut m, &c);
        assert_eq!(m.len(), 2);
        assert!((m[0].weight - 0.3 / 0.7).abs() < 1e-9 && (m[1].weight - 0.4 / 0.7).abs() < 1e-9);
    }
}
