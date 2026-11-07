//! `eval identify`: Phase 12 E1 and E2 over `fixtures/filenames/`, against the real
//! catalogue.
//!
//! - **Top-1 (E1, > 95%)**: of the names whose truth is known, how often the best
//!   candidate is it.
//! - **False-confident (E2, < 1%)**: of the names the library would accept without
//!   asking, how many are wrong. A name labelled `?` (nobody could honestly identify it
//!   from what is given) that is accepted counts as wrong. This is the strict reading of
//!   E2; the share of ALL names is printed beside it.
//!
//! The threshold is swept to show the trade, and both sets are judged at the library's
//! own `ACCEPT_AT`.

use std::path::Path;

use sinephile_library::{identify, Evidence, Identification, ACCEPT_AT};
use sinephile_persistence::repositories::SearchRepository;
use sinephile_persistence::Db;

use crate::error::EvalError;

struct Row {
    name: String,
    folders: Vec<String>,
    minutes: Option<f64>,
    /// `None` for a `?` row.
    truth: Option<(String, String, i64)>,
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
        if f.len() != 15 {
            return Err(EvalError::Fixture(file.display().to_string(), line.into()));
        }
        let episode = !f[3].is_empty() || !f[4].is_empty() || !f[5].is_empty();
        let truth = match f[10] {
            "?" => None,
            "" => {
                let year = f[2].parse().ok();
                year.map(|y| {
                    let kind = if episode { "series" } else { "movie" };
                    (kind.to_string(), f[1].to_string(), y)
                })
            }
            kind => f[12]
                .parse()
                .ok()
                .map(|y| (kind.to_string(), f[11].to_string(), y)),
        };
        out.push(Row {
            name: f[0].to_string(),
            folders: f[13]
                .split('/')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            minutes: f[14].parse().ok(),
            truth,
        });
    }
    Ok(out)
}

/// The catalogue item a label names: its title in any variant, the year, the kind.
async fn resolve(db: &Db, kind: &str, title: &str, year: i64) -> Result<Option<i64>, EvalError> {
    let kinds: &[&str] = if kind == "series" {
        &["series", "anime_series"]
    } else {
        &["film", "anime_film"]
    };
    let hits = SearchRepository::new(db).exact_title(title, 200).await?;
    Ok(hits
        .into_iter()
        .find(|h| kinds.contains(&h.kind.as_str()) && h.year == Some(year))
        .map(|h| h.media_item_id))
}

struct Outcome {
    name: String,
    truth: Option<i64>,
    labelled: bool,
    id: Identification,
}

async fn measure(db: &Db, file: &Path) -> Result<(Vec<Outcome>, usize), EvalError> {
    let mut out = Vec::new();
    let mut unresolved = 0;
    for r in rows(file)? {
        let truth = match &r.truth {
            Some((k, t, y)) => {
                let id = resolve(db, k, t, *y).await?;
                if id.is_none() {
                    unresolved += 1;
                    println!(
                        "    label not in the catalogue: {k} {t:?} {y}   ({})",
                        r.name
                    );
                    continue;
                }
                id
            }
            None => None,
        };
        let id = identify(
            db,
            &Evidence {
                name: r.name.clone(),
                folders: r.folders.clone(),
                duration_minutes: r.minutes,
            },
        )
        .await?;
        out.push(Outcome {
            name: r.name,
            truth,
            labelled: r.truth.is_some(),
            id,
        });
    }
    Ok((out, unresolved))
}

struct AtThreshold {
    accepted: usize,
    wrong_accepted: usize,
}

fn at(outcomes: &[Outcome], threshold: f64) -> AtThreshold {
    let mut accepted = 0;
    let mut wrong = 0;
    for o in outcomes {
        if o.id.confidence >= threshold && o.id.best().is_some() {
            accepted += 1;
            let best = o.id.best().map(|b| b.media_item_id);
            if !o.labelled || best != o.truth {
                wrong += 1;
            }
        }
    }
    AtThreshold {
        accepted,
        wrong_accepted: wrong,
    }
}

