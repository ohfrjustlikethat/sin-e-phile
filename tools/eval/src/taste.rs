//! Phase 14 E2, the second half: an imported history "is reflected in the taste model".
//!
//! Real tastes, not invented ones: from the local `ml-25m.zip`, the first three MovieLens
//! users with 600 to 900 ratings. Each one's history is written as a Letterboxd export
//! (their own half-star ratings, which is Letterboxd's scale, and their own dates),
//! imported through `sinephile_imports` into a temporary profile, and judged by five-fold
//! cross-validation: the seed is built from four fifths of what they liked, and the test
//! is whether the fifth held out of it comes up among the 500 titles nearest the seed.
//!
//! Compared with two baselines on the same 500 slots: chance (500 of every indexed title)
//! and popularity (the 500 most-voted titles, the honest one: a model that cannot beat
//! "show the most popular" reflects nobody's taste). Nothing is committed; the profiles are
//! deleted after.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use sinephile_imports::{import, read};
use sinephile_persistence::repositories::{CatalogueRepository, NewProfile, ProfileRepository};
use sinephile_persistence::Db;

use crate::error::EvalError;
use crate::imports::letterboxd_name;

const K: usize = 500;
const FOLDS: usize = 5;
const USERS: usize = 3;

fn missing(e: impl std::fmt::Display) -> EvalError {
    EvalError::Missing(e.to_string())
}

struct Rating {
    movie: i64,
    stars: f64,
    day: String,
}

