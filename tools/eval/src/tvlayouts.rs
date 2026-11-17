//! `eval tvlayouts`: Phase 16 E4 over `fixtures/tv-layouts/layouts.psv`, against the real
//! catalogue. Each file goes through what the scanner decides (an extra is never searched
//! for) and then identification. Judged:
//!
//! - **Episodes right** (at least 98%): the best candidate is the series, and the reading
//!   has its season and episode.
//! - **No episode filed as a film**, and **no extra searched for** as anything.
//! - Films and the split show (one series from several folders) are counted in "right".
//!
//! `--before` skips the extras rule: the scanner as it was until Phase 16.

use std::path::Path;

use sinephile_library::folders::is_extra;
use sinephile_library::identify::read;
use sinephile_library::{identify, Evidence};
use sinephile_persistence::repositories::SearchRepository;
use sinephile_persistence::Db;

use crate::error::EvalError;

struct Row {
    folders: Vec<String>,
    name: String,
    kind: String,
    title: String,
    year: Option<i64>,
    season: Option<u32>,
    episode: Option<u32>,
}

fn rows(file: &Path) -> Result<Vec<Row>, EvalError> {
    let text = std::fs::read_to_string(file).map_err(|e| EvalError::Io {
        path: file.display().to_string(),
        source: e,
    })?;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('|').map(str::trim).collect();
        if f.len() != 6 {
            return Err(EvalError::Fixture(file.display().to_string(), line.into()));
        }
        let mut parts: Vec<String> = f[0].split('/').map(str::to_string).collect();
        let name = parts.pop().unwrap_or_default();
        out.push(Row {
            folders: parts,
            name,
            kind: f[1].to_string(),
            title: f[2].to_string(),
            year: f[3].parse().ok(),
            season: f[4].parse().ok(),
            episode: f[5].parse().ok(),
        });
    }
    Ok(out)
}

async fn resolve(db: &Db, kind: &str, title: &str, year: i64) -> Result<Option<i64>, EvalError> {
    let kinds: &[&str] = if kind == "episode" {
        &["series", "anime_series"]
    } else {
        &["film", "anime_film"]
    };
    Ok(SearchRepository::new(db)
        .exact_title(title, 200)
        .await?
        .into_iter()
        .find(|h| kinds.contains(&h.kind.as_str()) && h.year == Some(year))
        .map(|h| h.media_item_id))
}

