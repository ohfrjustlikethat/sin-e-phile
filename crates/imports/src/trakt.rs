//! Trakt's export: JSON files of items, each holding a `movie` or a `show` (title, year,
//! `ids.imdb`). `watched-*.json` (`plays`, `last_watched_at`), `history.json` (one item per
//! watch, `watched_at`), `ratings-*.json` (`rating` 1–10, `rated_at`) and `watchlist-*.json`
//! (`listed_at`). A `.zip` of them is read whole and merged per title.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::{Entry, ImportError, Kind};

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Watched,
    History,
    Ratings,
    Watchlist,
}

fn role(file: &str, first: Option<&Value>) -> Option<Role> {
    let f = file.to_ascii_lowercase();
    if f.contains("watchlist") {
        return Some(Role::Watchlist);
    }
    if f.contains("rating") {
        return Some(Role::Ratings);
    }
    if f.contains("history") {
        return Some(Role::History);
    }
    if f.contains("watched") {
        return Some(Role::Watched);
    }
    // Not named as Trakt names them: by what the items carry.
    let item = first?;
    if item.get("rating").is_some() {
        Some(Role::Ratings)
    } else if item.get("listed_at").is_some() {
        Some(Role::Watchlist)
    } else if item.get("watched_at").is_some() {
        Some(Role::History)
    } else if item.get("last_watched_at").is_some() || item.get("plays").is_some() {
        Some(Role::Watched)
    } else {
        None
    }
}

fn day(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?;
    (s.len() >= 10).then(|| s[..10].to_string())
}

type Films = BTreeMap<String, Entry>;

fn add(films: &mut Films, file: &str, items: &[Value]) {
    let Some(role) = role(file, items.first()) else {
        return;
    };
    for item in items {
        // An episode in history counts towards its show.
        let (what, kind) = if let Some(m) = item.get("movie") {
            (m, Kind::Film)
        } else if let Some(s) = item.get("show") {
            (s, Kind::Series)
        } else {
            continue;
        };
        let title = what
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let year = what.get("year").and_then(Value::as_i64);
        let imdb = what
            .get("ids")
            .and_then(|i| i.get("imdb"))
            .and_then(Value::as_str)
            .filter(|i| i.starts_with("tt"))
            .map(str::to_string);
        let key = imdb
            .clone()
            .unwrap_or_else(|| format!("{}|{year:?}", title.to_lowercase()));
        let e = films.entry(key).or_insert_with(|| Entry {
            title: title.to_string(),
            year,
            imdb: imdb.clone(),
            kind: Some(kind),
            ..Default::default()
        });
        match role {
            Role::Ratings => e.rating = item.get("rating").and_then(Value::as_f64),
            Role::Watchlist => e.watchlist = true,
            Role::History => {
                if kind == Kind::Film || e.watches.is_empty() {
                    e.watches.push(day(item.get("watched_at")));
                }
            }
            Role::Watched => {
                if e.watches.is_empty() {
                    e.watches.push(day(item.get("last_watched_at")));
                }
            }
        }
    }
}

fn finish(films: Films) -> Vec<Entry> {
    films
        .into_values()
        .map(|mut e| {
            e.watches.sort();
            e.watches.dedup();
            e
        })
        .collect()
}

pub fn read(text: &str, file: &str) -> Result<Vec<Entry>, ImportError> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| ImportError::Read(file.to_string(), e.to_string()))?;
    let items = v
        .as_array()
        .ok_or_else(|| ImportError::Unknown(file.to_string()))?;
    let mut films = Films::new();
    add(&mut films, file, items);
    Ok(finish(films))
}

pub fn read_zip(path: &Path) -> Result<Vec<Entry>, ImportError> {
    let mut films = Films::new();
    // History first: its dated watches are better than watched-*.json's last date.
    let mut texts: Vec<(String, String)> = crate::zip_texts(path)?.into_iter().collect();
    texts.sort_by_key(|(name, _)| (!name.contains("history"), name.clone()));
    for (name, text) in texts.iter().filter(|(n, _)| n.ends_with(".json")) {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) {
            add(&mut films, name, &items);
        }
    }
    Ok(finish(films))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_ratings_and_watchlist_merge_per_title() {
        let mut films = Films::new();
        add(
            &mut films,
            "watched-movies.json",
            &serde_json::from_str::<Vec<Value>>(
                r#"[{"plays":2,"last_watched_at":"2024-05-01T20:00:00.000Z","movie":{"title":"Heat","year":1995,"ids":{"imdb":"tt0113277"}}}]"#,
            )
            .unwrap(),
        );
        add(
            &mut films,
            "ratings-movies.json",
            &serde_json::from_str::<Vec<Value>>(
                r#"[{"rated_at":"2024-05-02T00:00:00.000Z","rating":9,"movie":{"title":"Heat","year":1995,"ids":{"imdb":"tt0113277"}}},
                    {"rated_at":"2024-05-02T00:00:00.000Z","rating":7,"show":{"title":"Twin Peaks","year":1990,"ids":{}}}]"#,
            )
            .unwrap(),
        );
        add(
            &mut films,
            "watchlist-movies.json",
            &serde_json::from_str::<Vec<Value>>(
                r#"[{"listed_at":"2024-05-03T00:00:00.000Z","movie":{"title":"Thief","year":1981,"ids":{"imdb":"tt0083190"}}}]"#,
            )
            .unwrap(),
        );
        let e = finish(films);
        let heat = e.iter().find(|x| x.title == "Heat").unwrap();
        assert_eq!(heat.rating, Some(9.0));
        assert_eq!(heat.watches, vec![Some("2024-05-01".to_string())]);
        let peaks = e.iter().find(|x| x.title == "Twin Peaks").unwrap();
        assert_eq!(
            (peaks.kind, peaks.imdb.as_deref()),
            (Some(Kind::Series), None)
        );
        assert!(e.iter().find(|x| x.title == "Thief").unwrap().watchlist);
    }

    #[test]
    fn a_file_named_otherwise_is_known_by_its_items() {
        let e = read(
            r#"[{"rated_at":"2024-01-01T00:00:00.000Z","rating":6,"movie":{"title":"Collateral","year":2004,"ids":{"imdb":"tt0369339"}}}]"#,
            "export (1).json",
        )
        .unwrap();
        assert_eq!(e[0].rating, Some(6.0));
    }
}
