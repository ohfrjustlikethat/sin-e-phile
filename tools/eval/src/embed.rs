//! The embedder harness: does a query cost what E1 can afford, and do the two halves
//! of the system still agree about what a vector is?
//!
//! # The agreement check is the important half
//!
//! A query is compared against document vectors by cosine, and that comparison is only
//! meaningful if both were produced the same way. Nothing enforces that at compile
//! time: tokenizer settings, truncation, pooling and normalisation are four
//! opportunities to differ, and every one of them fails *silently* — search gets
//! quietly worse and there is nothing to point at.
//!
//! So this re-derives a document exactly as the producer built it, embeds it with the
//! query path's embedder, quantises it, and compares the result against that title's
//! vector **in the published artefact**. ADR-0014 requires the artefact to be
//! deterministic, so the correct outcome is not "close" — it is byte-identical.
//!
//! # The latency half is E1's missing number
//!
//! E1 budgets 80 ms from keystroke to results and says explicitly that the query
//! embedding is inside that budget. Until now nothing measured it, because nothing
//! embedded a query at all.

use std::path::Path;
use std::time::Instant;

use sinephile_embedding::{quantise, Artefact};
use sinephile_persistence::repositories::CatalogueRepository;
use sinephile_persistence::Db;

use crate::error::EvalError;

/// Queries timed for the latency figure. Short, ordinary things a person types.
const QUERIES: &[&str] = &[
    "films about grief that aren't depressing",
    "like Wong Kar-wai but Korean",
    "slow science fiction",
    "a heist that goes wrong",
    "anime about loneliness in a big city",
    "documentaries about mountains",
    "westerns with no shooting",
    "something funny and short",
    "post-war Italian neorealism",
    "movies where the city is the main character",
];

/// Catalogue items re-embedded and compared against the artefact.
///
/// Ten is enough: this is a check for *drift*, and drift is systematic. A tokenizer
/// setting that changed does not affect one title in a thousand, it affects all of them.
const AGREEMENT_SAMPLES: usize = 10;

/// `--compare "<query>" <id>,<id>,…`: cosine between a query and each item's document,
/// at the shipped synopsis budget and at a longer one.
///
/// Exists because the alternative was a 75-minute re-embed on a hunch. `SYNOPSIS_CHARS`
/// truncates at 400, and *Manchester by the Sea*'s lead puts its plot at characters
/// 280-420 — so the sentence that answers a plot query is cut in half. This measures
/// whether that is actually what costs the ranking, before anything is rebuilt.
pub async fn compare(data_dir: &Path, query: &str, ids: &[i64]) -> Result<bool, EvalError> {
    let models = Path::new("models");
    let mut embedder = sinephile_embedder::Embedder::pinned(
        &models.join("bge-small-en-v1.5-int8.onnx"),
        &models.join("bge-small-en-v1.5-tokenizer.json"),
    )
    .map_err(|e| EvalError::Missing(e.to_string()))?;

    let db = Db::open_in(data_dir).await?;
    let query_vector = embedder
        .embed_query(query)
        .map_err(|e| EvalError::Missing(e.to_string()))?;

    println!();
    println!("  query: {query:?}");
    println!();
    println!(
        "  {:<34} {:>9} {:>9}  {:>5}",
        "item", "shipped", "longer", "chars"
    );

    for id in ids {
        let Some(document) = sinephile_ingest::embed::document_for(&db, *id)
            .await
            .map_err(|e| EvalError::Missing(e.to_string()))?
        else {
            println!("  {id}: not in the core tier");
            continue;
        };
        let title: String =
            sqlx::query_scalar("SELECT primary_title FROM media_items WHERE id = ?")
                .bind(id)
                .fetch_one(db.pool())
                .await?;
        let full: Option<String> =
            sqlx::query_scalar("SELECT synopsis FROM media_items WHERE id = ?")
                .bind(id)
                .fetch_one(db.pool())
                .await?;

        // The shipped document, and the same document with the whole synopsis rather
        // than its first 400 characters. The model reads 256 tokens either way, so the
        // second is what the budget is leaving on the table.
        let longer = match &full {
            Some(text) => format!("{document} {text}"),
            None => document.clone(),
        };

        let a = embedder
            .embed_document(&document)
            .map_err(|e| EvalError::Missing(e.to_string()))?;
        let b = embedder
            .embed_document(&longer)
            .map_err(|e| EvalError::Missing(e.to_string()))?;

        println!(
            "  {:<34} {:>9.4} {:>9.4}  {:>5}",
            title.chars().take(34).collect::<String>(),
            sinephile_embedding::cosine(&query_vector, &a),
            sinephile_embedding::cosine(&query_vector, &b),
            full.as_ref().map(|t| t.len()).unwrap_or(0)
        );
    }
    println!();
    Ok(true)
}