pub async fn run(data_dir: &Path, before: bool, report: bool) -> Result<bool, EvalError> {
    let db = Db::open_in(data_dir).await?;
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tv-layouts/layouts.psv");
    println!(
        "tvlayouts — what the scanner makes of each file in fixtures/tv-layouts{}\n",
        if before {
            " (BEFORE: no extras rule)"
        } else {
            ""
        }
    );
    let (mut episodes, mut episodes_right, mut as_film) = (0, 0, 0);
    let (mut films, mut films_right) = (0, 0);
    let (mut extras, mut extras_right, mut searched) = (0, 0, 0);
    let mut misses = Vec::new();
    for r in rows(&file)? {
        let extra = !before && is_extra(&r.folders, &r.name);
        if r.kind == "extra" {
            extras += 1;
            if extra {
                extras_right += 1;
            } else {
                searched += 1;
                misses.push(format!("extra searched for: {}", r.name));
            }
            continue;
        }
        if extra {
            misses.push(format!("{} taken for an extra: {}", r.kind, r.name));
            if r.kind == "episode" {
                episodes += 1;
            } else {
                films += 1;
            }
            continue;
        }
        let evidence = Evidence {
            name: r.name.clone(),
            folders: r.folders.clone(),
            duration_minutes: None,
        };
        let truth = match r.year {
            Some(y) => resolve(&db, &r.kind, &r.title, y).await?,
            None => None,
        };
        let id = identify(&db, &evidence).await?;
        let best = id.best();
        let reading = read(&evidence);
        if r.kind == "episode" {
            episodes += 1;
            let series = best.is_some_and(|b| Some(b.media_item_id) == truth);
            // Where the catalogue has the episode, identification must have placed the file
            // on it (anime's "- 05" resolves through AniList's numbering, ADR-0031); where
            // it does not (Twin Peaks' 2017 season, before a refresh), the name and folders
            // must still read as that season and episode.
            let row: Option<i64> = match (truth, r.season, r.episode) {
                (Some(t), Some(s), Some(e)) => {
                    sqlx::query_scalar(
                        "SELECT media_item_id FROM episodes
                     WHERE series_id = ? AND season_number = ? AND episode_number = ?",
                    )
                    .bind(t)
                    .bind(i64::from(s))
                    .bind(i64::from(e))
                    .fetch_optional(db.pool())
                    .await?
                }
                _ => None,
            };
            let numbers = match row {
                Some(want) => id.episode.as_ref().map(|h| h.episode_id) == Some(want),
                None => {
                    reading.season == r.season && reading.episodes.first().copied() == r.episode
                }
            };
            if best.is_some_and(|b| b.kind == "film" || b.kind == "anime_film") {
                as_film += 1;
            }
            if series && numbers {
                episodes_right += 1;
            } else {
                misses.push(format!(
                    "episode: {} -> {:?} S{:?}E{:?} (want {} S{:?}E{:?})",
                    r.name,
                    best.map(|b| (&b.title, b.year, &b.kind)),
                    reading.season,
                    reading.episodes.first(),
                    r.title,
                    r.season,
                    r.episode
                ));
            }
        } else {
            films += 1;
            if best.is_some_and(|b| Some(b.media_item_id) == truth) {
                films_right += 1;
            } else {
                misses.push(format!(
                    "film: {} -> {:?}",
                    r.name,
                    best.map(|b| (&b.title, b.year, &b.kind))
                ));
            }
        }
    }
    let pct = |a: usize, b: usize| {
        if b == 0 {
            0.0
        } else {
            100.0 * a as f64 / b as f64
        }
    };
    println!(
        "  episodes  {episodes_right}/{episodes} right = {:.1}%   (target at least 98%)",
        pct(episodes_right, episodes)
    );
    println!("  episodes filed as films: {as_film}   (target 0)");
    println!("  films     {films_right}/{films} right");
    println!(
        "  extras    {extras_right}/{extras} kept out of identification; {searched} searched for as titles   (target 0)"
    );
    if report || !misses.is_empty() {
        println!("\n  misses ({}):", misses.len());
        for m in misses.iter().take(if report { usize::MAX } else { 25 }) {
            println!("    {m}");
        }
    }
    let ok = pct(episodes_right, episodes) >= 98.0
        && as_film == 0
        && searched == 0
        && films_right == films;
    println!("\n  E4 {}", if ok { "MET" } else { "NOT MET" });
    Ok(ok)
}

/// `--folder <path>`: walk a real folder as the scanner does and identify every file it
/// keeps, writing nothing: what a library scan of it would decide. Reads names only.
pub async fn folder(data_dir: &Path, root: &Path) -> Result<bool, EvalError> {
    let db = Db::open_in(data_dir).await?;
    let quiet = |_| {};
    let kept = sinephile_library::scan::walk(root, &quiet);
    let all = walkdir_count(root);
    println!(
        "folder — {}: {} videos, {} kept by the scanner, {} left out as extras\n",
        root.display(),
        all,
        kept.len(),
        all - kept.len()
    );
    let (mut series, mut films, mut asked) = (
        std::collections::BTreeMap::<String, usize>::new(),
        Vec::new(),
        0,
    );
    for f in &kept {
        let evidence = Evidence {
            name: f
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            folders: sinephile_library::scan::folders_between(root, &f.path),
            duration_minutes: None,
        };
        let id = identify(&db, &evidence).await?;
        if id.confidence < sinephile_library::ACCEPT_AT {
            asked += 1;
        }
        match id.best() {
            Some(b) if b.kind.ends_with("series") => {
                *series
                    .entry(format!(
                        "{} ({:?}){}",
                        b.title,
                        b.year,
                        if id.episode.is_some() {
                            ""
                        } else {
                            " [episode not in catalogue]"
                        }
                    ))
                    .or_default() += 1
            }
            Some(b) => films.push(format!(
                "{} ({:?}) {} <- {}",
                b.title, b.year, b.kind, evidence.name
            )),
            None => films.push(format!("nothing <- {}", evidence.name)),
        }
    }
    for (s, n) in &series {
        println!("  series  {s}: {n} episodes");
    }
    for f in &films {
        println!("  film    {f}");
    }
    println!("\n  {asked} would go to the review queue");
    Ok(true)
}

fn walkdir_count(root: &Path) -> usize {
    fn go(p: &Path) -> usize {
        std::fs::read_dir(p)
            .map(|it| {
                it.filter_map(Result::ok)
                    .map(|e| {
                        let p = e.path();
                        if p.is_dir() {
                            go(&p)
                        } else {
                            usize::from(sinephile_library::scan::is_video(&p))
                        }
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    go(root)
}
