//! Phase 14 E2: a Letterboxd export of 500+ films imports correctly.
//!
//! The export is made at run time, never committed: MovieLens and IMDb data may not be
//! redistributed (the same reason no catalogue ships). From the local `ml-25m.zip`: the 600
//! films most rated, their titles as MovieLens writes them, which is not how the catalogue
//! does ("Lion King, The (1994)" becomes "The Lion King", 1994, as Letterboxd shows it),
//! and the IMDb id of each as ground truth. It is written as a Letterboxd `.zip`
//! (watched.csv, ratings.csv, diary.csv, watchlist.csv), read by `sinephile_imports::read`,
//! imported into a temporary profile, judged title by title, and the profile deleted.
//!
//! Right: placed on the film the IMDb id names. Wrong: placed on a different one (the
//! failure that matters: a stranger's film in your history). Unmatched: asked about.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use sinephile_imports::{import, place, read, Entry};
use sinephile_persistence::model::IdSource;
use sinephile_persistence::repositories::{MediaRepository, NewProfile, ProfileRepository};
use sinephile_persistence::Db;

use crate::error::EvalError;

const FILMS: usize = 600;

fn io(path: &Path) -> impl Fn(std::io::Error) -> EvalError + '_ {
    move |source| EvalError::Io {
        path: path.display().to_string(),
        source,
    }
}

fn entry_text(zip: &mut zip::ZipArchive<std::fs::File>, suffix: &str) -> Result<String, EvalError> {
    let i = (0..zip.len())
        .find(|i| zip.name_for_index(*i).is_some_and(|n| n.ends_with(suffix)))
        .ok_or_else(|| EvalError::Missing(format!("no {suffix} in ml-25m.zip")))?;
    let mut text = String::new();
    zip.by_index(i)
        .map_err(|e| EvalError::Missing(e.to_string()))?
        .read_to_string(&mut text)
        .map_err(|e| EvalError::Missing(e.to_string()))?;
    Ok(text)
}

