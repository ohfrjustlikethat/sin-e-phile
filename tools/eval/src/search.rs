//! The search relevance harness — exit criteria E1, E2 and E3.
//!
//! # Why the exact-title half is a separate number
//!
//! E2 asks for **100%** exact-title top-1 and E3 asks for nDCG@10 > 0.75. Averaging
//! them into one relevance score would let a good semantic result hide a wrong exact
//! one, and a wrong exact one is the failure a search engine is not allowed to have.
//! They are measured apart and reported apart.

use std::path::Path;
use std::time::Instant;

use sinephile_persistence::repositories::{MatchReason, SearchRepository};
use sinephile_persistence::Db;

use crate::error::EvalError;

struct Case {
    query: String,
    expect_imdb: String,
    why: String,
}

fn load(path: &Path) -> Result<Vec<Case>, EvalError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| EvalError::Fixture(path.display().to_string(), e.to_string()))?;
    let mut cases = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') || line.starts_with("query\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            return Err(EvalError::Fixture(
                path.display().to_string(),
                format!("expected 3 columns, got {}: {line}", fields.len()),
            ));
        }
        cases.push(Case {
            query: fields[0].to_string(),
            expect_imdb: fields[1].to_string(),
            why: fields[2].to_string(),
        });
    }
    Ok(cases)
}

/// What a single case did.
struct Outcome {
    case: Case,
    got_imdb: Option<String>,
    got_title: Option<String>,
    reason: Option<MatchReason>,
    millis: f64,
    passed: bool,
}

pub async fn run(data_dir: &Path, report: bool) -> Result<bool, EvalError> {
    let fixture = Path::new("fixtures/search/exact-titles.tsv");
    let cases = load(fixture)?;
    let db = Db::open_in(data_dir).await?;
    let search = SearchRepository::new(&db);

    let mut outcomes = Vec::with_capacity(cases.len());
    for case in cases {
        let started = Instant::now();
        let hits = search.search(&case.query, 5).await?;
        let millis = started.elapsed().as_secs_f64() * 1000.0;

        // Top-1 only. E2 is about what comes FIRST; a correct answer in position three
        // is a different and much weaker property.
        let (got_imdb, got_title, reason) = match hits.first() {
            Some(hit) => {
                let imdb: Option<String> = sqlx::query_scalar(
                    "SELECT external_id FROM external_ids
                      WHERE media_item_id = ? AND source = 'imdb'",
                )
                .bind(hit.media_item_id)
                .fetch_optional(db.pool())
                .await?;
                (imdb, Some(hit.title.clone()), Some(hit.why))
            }
            None => (None, None, None),
        };

        let passed = got_imdb.as_deref() == Some(case.expect_imdb.as_str());
        outcomes.push(Outcome {
            case,
            got_imdb,
            got_title,
            reason,
            millis,
            passed,
        });
    }

    let total = outcomes.len();
    let passed = outcomes.iter().filter(|o| o.passed).count();
    let mut times: Vec<f64> = outcomes.iter().map(|o| o.millis).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p = |q: f64| times[((times.len().saturating_sub(1)) as f64 * q) as usize];

    println!();
    println!("  E2 — exact-title top-1");
    println!(
        "     {passed}/{total} = {:.1}%   (target 100%)",
        passed as f64 * 100.0 / total as f64
    );
    println!();
    println!("  E1 — latency over the fixture (keyword path only; no embedding yet)");
    println!(
        "     p50 {:.1} ms   p95 {:.1} ms   max {:.1} ms   (target p95 < 80 ms)",
        p(0.50),
        p(0.95),
        times.last().copied().unwrap_or(0.0)
    );

    if report {
        let failures: Vec<&Outcome> = outcomes.iter().filter(|o| !o.passed).collect();
        if !failures.is_empty() {
            println!();
            println!("  failures:");
            for o in failures {
                println!(
                    "    {:<38} expected {} but got {} {:?}",
                    o.case.query,
                    o.case.expect_imdb,
                    o.got_imdb.as_deref().unwrap_or("NOTHING"),
                    o.got_title.as_deref().unwrap_or("")
                );
                println!("      {} · {:?} · {:.1} ms", o.case.why, o.reason, o.millis);
            }
        }
        // The slowest cases, because a p95 hides which query is expensive and that is
        // the thing worth fixing.
        let mut slowest: Vec<&Outcome> = outcomes.iter().collect();
        slowest.sort_by(|a, b| {
            b.millis
                .partial_cmp(&a.millis)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        println!();
        println!("  slowest queries:");
        for o in slowest.iter().take(5) {
            println!(
                "    {:>8.1} ms  {:<34} {:?}",
                o.millis, o.case.query, o.reason
            );
        }
    }
    println!();

    let e2_met = passed == total;
    let e1_met = p(0.95) < 80.0;
    if !e2_met {
        println!(
            "  E2 NOT MET: {} of {total} queries return the wrong film first.",
            total - passed
        );
    }
    if !e1_met {
        println!(
            "  E1 NOT MET on this path: p95 {:.1} ms exceeds 80 ms.",
            p(0.95)
        );
    }
    Ok(e2_met && e1_met)
}
