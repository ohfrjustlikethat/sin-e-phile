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

use sinephile_persistence::repositories::MIN_SYNOPSIS;
use sinephile_persistence::Db;

use crate::error::EvalError;

/// Ranks below this are not looked at. E3 says nDCG@**10**, and a result page is ten.
const K: usize = 10;

/// E3's gate.
const TARGET: f64 = 0.75;

/// One query and everything the fixture says about it.
struct Case {
    kind: String,
    /// A second axis under `kind`, written `meaning/known-item` in the fixture.
    ///
    /// D40: two of the meaning queries describe ONE film rather than a topic, and nDCG
    /// over a two-answer graded set scores 0 whenever that film lands at rank 11. They
    /// are still meaning queries and still counted in E3 — this only lets the report say
    /// which of them they are.
    subkind: Option<String>,
    query: String,
    /// imdb id → grade. Ordered so the report is stable between runs.
    graded: BTreeMap<String, f64>,
}

pub struct Outcome {
    pub kind: String,
    pub subkind: Option<String>,
    pub query: String,
    pub ndcg: f64,
    /// The best nDCG@10 this query could possibly score, given that some of its graded
    /// answers are not in the vector index at all. 1.0 when every answer is reachable.
    ///
    /// Without this a query that is scoring as well as it *can* is indistinguishable
    /// from one that is failing, and the fixture cannot tell you which it is looking at.
    pub ceiling: f64,
    /// Graded answers `MIN_SYNOPSIS` keeps out of the vector index, with their grades.
    pub unreachable: Vec<(String, f64)>,
    /// 1/rank of the first graded answer in the top 10, else 0 — the metric a known-item
    /// query should have been scored with.
    pub reciprocal_rank: f64,
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
        let (kind, subkind) = match kind.split_once('/') {
            Some((k, sub)) => (k, Some(sub.to_string())),
            None => (kind, None),
        };
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
                    subkind,
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

/// Grades sorted best-first — the order a perfect ranking would return them in.
fn ideal_order(grades: &[f64]) -> Vec<f64> {
    let mut sorted = grades.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(K);
    sorted
}

/// Is this IMDb id in the vector index at all?
///
/// Mirrors `CatalogueRepository::core_ids_for_vectors`: core tier, not an episode, and
/// enough synopsis to be worth embedding. Written out rather than reusing that method
/// because this asks about ONE id and that returns eight hundred thousand.
async fn vector_reachable(db: &Db, imdb: &str) -> Result<bool, EvalError> {
    let found: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1 FROM external_ids e JOIN media_items m ON m.id = e.media_item_id
          WHERE e.source = 'imdb' AND e.external_id = ?
            AND m.in_core = 1 AND m.kind <> 'episode'
            AND m.synopsis IS NOT NULL
            AND length(trim(m.synopsis)) >= {MIN_SYNOPSIS}"
    ))
    .bind(imdb)
    .fetch_optional(db.pool())
    .await?;
    Ok(found.is_some())
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

        // What this query could score at best. A graded answer whose synopsis is under
        // MIN_SYNOPSIS is not in the vector index (see `core_ids_for_vectors`), so the
        // meaning half CANNOT return it however good it gets — and a fixture that cannot
        // separate "failing" from "already at its ceiling" is measuring the wrong thing.
        let mut reachable: Vec<f64> = Vec::new();
        let mut unreachable: Vec<(String, f64)> = Vec::new();
        for (imdb, grade) in &case.graded {
            if vector_reachable(db, imdb).await? {
                reachable.push(*grade);
            } else {
                unreachable.push((imdb.clone(), *grade));
            }
        }
        let ceiling = ndcg(&ideal_order(&reachable), &available);

        // The known-item metric. Rank of the first graded answer, reciprocated.
        let reciprocal_rank = found
            .iter()
            .position(|g| *g > 0.0)
            .map(|i| 1.0 / (i as f64 + 1.0))
            .unwrap_or(0.0);

        outcomes.push(Outcome {
            kind: case.kind,
            subkind: case.subkind,
            query: case.query,
            ndcg: ndcg(&found, &available),
            ceiling,
            unreachable,
            reciprocal_rank,
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

    // ── diagnostics ──────────────────────────────────────────────────────────────
    //
    // These do NOT move E3. The headline above is the criterion exactly as SPEC.md
    // Phase 5 words it, over every meaning query, and it stays that way — §10.11 forbids
    // redefining an exit criterion, and a criterion rewritten after seeing the number it
    // produced is not a criterion. What follows says what the number is MADE OF, which
    // is a different thing and the only honest way to act on it.
    let meaning: Vec<&Outcome> = outcomes.iter().filter(|o| o.kind == "meaning").collect();
    let known: Vec<&&Outcome> = meaning
        .iter()
        .filter(|o| o.subkind.as_deref() == Some("known-item"))
        .collect();
    let topical: Vec<&&Outcome> = meaning
        .iter()
        .filter(|o| o.subkind.as_deref() != Some("known-item"))
        .collect();

    if !known.is_empty() {
        println!();
        println!("  diagnostics — what that number is made of, not a restatement of it");

        let topical_mean =
            topical.iter().map(|o| o.ndcg).sum::<f64>() / topical.len().max(1) as f64;
        println!(
            "     topical      nDCG@{K} {topical_mean:.4}  over {} queries",
            topical.len()
        );

        // D40: a known-item query names ONE film. nDCG over a two-answer graded set
        // scores 0 the moment it lands at rank 11, which says nothing about how close it
        // came. Reciprocal rank says that, so it is reported alongside — not instead.
        let mrr = known.iter().map(|o| o.reciprocal_rank).sum::<f64>() / known.len() as f64;
        let present = known.iter().filter(|o| o.reciprocal_rank > 0.0).count();
        println!(
            "     known-item   MRR {mrr:.4}, found in the top {K}: {present}/{}",
            known.len()
        );
        for outcome in &known {
            println!("                    {:?}", outcome.query);
        }
    }

    // A query whose answers are not in the index cannot reach the target however good
    // the engine is, and until this printed, nothing distinguished that from failure.
    let capped: Vec<&Outcome> = outcomes
        .iter()
        .filter(|o| o.kind == "meaning" && !o.unreachable.is_empty())
        .collect();
    if !capped.is_empty() {
        println!();
        println!("  UNREACHABLE ANSWERS — graded films the vector half cannot return at all");
        println!("     (synopsis under MIN_SYNOPSIS = {MIN_SYNOPSIS}, so not in the index)");
        for outcome in &capped {
            println!(
                "     ceiling {:.4}{}  {:?}",
                outcome.ceiling,
                if outcome.ceiling < TARGET {
                    " — BELOW THE TARGET"
                } else {
                    ""
                },
                outcome.query
            );
            for (imdb, grade) in &outcome.unreachable {
                println!("                        {imdb}  graded {grade}");
            }
        }
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