/// "2015-03-04" for a Unix time (days-from-civil, as everywhere else here).
fn day(unix: i64) -> String {
    let z = unix.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub async fn run(data_dir: &Path) -> Result<bool, EvalError> {
    let path = data_dir.join("datasets").join("ml-25m.zip");
    let (mut engine, hybrid) = crate::search::engine(data_dir)?;
    let Some(semantic) = engine.semantic_mut() else {
        return Err(missing(
            "the vector index and model are needed (Tier 1+, or the artefact downloaded)",
        ));
    };
    let _ = hybrid;
    println!("taste — is an imported history reflected? {USERS} real MovieLens users, {FOLDS}-fold, {K} nearest\n");

    // Users and their ratings: one pass over ratings.csv.
    let file = std::fs::File::open(&path).map_err(missing)?;
    let mut zip = zip::ZipArchive::new(file).map_err(missing)?;
    let mut by_user: HashMap<i64, Vec<Rating>> = HashMap::new();
    {
        let i = (0..zip.len())
            .find(|i| {
                zip.name_for_index(*i)
                    .is_some_and(|n| n.ends_with("ratings.csv"))
            })
            .ok_or_else(|| missing("no ratings.csv"))?;
        let reader = BufReader::with_capacity(1 << 20, zip.by_index(i).map_err(missing)?);
        for line in reader.lines().skip(1).map_while(Result::ok) {
            let mut f = line.split(',');
            let (Some(u), Some(m), Some(r), Some(t)) = (f.next(), f.next(), f.next(), f.next())
            else {
                continue;
            };
            let (Ok(u), Ok(m), Ok(r), Ok(t)) =
                (u.parse::<i64>(), m.parse(), r.parse(), t.parse::<i64>())
            else {
                continue;
            };
            by_user.entry(u).or_default().push(Rating {
                movie: m,
                stars: r,
                day: day(t),
            });
        }
    }
    let mut movies_text = String::new();
    {
        let i = (0..zip.len())
            .find(|i| {
                zip.name_for_index(*i)
                    .is_some_and(|n| n.ends_with("movies.csv"))
            })
            .ok_or_else(|| missing("no movies.csv"))?;
        zip.by_index(i)
            .map_err(missing)?
            .read_to_string(&mut movies_text)
            .map_err(missing)?;
    }
    let names: HashMap<i64, (String, i64)> = sinephile_imports::csv::records(&movies_text)
        .into_iter()
        .skip(1)
        .filter_map(|r| Some((r.first()?.parse().ok()?, letterboxd_name(r.get(1)?)?)))
        .collect();
    let mut users: Vec<i64> = by_user
        .iter()
        .filter(|(_, r)| (600..=900).contains(&r.len()))
        .map(|(u, _)| *u)
        .collect();
    users.sort_unstable();
    users.truncate(USERS);

    let db = Db::open_in(data_dir).await?;
    // What the index holds, and how popular each is: the baselines' population.
    let indexed: Vec<i64> = CatalogueRepository::new(&db)
        .core_ids_for_vectors()
        .await?
        .into_iter()
        .flatten()
        .collect();
    let votes: HashMap<i64, i64> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, COALESCE(rating_votes, 0) FROM media_items WHERE rating_votes IS NOT NULL",
    )
    .fetch_all(db.pool())
    .await?
    .into_iter()
    .collect();
    let mut popular: Vec<i64> = indexed.clone();
    popular.sort_by_key(|id| std::cmp::Reverse(votes.get(id).copied().unwrap_or(0)));
    let chance = K as f64 / indexed.len().max(1) as f64;

    let (mut seed_hits, mut pop_hits, mut held_total) = (0usize, 0usize, 0usize);
    for user in &users {
        let ratings = &by_user[user];
        // Their history as a Letterboxd export: watched, ratings (their half stars), a diary.
        let dir = tempfile::tempdir().map_err(missing)?;
        let export = dir.path().join("letterboxd.zip");
        {
            let mut watched = String::from("Date,Name,Year,Letterboxd URI\n");
            let mut rated = String::from("Date,Name,Year,Letterboxd URI,Rating\n");
            for r in ratings {
                let Some((name, year)) = names.get(&r.movie) else {
                    continue;
                };
                let n = if name.contains([',', '"']) {
                    format!("\"{}\"", name.replace('"', "\"\""))
                } else {
                    name.clone()
                };
                watched.push_str(&format!("{},{n},{year},\n", r.day));
                rated.push_str(&format!("{},{n},{year},,{}\n", r.day, r.stars));
            }
            let f = std::fs::File::create(&export).map_err(missing)?;
            let mut z = zip::ZipWriter::new(f);
            for (name, text) in [("watched.csv", watched), ("ratings.csv", rated)] {
                z.start_file(name, zip::write::SimpleFileOptions::default())
                    .map_err(missing)?;
                z.write_all(text.as_bytes()).map_err(missing)?;
            }
            z.finish().map_err(missing)?;
        }
        let (_, entries) = read(&export).map_err(missing)?;
        let profiles = ProfileRepository::new(&db);
        let name = format!("eval-taste-{user}-{}", std::process::id());
        let me = profiles
            .create_with(&NewProfile {
                name: &name,
                ..Default::default()
            })
            .await?;
        let result = async {
            let summary = import(&db, me, &entries, "2026-11-07 12:00:00").await?;
            let liked = profiles.liked(me).await?;
            Ok::<_, EvalError>((summary, liked))
        }
        .await;
        profiles.delete(me).await?;
        let (summary, liked) = result?;

        // Favourites (four and five stars) that the index holds: what is held out.
        let index_set: HashSet<i64> = indexed.iter().copied().collect();
        let favourites: Vec<i64> = liked
            .iter()
            .filter(|(id, w)| *w >= 0.7 && index_set.contains(id))
            .map(|(id, _)| *id)
            .collect();
        let (mut u_seed, mut u_pop, mut u_held) = (0, 0, 0);
        for fold in 0..FOLDS {
            let held: HashSet<i64> = favourites
                .iter()
                .enumerate()
                .filter(|(i, _)| i % FOLDS == fold)
                .map(|(_, id)| *id)
                .collect();
            let train: Vec<(i64, f64)> = liked
                .iter()
                .copied()
                .filter(|(id, _)| !held.contains(id))
                .collect();
            let seen: HashSet<i64> = train.iter().map(|(id, _)| *id).collect();
            let Some(seed) = sinephile_taste::seed(semantic, &train, None) else {
                continue;
            };
            let near: HashSet<i64> = sinephile_taste::nearest(semantic, &seed, K, &seen)
                .into_iter()
                .collect();
            let pop: HashSet<i64> = popular
                .iter()
                .filter(|id| !seen.contains(id))
                .take(K)
                .copied()
                .collect();
            u_seed += held.iter().filter(|id| near.contains(id)).count();
            u_pop += held.iter().filter(|id| pop.contains(id)).count();
            u_held += held.len();
        }
        println!(
            "  user {user:>6}: {} rated, {} matched, {} liked, {} favourites held out in turn: seed {:.1}%, popularity {:.1}%",
            ratings.len(),
            summary.matched,
            liked.len(),
            u_held,
            100.0 * u_seed as f64 / u_held.max(1) as f64,
            100.0 * u_pop as f64 / u_held.max(1) as f64
        );
        seed_hits += u_seed;
        pop_hits += u_pop;
        held_total += u_held;
    }

    let seed = seed_hits as f64 / held_total.max(1) as f64;
    let pop = pop_hits as f64 / held_total.max(1) as f64;
    println!(
        "\n  held-out favourites among the {K} nearest the seed: {:.2}%",
        100.0 * seed
    );
    println!(
        "  among the {K} most popular:                         {:.2}%",
        100.0 * pop
    );
    println!(
        "  by chance ({K} of {} indexed):                     {:.3}%",
        indexed.len(),
        100.0 * chance
    );
    println!(
        "  lift over chance {:.0}x, over popularity {:.2}x",
        seed / chance.max(1e-9),
        seed / pop.max(1e-9)
    );
    let ok = seed > pop && seed > 20.0 * chance;
    println!(
        "\n  E2 (reflected in the taste model) {}",
        if ok { "MET" } else { "NOT MET" }
    );
    Ok(ok)
}

