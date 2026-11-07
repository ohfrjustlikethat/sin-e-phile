//! Letterboxd's export: a `.zip` whose top level holds `watched.csv` (Date, Name, Year,
//! Letterboxd URI), `ratings.csv` (… Rating, 0.5 to 5), `diary.csv` (… Rating, Rewatch,
//! Tags, Watched Date) and `watchlist.csv`. Subfolders (`deleted/`, `orphaned/`, `lists/`,
//! `likes/`) are not the person's history and are not read.
//!
//! One film appears in several files. Merged per (name, year): the diary's dated watches
//! (rewatches included) when it has any, otherwise the one watch `watched.csv` logged; the
//! rating from `ratings.csv`, otherwise the diary's latest; the watchlist as listed. The
//! URI is not used: matching is by title and year.

use std::collections::BTreeMap;
use std::path::Path;

use crate::csv::Table;
use crate::{Entry, ImportError, Kind};

#[derive(Default)]
struct Film {
    title: String,
    year: Option<i64>,
    logged: Option<String>,
    diary: Vec<String>,
    rating: Option<f64>,
    diary_rating: Option<f64>,
    watched: bool,
    watchlist: bool,
}

fn day(s: Option<&str>) -> Option<String> {
    let s = s?;
    (s.len() >= 10 && s.as_bytes()[4] == b'-').then(|| s[..10].to_string())
}

fn key(t: &Table, row: &[String]) -> Option<(String, Option<i64>)> {
    let name = t.get(row, "name")?;
    Some((
        name.to_lowercase(),
        t.get(row, "year").and_then(|y| y.parse().ok()),
    ))
}

#[derive(Clone, Copy)]
enum File {
    Watched,
    Ratings,
    Diary,
    Watchlist,
}

fn add(films: &mut BTreeMap<(String, Option<i64>), Film>, which: File, t: &Table) {
    for row in &t.rows {
        let Some(k) = key(t, row) else {
            continue;
        };
        let f = films.entry(k.clone()).or_insert_with(|| Film {
            title: t.get(row, "name").unwrap_or_default().to_string(),
            year: k.1,
            ..Default::default()
        });
        let rating = t
            .get(row, "rating")
            .and_then(|r| r.parse::<f64>().ok())
            .map(|r| r * 2.0);
        match which {
            File::Watched => {
                f.watched = true;
                f.logged = day(t.get(row, "date"));
            }
            File::Ratings => {
                // Letterboxd only lets you rate what you have watched.
                f.watched = true;
                f.rating = rating.or(f.rating);
            }
            File::Diary => {
                f.watched = true;
                if let Some(d) = day(t.get(row, "watched date").or(t.get(row, "date"))) {
                    f.diary.push(d);
                }
                if rating.is_some() {
                    f.diary_rating = rating;
                }
            }
            File::Watchlist => f.watchlist = true,
        }
    }
}

fn entries(films: BTreeMap<(String, Option<i64>), Film>) -> Vec<Entry> {
    films
        .into_values()
        .map(|mut f| {
            f.diary.sort();
            f.diary.dedup();
            let watches = if !f.diary.is_empty() {
                f.diary.into_iter().map(Some).collect()
            } else if f.watched {
                vec![f.logged]
            } else {
                Vec::new()
            };
            Entry {
                title: f.title,
                year: f.year,
                kind: Some(Kind::Film),
                rating: f.rating.or(f.diary_rating),
                watches,
                watchlist: f.watchlist,
                ..Default::default()
            }
        })
        .collect()
}

pub fn read_zip(path: &Path) -> Result<Vec<Entry>, ImportError> {
    let texts = crate::zip_texts(path)?;
    // Top level only: the export nests it under no folder, or under one named for the user.
    let top = |name: &str| {
        texts.iter().find_map(|(k, v)| {
            let parts: Vec<&str> = k.split('/').collect();
            let at_top = parts.len() == 1
                || (parts.len() == 2
                    && !matches!(parts[0], "deleted" | "orphaned" | "lists" | "likes"));
            (at_top && parts.last() == Some(&name)).then_some(v.as_str())
        })
    };
    let mut films = BTreeMap::new();
    let mut any = false;
    for (name, which) in [
        ("watched.csv", File::Watched),
        ("diary.csv", File::Diary),
        ("ratings.csv", File::Ratings),
        ("watchlist.csv", File::Watchlist),
    ] {
        if let Some(t) = top(name).and_then(Table::parse) {
            add(&mut films, which, &t);
            any = true;
        }
    }
    if !any {
        return Err(ImportError::Unknown(format!(
            "{}: no watched.csv, diary.csv, ratings.csv or watchlist.csv",
            path.display()
        )));
    }
    Ok(entries(films))
}

/// One of the export's CSVs on its own, told apart by its file name.
pub fn read_one(file: &str, table: &Table) -> Vec<Entry> {
    let which = if file.contains("watchlist") {
        File::Watchlist
    } else if file.contains("rating") {
        File::Ratings
    } else if file.contains("diary") || table.has("watched date") {
        File::Diary
    } else {
        File::Watched
    };
    let mut films = BTreeMap::new();
    add(&mut films, which, table);
    entries(films)
}
