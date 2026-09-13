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

use sinephile_persistence::repositories::MatchReason;
use sinephile_persistence::Db;
use sinephile_search_engine::{Engine, Semantic};
use sinephile_vector_index::{index_path, VectorIndex};

use crate::error::EvalError;

/// Build the engine the application would build: hybrid when the model and the index
/// are both present, keyword-only otherwise.
///
/// Silence would be the bug here — a missing model would quietly turn every future E1
/// number into a keyword-only measurement wearing a hybrid label — so the caller prints
/// which one it got.
fn engine(data_dir: &Path) -> Result<(Engine, bool), EvalError> {
    let models = Path::new("models");
    let model = models.join("bge-small-en-v1.5-int8.onnx");
    let tokenizer = models.join("bge-small-en-v1.5-tokenizer.json");
    let index = index_path(data_dir);

    if !model.is_file() || !tokenizer.is_file() || !index.exists() {
        return Ok((Engine::keyword_only(), false));
    }

    let embedder = sinephile_embedder::Embedder::pinned(&model, &tokenizer)
        .map_err(|e| EvalError::Missing(e.to_string()))?;
    let index = VectorIndex::view(&index)?;
    Ok((Engine::hybrid(Semantic { index, embedder }), true))
}

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

/// Run one query and print what comes back, with the reason for each hit.
///
/// Not a measurement — a way to LOOK at the results, which is the only way to judge
/// whether fusion is working at all. E2's fixture is exact titles by construction, so it
/// exercises the short-circuit and says nothing about whether the semantic half returns
/// anything defensible. E4 asks exactly that question about two named queries.
pub async fn one(data_dir: &Path, query: &str) -> Result<bool, EvalError> {
    let db = Db::open_in(data_dir).await?;
    let (mut engine, hybrid) = engine(data_dir)?;

    let started = Instant::now();
    let hits = engine.search(&db, query, 10).await?;
    let millis = started.elapsed().as_secs_f64() * 1000.0;

    println!();
    println!(
        "  {query:?}   {millis:.1} ms   {}",
        if hybrid { "hybrid" } else { "keyword only" }
    );
    println!();
    for (rank, hit) in hits.iter().enumerate() {
        println!(
            "  {:>2}. {:<52} {:<6} {:?}",
            rank + 1,
            format!(
                "{} ({})",
                hit.title,
                hit.year
                    .map(|y| y.to_string())
                    .unwrap_or_else(|| "—".into())
            ),
            hit.kind,
            hit.why
        );
    }
    if hits.is_empty() {
        println!("  nothing");
    }
    println!();
    Ok(!hits.is_empty())
}

pub async fn run(data_dir: &Path, report: bool) -> Result<bool, EvalError> {
    let fixture = Path::new("fixtures/search/exact-titles.tsv");
    let cases = load(fixture)?;
    let db = Db::open_in(data_dir).await?;

    // THE WHOLE ENGINE, not the keyword half. E1's budget is keystroke to results and
    // says explicitly that the query embedding is inside it, so timing anything less
    // than this would be reporting a number the criterion does not ask for.
    //
    // Keyword-only when the model or the index is absent — which is not a fallback for
    // the harness's convenience, it is the Tier 0 path (SPEC.md 8, ADR-0014) and it
    // deserves to be measured too. The banner says which one ran.
    let (mut engine, hybrid) = engine(data_dir)?;

    let mut outcomes = Vec::with_capacity(cases.len());
    for case in cases {
        let started = Instant::now();
        let hits = engine.search(&db, &case.query, 5).await?;
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

    // E3 shares this harness's engine deliberately: a relevance number taken against a
    // differently-configured engine than E1 and E2 would not be comparable to them.
    let relevance = crate::relevance::run(data_dir, &mut engine, &db).await?;

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
    println!(
        "  E1 — latency over the fixture ({})",
        if hybrid {
            "hybrid: query embedding + BM25 + vectors, fused"
        } else {
            "keyword only — no model or no index present"
        }
    );
    println!(
        "     p50 {:.1} ms   p95 {:.1} ms   max {:.1} ms   (target p95 < 80 ms)",
        p(0.50),
        p(0.95),
        times.last().copied().unwrap_or(0.0)
    );

    let relevance_passed = crate::relevance::report(&relevance, report);

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
    if !relevance_passed {
        println!("  E3 NOT MET: the meaning half is below the 0.75 target.");
    }
    Ok(e2_met && e1_met && relevance_passed)
}
