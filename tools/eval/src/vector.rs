//! The vector-index harness: how much of the truth the approximate index actually finds.
//!
//! # Why recall is measured before any relevance number is reported
//!
//! HNSW is *approximate*. It can miss a neighbour and say nothing, and every downstream
//! number — E3's nDCG, E4's two hard queries, the rank fusion in subtask 5.4 — is
//! computed over whatever it returned. A poor nDCG would be indistinguishable from a
//! good ranking over a lossy candidate set, and the natural response (tune the ranking)
//! would be work spent on the wrong half.
//!
//! So this runs first and separately: **is the candidate set the right one**, measured
//! against a brute-force scan of the same artefact, which is exact by construction.
//!
//! # This harness is not E1
//!
//! It times the index and nothing else. E1's budget is keystroke-to-results over the
//! whole engine, including a query the model has to embed and a BM25 pass the keyword
//! half runs. This is one component's contribution to that budget, reported so the rest
//! of it can be spent knowingly.

use std::path::Path;
use std::time::Instant;

use sinephile_embedding::Artefact;
use sinephile_persistence::repositories::CatalogueRepository;
use sinephile_persistence::Db;
use sinephile_vector_index::{brute, index_path, VectorIndex};

use crate::error::EvalError;

/// How many queries. Every one costs a full brute-force scan of the artefact, which is
/// 855,703 × 384 multiply-adds — so this is chosen to keep the harness inside a couple
/// of minutes while leaving the recall estimate tight enough to act on.
const QUERIES: usize = 200;

/// Neighbours asked for. Ten because that is what a result page shows, and recall of a
/// list nobody sees is not the property under test.
const K: usize = 10;

/// The gate. Below this the index is losing candidates the ranking can never recover,
/// and no relevance number taken over it means anything.
///
/// 0.95 rather than 1.0: an exact index is a brute-force scan, which is the thing HNSW
/// exists to avoid. Losing one candidate in twenty from a set of ten, where the ranking
/// re-orders them anyway and the exact-title short-circuit sits above all of it, is a
/// trade that buys three orders of magnitude of latency.
const RECALL_TARGET: f64 = 0.95;

struct Outcome {
    position: usize,
    recall: f64,
    millis: f64,
}

/// How wide a band around the truth's last place still counts as "as good as".
///
/// A recall of 0.2 has two opposite explanations and they must be told apart. Either the
/// graph missed genuinely closer neighbours — a real loss — or the query sits in a
/// crowded band where thousands of vectors are equally distant, "the true top ten" is an
/// arbitrary ten of them, and any different-but-equally-good choice is scored as a miss.
///
/// 1% of the cutoff distance: tight enough that a real miss stays a miss, wide enough to
/// catch the crowd.
const BAND: f32 = 0.01;

/// `--memory`: open the index both ways and report what each costs.
///
/// P11 chose usearch because `view` maps the file instead of reading it, and `SPEC.md`
/// §2.3 caps idle RAM at 250 MB on the tier this index exists to serve. That is a claim
/// about two numbers, so here are the two numbers.
pub fn memory(data_dir: &Path) -> Result<bool, EvalError> {
    let path = index_path(data_dir);

    let started = Instant::now();
    let viewed = VectorIndex::view(&path)?;
    let view_millis = started.elapsed().as_secs_f64() * 1000.0;
    let view_bytes = viewed.memory_usage();
    drop(viewed);

    let started = Instant::now();
    let loaded = VectorIndex::load(&path)?;
    let load_millis = started.elapsed().as_secs_f64() * 1000.0;
    let load_bytes = loaded.memory_usage();

    let on_disk = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!("vector — what opening the index costs\n");
    println!("  on disk          {:.0} MB", on_disk as f64 / 1_048_576.0);
    println!(
        "  view  (mmap)     {:>8.1} MB resident   opened in {view_millis:.0} ms",
        view_bytes as f64 / 1_048_576.0
    );
    println!(
        "  load  (read)     {:>8.1} MB resident   opened in {load_millis:.0} ms",
        load_bytes as f64 / 1_048_576.0
    );
    println!();
    println!("  Tier 0's whole idle budget is 250 MB (SPEC.md §2.3).");

    // The comparison IS the point, so it fails when there is nothing to compare — a
    // `view` that had quietly read the file would otherwise look like a pass.
    Ok(view_bytes < load_bytes)
}