/// "Lion King, The (1994)" -> ("The Lion King", 1994); "Spirited Away (Sen to Chihiro no
/// kamikakushi) (2001)" -> ("Spirited Away", 2001): the English name, article in front.
pub fn letterboxd_name(movielens: &str) -> Option<(String, i64)> {
    let t = movielens.trim();
    let open = t.rfind(" (")?;
    let year: i64 = t[open + 2..].trim_end_matches(')').parse().ok()?;
    let mut name = t[..open].trim().to_string();
    // Every alternative title in parentheses ("(a.k.a. The Professional) (Léon)").
    while name.ends_with(')') {
        match name.rfind(" (") {
            Some(alt) if alt > 0 => name.truncate(alt),
            _ => break,
        }
    }
    for article in [
        "The", "A", "An", "Les", "La", "Le", "L'", "Il", "El", "Das", "Die", "Der",
    ] {
        if let Some(stem) = name.strip_suffix(&format!(", {article}")) {
            name = if article.ends_with('\'') {
                format!("{article}{stem}")
            } else {
                format!("{article} {stem}")
            };
            break;
        }
    }
    Some((name, year))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// `from`: how many of the most-rated films to skip, so a set chosen after a change is
/// disjoint from the one the change was made on (0 = the first 600, 600 = the next).
pub async fn run(data_dir: &Path, from: usize) -> Result<bool, EvalError> {
    let path = data_dir.join("datasets").join("ml-25m.zip");
    if !path.is_file() {
        return Err(EvalError::Missing(format!(
            "{} (run ingest movielens first)",
            path.display()
        )));
    }
    println!(
        "import — a Letterboxd export of {FILMS} films (ranks {} to {} by ratings), made from the local MovieLens data\n",
        from + 1,
        from + FILMS
    );
    let file = std::fs::File::open(&path).map_err(io(&path))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| EvalError::Missing(e.to_string()))?;

    // The most-rated films: the ones a real export is made of.
    let mut counts: HashMap<i64, u32> = HashMap::new();
    {
        let i = (0..zip.len())
            .find(|i| {
                zip.name_for_index(*i)
                    .is_some_and(|n| n.ends_with("ratings.csv"))
            })
            .ok_or_else(|| EvalError::Missing("no ratings.csv".into()))?;
        let reader = BufReader::with_capacity(
            1 << 20,
            zip.by_index(i)
                .map_err(|e| EvalError::Missing(e.to_string()))?,
        );
        for line in reader.lines().skip(1).map_while(Result::ok) {
            if let Some(id) = line.split(',').nth(1).and_then(|m| m.parse().ok()) {
                *counts.entry(id).or_default() += 1;
            }
        }
    }
    let movies = entry_text(&mut zip, "movies.csv")?;
    let links = entry_text(&mut zip, "links.csv")?;
    let imdb: HashMap<i64, String> = links
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.split(',');
            let id = f.next()?.parse().ok()?;
            let tt: u32 = f.next()?.parse().ok()?;
            Some((id, format!("tt{tt:07}")))
        })
        .collect();
    let mut films: Vec<(u32, String, i64, String)> = sinephile_imports::csv::records(&movies)
        .into_iter()
        .skip(1)
        .filter_map(|r| {
            let id: i64 = r.first()?.parse().ok()?;
            let (name, year) = letterboxd_name(r.get(1)?)?;
            Some((*counts.get(&id)?, name, year, imdb.get(&id)?.clone()))
        })
        .collect();
    films.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let films: Vec<_> = films.into_iter().skip(from).take(FILMS).collect();

    // The export, shaped as Letterboxd writes it.
    let dir = tempfile::tempdir().map_err(io(data_dir))?;
    let export = dir.path().join("letterboxd-eval.zip");
    {
        let f = std::fs::File::create(&export).map_err(io(&export))?;
        let mut z = zip::ZipWriter::new(f);
        let opt = zip::write::SimpleFileOptions::default();
        let mut watched = String::from("Date,Name,Year,Letterboxd URI\n");
        let mut ratings = String::from("Date,Name,Year,Letterboxd URI,Rating\n");
        let mut diary =
            String::from("Date,Name,Year,Letterboxd URI,Rating,Rewatch,Tags,Watched Date\n");
        let mut watchlist = String::from("Date,Name,Year,Letterboxd URI\n");
        for (i, (_, name, year, _)) in films.iter().enumerate() {
            let n = csv_field(name);
            if i % 12 == 11 {
                watchlist.push_str(&format!("2024-02-01,{n},{year},\n"));
                continue;
            }
            watched.push_str(&format!("2024-01-01,{n},{year},\n"));
            if i % 3 != 0 {
                let stars = 0.5 * (1 + i % 10) as f64;
                ratings.push_str(&format!("2024-01-01,{n},{year},,{stars}\n"));
            }
            if i % 5 == 0 {
                diary.push_str(&format!(
                    "2024-01-01,{n},{year},,,,,2023-0{}-1{}\n",
                    1 + i % 9,
                    i % 10
                ));
            }
        }
        for (name, text) in [
            ("watched.csv", watched),
            ("ratings.csv", ratings),
            ("diary.csv", diary),
            ("watchlist.csv", watchlist),
        ] {
            z.start_file(name, opt)
                .map_err(|e| EvalError::Missing(e.to_string()))?;
            z.write_all(text.as_bytes()).map_err(io(&export))?;
        }
        z.finish().map_err(|e| EvalError::Missing(e.to_string()))?;
    }

    let db = Db::open_in(data_dir).await?;
    let media = MediaRepository::new(&db);
    let (_, entries) = read(&export).map_err(|e| EvalError::Missing(e.to_string()))?;
    let truth: HashMap<(String, i64), String> = films
        .iter()
        .map(|(_, name, year, tt)| ((name.to_lowercase(), *year), tt.clone()))
        .collect();

    let (mut right, mut wrong, mut unmatched, mut absent) = (0, 0, 0, 0);
    let mut wrong_rows = Vec::new();
    let mut asked_rows = Vec::new();
    let mut reasons: HashMap<String, usize> = HashMap::new();
    for e in &entries {
        let Some(tt) = truth.get(&(e.title.to_lowercase(), e.year.unwrap_or(0))) else {
            continue;
        };
        let expected = media
            .by_external_id(IdSource::Imdb, tt)
            .await?
            .map(|m| m.id);
        if expected.is_none() {
            // Not in this catalogue at all: nothing to be right about.
            absent += 1;
            continue;
        }
        match place(&db, e).await? {
            Ok(id) if Some(id) == expected => right += 1,
            Ok(id) => {
                wrong += 1;
                wrong_rows.push(format!("{} ({:?}) -> item {id}, not {tt}", e.title, e.year));
            }
            Err(why) => {
                unmatched += 1;
                asked_rows.push(format!("{} ({:?}): {why}", e.title, e.year));
                *reasons
                    .entry(why.split(" from ").next().unwrap_or(&why).to_string())
                    .or_default() += 1;
            }
        }
    }
    let judged = right + wrong + unmatched;

    // And the import itself, into a profile made for it and deleted after.
    let profiles = ProfileRepository::new(&db);
    let name = format!("eval-import-{}", std::process::id());
    let me = profiles
        .create_with(&NewProfile {
            name: &name,
            ..Default::default()
        })
        .await?;
    let t0 = std::time::Instant::now();
    let summary = import(&db, me, &entries, "2026-11-07 12:00:00").await;
    let took = t0.elapsed().as_secs_f64();
    let again = import(&db, me, &entries, "2026-11-08 12:00:00").await;
    profiles.delete(me).await?;
    let summary = summary?;
    let again = again?;

    let pct = |n: usize| 100.0 * n as f64 / judged.max(1) as f64;
    println!("  {} films in the export ({} watched, {} on the watchlist); {absent} not in this catalogue", entries.len(), entries.iter().filter(|e: &&Entry| !e.watches.is_empty()).count(), entries.iter().filter(|e| e.watchlist).count());
    println!(
        "  right      {right:>4} = {:.2}%   (target at least 95%)",
        pct(right)
    );
    println!(
        "  wrong      {wrong:>4} = {:.2}%   (target under 1%: a stranger's film in your history)",
        pct(wrong)
    );
    println!("  asked      {unmatched:>4} = {:.2}%", pct(unmatched));
    let mut reasons: Vec<_> = reasons.into_iter().collect();
    reasons.sort_by_key(|r| std::cmp::Reverse(r.1));
    for (why, n) in reasons {
        println!("               {n:>4}  {why}");
    }
    for w in &wrong_rows {
        println!("    wrong: {w}");
    }
    for a in &asked_rows {
        println!("    asked: {a}");
    }
    println!(
        "  recorded   {} ratings, {} watches, {} on the watchlist in {:.1} s; imported again: {} new watches",
        summary.ratings, summary.watches, summary.watchlist, took, again.watches
    );
    let ok = entries.len() >= 500 && pct(right) >= 95.0 && pct(wrong) < 1.0 && again.watches == 0;
    println!("\n  E2 {}", if ok { "MET" } else { "NOT MET" });
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movielens_titles_read_as_letterboxd_shows_them() {
        assert_eq!(
            letterboxd_name("Lion King, The (1994)"),
            Some(("The Lion King".into(), 1994))
        );
        assert_eq!(
            letterboxd_name("Spirited Away (Sen to Chihiro no kamikakushi) (2001)"),
            Some(("Spirited Away".into(), 2001))
        );
        assert_eq!(letterboxd_name("Heat (1995)"), Some(("Heat".into(), 1995)));
        assert_eq!(
            letterboxd_name("Grand Illusion (Grande illusion, La) (1937)"),
            Some(("Grand Illusion".into(), 1937))
        );
        assert_eq!(
            letterboxd_name("Léon: The Professional (a.k.a. The Professional) (Léon) (1994)"),
            Some(("Léon: The Professional".into(), 1994))
        );
        assert_eq!(letterboxd_name("no year"), None);
    }
}
