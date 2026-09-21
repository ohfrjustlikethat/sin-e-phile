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

/// `--compare "<query>" <id>,<id>,…`: how each item's document would score against a
/// query under several layouts, and — the part that matters — **in what order**.
///
/// Exists because the alternative was a three-hour re-embed on a hunch.
///
/// # Why it reports ranks and not just cosines
///
/// D39 priced a layout change here and read a rise of +0.065 on the right answer as
/// encouraging, until the same +0.065 turned up on the two wrong answers above it. A
/// cosine that goes up on everything changes nothing a user sees. **Search is ordinal**,
/// so the question a variant has to answer is whether anything moved past anything else.
///
/// # And why the variants are rebuilt rather than concatenated
///
/// The three variants this replaces were assembled with `format!` — `"{document}
/// {synopsis}"` and `"{stripped} {document}"`. Both contain the first 400 characters of
/// the synopsis **twice**, and the second contains the metadata twice as well, so they
/// measured documents no rebuild would ever produce. These go through
/// `document::build_with`, which is the producer's own builder: what is measured here is
/// what a re-embed would write.
pub async fn compare(data_dir: &Path, query: &str, ids: &[i64]) -> Result<bool, EvalError> {
    use sinephile_embedding::document::{Layout, SHIPPED};

    /// The layouts on offer, shipped first.
    ///
    /// `1000 chars` because the model reads 256 tokens — roughly a thousand characters —
    /// against a budget of 400, so a third of what it could read is being thrown away
    /// before it ever sees it. The rest are D39's untried residue: the document still
    /// opens with year, genres and up to six cast names, and CLS pooling weights the
    /// head of the sequence most.
    const VARIANTS: &[(&str, Layout)] = &[
        ("shipped", SHIPPED),
        (
            "1000 chars",
            Layout {
                synopsis_chars: 1_000,
                ..SHIPPED
            },
        ),
        (
            "plot first",
            Layout {
                synopsis_chars: 1_000,
                synopsis_first: true,
                strip_lead: true,
                ..SHIPPED
            },
        ),
        (
            "no cast",
            Layout {
                synopsis_chars: 1_000,
                synopsis_first: true,
                strip_lead: true,
                people: false,
            },
        ),
    ];

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

    // [variant][item] cosine, so the ranking can be read down a column afterwards.
    let mut scores: Vec<Vec<f32>> = vec![Vec::new(); VARIANTS.len()];
    let mut titles: Vec<String> = Vec::new();

    for id in ids {
        let title: String =
            sqlx::query_scalar("SELECT primary_title FROM media_items WHERE id = ?")
                .bind(id)
                .fetch_one(db.pool())
                .await?;

        let mut row = Vec::with_capacity(VARIANTS.len());
        for (_, layout) in VARIANTS {
            let Some(document) = sinephile_catalogue::embed::document_for_layout(&db, *id, *layout)
                .await
                .map_err(|e| EvalError::Missing(e.to_string()))?
            else {
                println!("  {id}: not in the core tier");
                break;
            };
            let vector = embedder
                .embed_document(&document)
                .map_err(|e| EvalError::Missing(e.to_string()))?;
            row.push(sinephile_embedding::cosine(&query_vector, &vector));
        }

        if row.len() == VARIANTS.len() {
            for (column, value) in row.into_iter().enumerate() {
                scores[column].push(value);
            }
            titles.push(title);
        }
    }

    println!();
    println!("  query: {query:?}");
    println!();
    print!("  {:<34}", "item");
    for (name, _) in VARIANTS {
        print!("{name:>18}");
    }
    println!();

    // Ranked once per variant, not once per cell: `ranking` sorts, and calling it inside
    // the inner loop re-sorted the same column for every row.
    let orders: Vec<Vec<usize>> = scores.iter().map(|column| ranking(column)).collect();
    let shipped_order = &orders[0];

    for (item, title) in titles.iter().enumerate() {
        print!("  {:<34}", title.chars().take(33).collect::<String>());
        for (column, order) in scores.iter().zip(&orders) {
            print!("{:>11.4} #{:<5}", column[item], order[item] + 1);
        }
        println!();
    }

    println!();
    let mut moved = false;
    for ((name, _), order) in VARIANTS.iter().zip(&orders).skip(1) {
        if order == shipped_order {
            println!("  {name:<12} same order — nothing a user would see changed");
        } else {
            moved = true;
            println!("  {name:<12} REORDERED against shipped:");
            for (item, title) in titles.iter().enumerate() {
                if order[item] != shipped_order[item] {
                    println!(
                        "                 {:<34} #{} → #{}",
                        title.chars().take(33).collect::<String>(),
                        shipped_order[item] + 1,
                        order[item] + 1
                    );
                }
            }
        }
    }
    println!();

    // A variant that reorders nothing is a re-embed that buys nothing, and saying so is
    // the entire value of this command. Not an error either way — it is a measurement.
    if !moved {
        println!("  No variant changes the ranking. A re-embed on this evidence buys nothing.");
        println!();
    }
    Ok(true)
}

/// Position of each item when sorted by score, descending. `out[i]` is item `i`'s rank.
fn ranking(scores: &[f32]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|a, b| {
        scores[*b]
            .partial_cmp(&scores[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut rank = vec![0usize; scores.len()];
    for (position, item) in order.into_iter().enumerate() {
        rank[item] = position;
    }
    rank
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
    let mut ids = CatalogueRepository::new(&db).core_ids().await?;

    // THE CATALOGUE OUTGROWS THE ARTEFACT, and that is by design now.
    //
    // Subtask 5.7 made the catalogue refresh itself on every launch (ADR-0030), so the
    // artefact — a published snapshot — falls behind the moment IMDb republishes. This
    // check used to demand equality and simply stopped working: 855,703 vectors against
    // 858,170 core ids, a week after the artefact was cut.
    //
    // A catalogue that is a strict PREFIX-superset is fine: ids are assigned in order,
    // refreshed titles land after every id the artefact covers, and position *n* still
    // means the same title. So compare over the artefact's length and say plainly how
    // far ahead the catalogue has run. FEWER ids is a different matter — something was
    // deleted, positions have shifted, and nothing positional can be trusted.
    if (ids.len() as u64) < artefact.header.count {
        return Err(EvalError::Missing(format!(
            "the artefact holds {} vectors and the catalogue offers only {} core ids — \
             items have been REMOVED, so every position after the first gap is wrong",
            artefact.header.count,
            ids.len()
        )));
    }
    let behind = ids.len() as u64 - artefact.header.count;
    if behind > 0 {
        println!();
        println!(
            "  note: the catalogue has grown {behind} core titles past the artefact \
             ({} vectors, {} ids).",
            artefact.header.count,
            ids.len()
        );
        println!("        Agreement is checked over the artefact's own range.");
        ids.truncate(artefact.header.count as usize);
    }

    let stride = (ids.len() / AGREEMENT_SAMPLES).max(1);
    let mut identical = 0usize;
    let mut worst: Option<(i64, f32, String)> = None;
    let mut checked = 0usize;

    for position in (0..ids.len()).step_by(stride).take(AGREEMENT_SAMPLES) {
        let id = ids[position];
        let Some(document) = sinephile_catalogue::embed::document_for(&db, id)
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