pub async fn run(
    data_dir: &Path,
    report: bool,
    prove: bool,
    expansion: Option<usize>,
) -> Result<bool, EvalError> {
    let artefact_path = sinephile_embedding::artefact_path(data_dir);
    let mut file = std::fs::File::open(&artefact_path).map_err(|source| EvalError::Io {
        path: artefact_path.display().to_string(),
        source,
    })?;
    let artefact = Artefact::read(&mut file)?;

    let db = Db::open_in(data_dir).await?;
    // The SAME subset the index was built from, or recall compares two different
    // populations and means nothing. Positions with no descriptive text are not in the
    // graph and must not be in the ground truth either.
    // The SAME subset the index was built from, holes included, or recall compares two
    // different populations and means nothing.
    let mut ids = CatalogueRepository::new(&db).core_ids_for_vectors().await?;
    let indexable = ids.iter().filter(|id| id.is_some()).count();

    if prove {
        // THE NEGATIVE CONTROL. A check that has never been seen to fail is not
        // evidence, and "the keys map to the right films" is the claim this harness
        // rests on — a wrong mapping still returns ten plausible films for every query.
        //
        // Rotating the id list by one leaves the artefact, the index and the metric
        // untouched and makes every position name the next film along. Recall must
        // collapse. If it does not, this harness is measuring usearch against itself
        // and every number it has ever printed is worthless.
        ids.rotate_left(1);
        println!("PROVING THE HARNESS: the id mapping is deliberately shifted by one.");
        println!("Recall must collapse. If it does not, the harness is not measuring anything.\n");
    }

    let path = index_path(data_dir);
    if !path.exists() {
        return Err(EvalError::Missing(format!(
            "{} does not exist — run `ingest vector-index` first",
            path.display()
        )));
    }

    let opened = Instant::now();
    let index = VectorIndex::view(&path)?;
    let open_millis = opened.elapsed().as_secs_f64() * 1000.0;

    // `--ef N` sweeps the recall/latency dial. The shipped value is whatever this sweep
    // showed clearing the target with margin; without the sweep the constant would be a
    // guess with a confident comment attached.
    let expansion = expansion.unwrap_or(sinephile_vector_index::EXPANSION_SEARCH);
    index.set_expansion_search(expansion);

    if index.len() != indexable {
        return Err(EvalError::Missing(format!(
            "the index holds {} vectors and the catalogue offers {} core ids — \
             rebuild it with `ingest vector-index`",
            index.len(),
            ids.len()
        )));
    }

    // Deterministic and spread across the whole artefact rather than random or
    // front-loaded: ids ascend roughly with registration date, so the first 200 vectors
    // would all be a century old and would answer a different question.
    let stride = (artefact.header.count as usize / QUERIES).max(1);
    let positions: Vec<usize> = (0..QUERIES)
        .map(|i| i * stride)
        .filter(|p| *p < artefact.header.count as usize)
        .collect();

    let mut outcomes = Vec::with_capacity(positions.len());
    for position in &positions {
        let query = artefact
            .vector(*position as u64)
            .expect("position is inside the artefact")
            .to_vec();

        let started = Instant::now();
        let found = index.search(&query, K)?;
        let millis = started.elapsed().as_secs_f64() * 1000.0;

        let truth = brute::nearest(&artefact, &ids, &query, K);
        outcomes.push(Outcome {
            position: *position,
            recall: brute::recall(&truth, &found),
            millis,
        });
    }

    let mean_recall = outcomes.iter().map(|o| o.recall).sum::<f64>() / outcomes.len().max(1) as f64;
    let perfect = outcomes.iter().filter(|o| o.recall >= 1.0).count();
    let worst = outcomes
        .iter()
        .min_by(|a, b| {
            a.recall
                .partial_cmp(&b.recall)
                .expect("no NaN in a recall")
                .then(a.position.cmp(&b.position))
        })
        .expect("at least one query");

    let mut latencies: Vec<f64> = outcomes.iter().map(|o| o.millis).collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a duration"));
    let p50 = percentile(&latencies, 0.50);
    let p95 = percentile(&latencies, 0.95);

    if report {
        println!("vector — HNSW recall against brute force over the same artefact\n");
        println!("  index            {}", path.display());
        println!(
            "  vectors          {} in {:.0} MB on disk",
            index.len(),
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) as f64 / 1_048_576.0
        );
        println!("  opened (mmap)    {open_millis:.1} ms");
        println!("  expansion_search {expansion}");
        println!(
            "  usearch reports  {:.1} MB resident",
            index.memory_usage() as f64 / 1_048_576.0
        );
        println!();
        println!("  queries          {}", outcomes.len());
        println!(
            "  recall@{K}        {:.4}  (target {RECALL_TARGET:.2})",
            mean_recall
        );
        println!(
            "  perfect          {perfect}/{}  worst {:.2}",
            outcomes.len(),
            worst.recall
        );

        // WHY THE WORST CASE IS WORTH ONE MORE SCAN. A recall of 0.2 reads as a broken
        // graph. It can equally be a query whose neighbourhood is a pile of identical
        // vectors, where "the true top ten" is an arbitrary ten of a hundred equals and
        // any different-but-equal choice is scored as a miss. Those are opposite
        // conclusions, so the tie set is counted rather than guessed at.
        let query = artefact
            .vector(worst.position as u64)
            .expect("position is inside the artefact")
            .to_vec();
        let truth = brute::nearest(&artefact, &ids, &query, K);
        let nearest = truth.first().map(|n| n.distance).unwrap_or(0.0);
        let cutoff = truth.last().map(|n| n.distance).unwrap_or(0.0);
        let crowd = (0..artefact.header.count)
            .filter_map(|p| artefact.vector(p))
            .filter(|v| 1.0 - brute::cosine_i8(&query, v) <= cutoff * (1.0 + BAND))
            .count();
        println!(
            "  worst query      position {} — nearest {nearest:.4}, 10th {cutoff:.4},",
            worst.position
        );
        println!("                   {crowd} vectors within 1% of that 10th distance");
        println!("  latency          p50 {p50:.2} ms   p95 {p95:.2} ms");
        println!();
        println!("  Query embedding is NOT included — this is the index alone. E1's 80 ms");
        println!("  budget covers keystroke to results, which is measured by `eval search`.");
    }

    let passed = mean_recall >= RECALL_TARGET;
    if !passed && !prove {
        println!(
            "\nrecall@{K} {mean_recall:.4} is below the {RECALL_TARGET:.2} target — \
             the candidate set is lossy, so no relevance number taken over it is meaningful."
        );
    }
    if prove {
        // Inverted on purpose: with the mapping shifted, HIGH recall is the failure.
        let collapsed = mean_recall < 0.5;
        println!(
            "\nshifted mapping gave recall@{K} {mean_recall:.4} — harness {}",
            if collapsed {
                "MEASURES SOMETHING REAL"
            } else {
                "IS BROKEN: a wrong mapping scored well"
            }
        );
        return Ok(collapsed);
    }

    Ok(passed)
}

/// Nearest-rank percentile over an already-sorted slice.
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() as f64 * fraction).ceil() as usize).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}