pub async fn run(data_dir: &Path, report: bool) -> Result<bool, EvalError> {
    let models = Path::new("models");
    let model = models.join("bge-small-en-v1.5-int8.onnx");
    let tokenizer = models.join("bge-small-en-v1.5-tokenizer.json");
    for path in [&model, &tokenizer] {
        if !path.is_file() {
            return Err(EvalError::Missing(format!(
                "{} is missing — see `ingest embed` for where it comes from",
                path.display()
            )));
        }
    }

    let loading = Instant::now();
    let mut embedder = sinephile_embedder::Embedder::pinned(&model, &tokenizer)
        .map_err(|e| EvalError::Missing(e.to_string()))?;
    let load_millis = loading.elapsed().as_secs_f64() * 1000.0;

    // A cold first inference includes ONNX Runtime's own warm-up, which a user pays
    // once and E1 should not be charged for on every keystroke. Timed separately rather
    // than hidden by discarding it.
    let first = Instant::now();
    embedder
        .embed_query("warm up")
        .map_err(|e| EvalError::Missing(e.to_string()))?;
    let first_millis = first.elapsed().as_secs_f64() * 1000.0;

    let mut latencies = Vec::with_capacity(QUERIES.len());
    for query in QUERIES {
        let started = Instant::now();
        let vector = embedder
            .embed_query(query)
            .map_err(|e| EvalError::Missing(e.to_string()))?;
        latencies.push(started.elapsed().as_secs_f64() * 1000.0);
        debug_assert_eq!(vector.len(), sinephile_embedder::DIMENSION as usize);
    }
    latencies.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a duration"));
    let p50 = percentile(&latencies, 0.50);
    let p95 = percentile(&latencies, 0.95);

    // ── agreement ────────────────────────────────────────────────────────────────
    let artefact_path = sinephile_embedding::artefact_path(data_dir);
    let mut file = std::fs::File::open(&artefact_path).map_err(|source| EvalError::Io {
        path: artefact_path.display().to_string(),
        source,
    })?;
    let artefact = Artefact::read(&mut file)?;

    let db = Db::open_in(data_dir).await?;
    let ids = CatalogueRepository::new(&db).core_ids().await?;
    if ids.len() as u64 != artefact.header.count {
        return Err(EvalError::Missing(format!(
            "the artefact holds {} vectors and the catalogue offers {} core ids — \
             they must line up position for position",
            artefact.header.count,
            ids.len()
        )));
    }

    let stride = (ids.len() / AGREEMENT_SAMPLES).max(1);
    let mut identical = 0usize;
    let mut worst: Option<(i64, f32, String)> = None;
    let mut checked = 0usize;

    for position in (0..ids.len()).step_by(stride).take(AGREEMENT_SAMPLES) {
        let id = ids[position];
        let Some(document) = sinephile_ingest::embed::document_for(&db, id)
            .await
            .map_err(|e| EvalError::Missing(e.to_string()))?
        else {
            continue;
        };

        let fresh = quantise::quantise(
            &embedder
                .embed_document(&document)
                .map_err(|e| EvalError::Missing(e.to_string()))?,
        );
        let stored = artefact
            .vector(position as u64)
            .ok_or_else(|| EvalError::Missing(format!("no vector at position {position}")))?;

        checked += 1;
        if fresh.values == stored {
            identical += 1;
        }
        // Reported even when identical: a similarity that is 1.0000 while the bytes
        // differ would mean the drift is inside quantisation rather than the model.
        let similarity = sinephile_vector_index::brute::cosine_i8(&fresh.values, stored);
        if worst.as_ref().is_none_or(|(_, s, _)| similarity < *s) {
            worst = Some((id, similarity, document.chars().take(60).collect()));
        }
    }

    let all_identical = identical == checked && checked > 0;

    if report {
        println!("embed — query latency, and whether the two halves still agree\n");
        println!("  model            {}", model.display());
        println!("  load             {load_millis:.0} ms  (once, at startup)");
        println!("  first inference  {first_millis:.1} ms  (ONNX warm-up, paid once)");
        println!();
        println!("  queries          {}", QUERIES.len());
        println!("  latency          p50 {p50:.1} ms   p95 {p95:.1} ms");
        println!();
        println!("  agreement        {identical}/{checked} byte-identical to the artefact");
        if let Some((id, similarity, text)) = &worst {
            println!("  weakest          item {id}, cosine {similarity:.6}");
            println!("                   {text}…");
        }
        println!();
        println!("  E1's budget is 80 ms keystroke-to-results INCLUDING this. The keyword");
        println!("  half measures 7.3 ms p95 warm and the vector half 4.2 ms, so the sum is");
        println!("  the number that matters — `eval search` will report it once fusion lands.");
    }

    if !all_identical {
        println!(
            "\nthe re-embedded documents are NOT byte-identical to the artefact \
             ({identical}/{checked}) — the producer and the query path have drifted, and \
             every cosine between a query and a document is now measuring two different \
             spaces."
        );
    }

    Ok(all_identical)
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() as f64 * fraction).ceil() as usize).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}
