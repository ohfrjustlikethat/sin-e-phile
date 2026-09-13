//! nDCG@10 over the semantic query set — exit criterion E3.
//!
//! # What nDCG measures, and why it is the right shape here
//!
//! **DCG** — discounted cumulative gain — adds up the relevance of what came back,
//! discounting each position by `log2(rank + 1)`, so a correct answer at rank 1 is worth
//! more than the same answer at rank 9. **nDCG** divides that by the best score the
//! fixture's own grades could possibly achieve, which puts every query on 0..1 no matter
//! how many known answers it has.
//!
//! That last part is what makes it usable here: "documentaries about mountain climbing"
//! has four graded answers and "post-war Italian neorealism" has three, and without the
//! normalisation the query with more answers would dominate the average for no reason.
//!
//! # Graded, not binary
//!
//! The fixture grades 2 for an obvious answer and 1 for a defensible one, because
//! relevance here genuinely has degrees: *Chungking Express* is not what "like Wong
//! Kar-wai but Korean" asked for, and it is not wrong to offer it either. A binary
//! judgement would have to call that either perfect or worthless, and both are lies.
//!
//! # The two kinds are never averaged
//!
//! A filter query fails when a WHERE clause is wrong; a meaning query fails when an
//! embedding is weak. Different fixes, different ceilings, and one number covering both
//! would move for reasons nobody could attribute.

use std::collections::BTreeMap;
use std::path::Path;

use sinephile_persistence::Db;

use crate::error::EvalError;

/// Ranks below this are not looked at. E3 says nDCG@**10**, and a result page is ten.
const K: usize = 10;

/// E3's gate.
const TARGET: f64 = 0.75;

/// One query and everything the fixture says about it.
struct Case {
    kind: String,
    query: String,
    /// imdb id → grade. Ordered so the report is stable between runs.
    graded: BTreeMap<String, f64>,
}

pub struct Outcome {
    pub kind: String,
    pub query: String,
    pub ndcg: f64,
    /// What actually came back, for the report — a number with no examples under it is
    /// not something anyone can act on.
    pub top: Vec<(String, Option<String>, f64)>,
}

fn load(path: &Path) -> Result<Vec<Case>, EvalError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| EvalError::Fixture(path.display().to_string(), e.to_string()))?;

    // Keyed by (kind, query) so a query's graded answers gather no matter how the file
    // is ordered, and so the same query text under both kinds stays two cases.
    let mut cases: Vec<Case> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') || line.starts_with("kind\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            return Err(EvalError::Fixture(
                path.display().to_string(),
                format!("expected at least 4 columns, got {}: {line}", fields.len()),
            ));
        }
        let (kind, query, imdb) = (fields[0], fields[1], fields[2]);
        let grade: f64 = fields[3].parse().map_err(|_| {
            EvalError::Fixture(
                path.display().to_string(),
                format!("grade {:?} is not a number: {line}", fields[3]),
            )
        })?;

        match cases
            .iter_mut()
            .find(|c| c.kind == kind && c.query == query)
        {
            Some(case) => {
                case.graded.insert(imdb.to_string(), grade);
            }
            None => {
                let mut graded = BTreeMap::new();
                graded.insert(imdb.to_string(), grade);
                cases.push(Case {
                    kind: kind.to_string(),
                    query: query.to_string(),
                    graded,
                });
            }
        }
    }
    Ok(cases)
}

/// `Σ gain / log2(rank + 1)`, ranks counted from 1.
///
/// The plain gain rather than `2^gain - 1`: with grades of only 1 and 2 the exponential
/// form triples the distance between "defensible" and "obvious", which asserts more
/// about the difference between them than the fixture's author can honestly claim.
fn dcg(grades: &[f64]) -> f64 {
    grades
        .iter()
        .enumerate()
        .map(|(i, gain)| gain / ((i as f64 + 2.0).log2()))
        .sum()
}

fn ndcg(found: &[f64], available: &[f64]) -> f64 {
    // The best any ranking could do with these grades: all of them, best first.
    let mut ideal: Vec<f64> = available.to_vec();
    ideal.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    ideal.truncate(K);

    let best = dcg(&ideal);
    if best == 0.0 {
        // A query whose fixture grades everything 0 cannot be scored. Returning 1.0
        // would quietly inflate the average with a query that asserts nothing.
        return 0.0;
    }
    dcg(found) / best
}

pub async fn run(
    data_dir: &Path,
    engine: &mut sinephile_search_engine::Engine,
    db: &Db,
) -> Result<Vec<Outcome>, EvalError> {
    let _ = data_dir;
    let fixture = Path::new("fixtures/search/semantic-queries.tsv");
    let cases = load(fixture)?;

    let mut outcomes = Vec::with_capacity(cases.len());
    for case in cases {
        let hits = engine.search(db, &case.query, K as i64).await?;

        // The engine returns catalogue ids; the fixture speaks IMDb. One lookup per
        // hit, which is ten queries against an indexed column — this is a harness, and
        // clarity is worth more than the microseconds.
        let mut found = Vec::with_capacity(hits.len());
        let mut top = Vec::with_capacity(hits.len());
        for hit in &hits {
            let imdb: Option<String> = sqlx::query_scalar(
                "SELECT external_id FROM external_ids
                  WHERE media_item_id = ? AND source = 'imdb'",
            )
            .bind(hit.media_item_id)
            .fetch_optional(db.pool())
            .await?;

            let grade = imdb
                .as_ref()
                .and_then(|id| case.graded.get(id))
                .copied()
                .unwrap_or(0.0);
            found.push(grade);
            top.push((hit.title.clone(), imdb, grade));
        }

        let available: Vec<f64> = case.graded.values().copied().collect();
        outcomes.push(Outcome {
            kind: case.kind,
            query: case.query,
            ndcg: ndcg(&found, &available),
            top,
        });
    }
    Ok(outcomes)
}

/// Print the report and say whether E3's gate is cleared.
pub fn report(outcomes: &[Outcome], verbose: bool) -> bool {
    let mean = |kind: &str| {
        let of_kind: Vec<f64> = outcomes
            .iter()
            .filter(|o| o.kind == kind)
            .map(|o| o.ndcg)
            .collect();
        if of_kind.is_empty() {
            return None;
        }
        Some(of_kind.iter().sum::<f64>() / of_kind.len() as f64)
    };

    println!();
    println!("  E3 — nDCG@{K} over the semantic query set");
    for kind in ["meaning", "filter"] {
        let Some(score) = mean(kind) else { continue };
        let count = outcomes.iter().filter(|o| o.kind == kind).count();
        println!("     {kind:<8} {score:.4}  over {count} queries   (target {TARGET:.2})");
    }

    if verbose {
        for outcome in outcomes {
            println!();
            println!(
                "  [{}] {:?}  nDCG {:.4}",
                outcome.kind, outcome.query, outcome.ndcg
            );
            for (rank, (title, imdb, grade)) in outcome.top.iter().enumerate() {
                let mark = match *grade {
                    g if g >= 2.0 => "**",
                    g if g >= 1.0 => " *",
                    _ => "  ",
                };
                println!(
                    "    {mark} {:>2}. {:<48} {}",
                    rank + 1,
                    title.chars().take(48).collect::<String>(),
                    imdb.as_deref().unwrap_or("")
                );
            }
        }
    }

    // The MEANING half is what E3 is about. A filter query scoring well says the WHERE
    // clause works, which is worth knowing and is not what the criterion asks.
    mean("meaning").is_some_and(|score| score >= TARGET)
}