/// One MovieLens user's real history, written as a Letterboxd export and imported into
/// `profile` (for `eval switch`).
pub async fn import_user(
    data_dir: &Path,
    db: &Db,
    profile: i64,
    user: i64,
) -> Result<(), EvalError> {
    let path = data_dir.join("datasets").join("ml-25m.zip");
    let file = std::fs::File::open(&path).map_err(missing)?;
    let mut zip = zip::ZipArchive::new(file).map_err(missing)?;
    let mut ratings = Vec::new();
    {
        let i = (0..zip.len())
            .find(|i| {
                zip.name_for_index(*i)
                    .is_some_and(|n| n.ends_with("ratings.csv"))
            })
            .ok_or_else(|| missing("no ratings.csv"))?;
        let reader = BufReader::with_capacity(1 << 20, zip.by_index(i).map_err(missing)?);
        let prefix = format!("{user},");
        for line in reader.lines().skip(1).map_while(Result::ok) {
            if !line.starts_with(&prefix) {
                if !ratings.is_empty() {
                    break; // the file is grouped by user
                }
                continue;
            }
            let f: Vec<&str> = line.split(',').collect();
            if let (Some(m), Some(r), Some(t)) = (f.get(1), f.get(2), f.get(3)) {
                if let (Ok(m), Ok(r), Ok(t)) = (m.parse(), r.parse(), t.parse::<i64>()) {
                    ratings.push(Rating {
                        movie: m,
                        stars: r,
                        day: day(t),
                    });
                }
            }
        }
    }
    let mut movies_text = String::new();
    {
        let i = (0..zip.len())
            .find(|i| {
                zip.name_for_index(*i)
                    .is_some_and(|n| n.ends_with("movies.csv"))
            })
            .ok_or_else(|| missing("no movies.csv"))?;
        zip.by_index(i)
            .map_err(missing)?
            .read_to_string(&mut movies_text)
            .map_err(missing)?;
    }
    let names: HashMap<i64, (String, i64)> = sinephile_imports::csv::records(&movies_text)
        .into_iter()
        .skip(1)
        .filter_map(|r| Some((r.first()?.parse().ok()?, letterboxd_name(r.get(1)?)?)))
        .collect();
    let mut rated = String::from("Date,Name,Year,Letterboxd URI,Rating\n");
    for r in &ratings {
        let Some((name, year)) = names.get(&r.movie) else {
            continue;
        };
        let n = if name.contains([',', '"']) {
            format!("\"{}\"", name.replace('"', "\"\""))
        } else {
            name.clone()
        };
        rated.push_str(&format!("{},{n},{year},,{}\n", r.day, r.stars));
    }
    let dir = tempfile::tempdir().map_err(missing)?;
    let csv = dir.path().join("ratings.csv");
    std::fs::write(&csv, rated).map_err(missing)?;
    let (_, entries) = read(&csv).map_err(missing)?;
    import(db, profile, &entries, "2026-11-07 12:00:00").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn unix_days_read_as_dates() {
        assert_eq!(super::day(0), "1970-01-01");
        assert_eq!(super::day(1_794_090_600), "2026-11-07");
        assert_eq!(super::day(951_782_400), "2000-02-29");
    }
}