fn top1(outcomes: &[Outcome]) -> (usize, usize) {
    let labelled: Vec<&Outcome> = outcomes.iter().filter(|o| o.labelled).collect();
    let right = labelled
        .iter()
        .filter(|o| o.id.best().map(|b| b.media_item_id) == o.truth)
        .count();
    (right, labelled.len())
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

/// Where identification's time goes: each catalogue query, timed over the first names.
pub async fn timing(data_dir: &Path) -> Result<bool, EvalError> {
    use sinephile_persistence::repositories::LibraryRepository;
    let db = Db::open_in(data_dir).await?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/filenames");
    let search = SearchRepository::new(&db);
    let library = LibraryRepository::new(&db);
    let mut spent = [0f64; 4];
    let names: Vec<Row> = rows(&root.join("names.psv"))?
        .into_iter()
        .take(80)
        .collect();
    for r in &names {
        let title = filename_parser::parse(&r.name).title;
        let t = std::time::Instant::now();
        let a = search.exact_title(&title, 20).await?;
        spent[0] += t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let b = search.keyword(&title, 15).await?;
        spent[1] += t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let c = search.fuzzy(&title, 8).await?;
        spent[2] += t.elapsed().as_secs_f64();
        let ids: Vec<i64> = a
            .iter()
            .chain(&b)
            .chain(&c)
            .map(|h| h.media_item_id)
            .collect();
        let t = std::time::Instant::now();
        library.candidate_facts(&ids).await?;
        spent[3] += t.elapsed().as_secs_f64();
    }
    let n = names.len() as f64;
    for (what, s) in ["exact_title", "keyword", "fuzzy", "candidate_facts"]
        .iter()
        .zip(spent)
    {
        println!("  {what:<16} {:>8.1} ms per name", 1000.0 * s / n);
    }
    Ok(true)
}

pub async fn run(data_dir: &Path, report: bool) -> Result<bool, EvalError> {
    let db = Db::open_in(data_dir).await?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/filenames");
    println!("identify — what each name in fixtures/filenames is, against the catalogue\n");

    let started = std::time::Instant::now();
    let (tuned, unresolved_t) = measure(&db, &root.join("names.psv")).await?;
    let (held, unresolved_h) = measure(&db, &root.join("heldout.psv")).await?;
    let per_file =
        started.elapsed().as_secs_f64() * 1000.0 / (tuned.len() + held.len()).max(1) as f64;

    // The sweep shows the trade; the library's own threshold is what is judged.
    let chosen = ACCEPT_AT;
    let mut lowest = 1.0;
    println!("  threshold  accepted   wrong among accepted   (tuning set)");
    for step in 0..=19 {
        let t = 0.30 + 0.035 * step as f64;
        let a = at(&tuned, t);
        let rate = pct(a.wrong_accepted, a.accepted);
        println!(
            "    {t:.3}    {:>5.1}%    {:>3} = {:>5.2}%",
            pct(a.accepted, tuned.len()),
            a.wrong_accepted,
            rate
        );
        if rate < 1.0 && t < lowest {
            lowest = t;
        }
    }

    println!("  (the lowest under 1% is {lowest:.3}; the library accepts at {chosen:.3})");
    let mut met = true;
    for (label, outcomes, unresolved) in [
        ("tuning set", &tuned, unresolved_t),
        ("held out  ", &held, unresolved_h),
    ] {
        let (right, n) = top1(outcomes);
        let a = at(outcomes, chosen);
        let fc = pct(a.wrong_accepted, a.accepted);
        println!(
            "\n  {label}: {} names ({unresolved} labels not in the catalogue, left out)\
             \n    E1 top-1          {right}/{n} = {:.2}%   (target > 95%)\
             \n    E2 false-confident at {chosen:.3}: {}/{} accepted = {fc:.2}%   (target < 1%); {:.2}% of all names\
             \n    asked (review queue) {:.1}%",
            outcomes.len(),
            pct(right, n),
            a.wrong_accepted,
            a.accepted,
            pct(a.wrong_accepted, outcomes.len()),
            100.0 - pct(a.accepted, outcomes.len()),
        );
        met &= pct(right, n) > 95.0 && fc < 1.0;
    }
    println!("\n  {per_file:.1} ms per name (identification only, catalogue on disk)");

    if report {
        println!("\n  misses (best is not the truth), and wrong acceptances:");
        for o in tuned.iter().chain(held.iter()) {
            let best = o.id.best();
            let wrong = best.map(|b| b.media_item_id) != o.truth;
            if (o.labelled && wrong) || (!o.labelled && o.id.confidence >= chosen) {
                println!(
                    "    {:<70} conf {:.2}  best {:?}",
                    o.name,
                    o.id.confidence,
                    best.map(|b| (&b.title, b.year, &b.kind, (b.score * 100.0).round() / 100.0))
                );
                for c in o.id.candidates.iter().take(3) {
                    println!(
                        "        {:<44} {:?} {:<12} {:.3}  t{:.2} y{:.2} k{:.2} r{:.2} p{:.2} e{:?}",
                        c.title,
                        c.year,
                        c.kind,
                        c.score,
                        c.signals.title,
                        c.signals.year,
                        c.signals.kind,
                        c.signals.runtime,
                        c.signals.popularity,
                        c.signals.episode_found
                    );
                }
            }
        }
    }
    if report {
        println!("\n  right but asked (confidence below {chosen:.3}):");
        for o in tuned.iter().chain(held.iter()) {
            let best = o.id.best();
            if o.labelled && best.map(|b| b.media_item_id) == o.truth && o.id.confidence < chosen {
                let next = o.id.candidates.get(1);
                println!(
                    "    {:<66} conf {:.2}  {:.3} vs {:?}",
                    o.name,
                    o.id.confidence,
                    best.map_or(0.0, |b| b.score),
                    next.map(|n| (
                        &n.title,
                        n.year,
                        &n.kind,
                        (n.score * 1000.0).round() / 1000.0
                    ))
                );
            }
        }
    }
    if !met {
        println!("\n  NOT MET.");
    }
    Ok(met)
}
