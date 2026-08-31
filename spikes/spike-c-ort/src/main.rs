//! Spike C — ONNX Runtime in Rust on Windows (SPEC.md Phase 1, risk R3).
//!
//! SPIKE CODE. Throwaway. Not the Phase 5 design.
//!
//! ADR-0015 raised R3 from Moderate to Moderate/Severe by establishing that query
//! embedding runs on EVERY tier, not just Tier 1+. So this spike measures the
//! thing that actually gates Phase 5:
//!
//!   **Query-embedding latency, specifically.** Model already loaded, a single
//!   ~30-token query, wall clock to a returned vector, p50 and p95.
//!   NOT document throughput, NOT an amortised average.
//!
//! ESCALATION TRIGGER (docs/RISKS.md R3): p95 above ~30 ms escalates under §10.9.
//! That is a third of §2.3's 80 ms keystroke-to-results budget, which also has to
//! cover ANN search, BM25, reciprocal rank fusion and render. Do NOT widen the
//! budget to accommodate a slow model.
//!
//! Also measured, because §2.3 budgets Tier 0 idle RAM at 250 MB: model load time
//! and resident memory.
//!
//! Usage: spike-c-ort <model.onnx> <tokenizer.json>

use std::time::Instant;

use anyhow::{Context, Result};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::TensorRef;
use tokenizers::Tokenizer;

/// ort's error type is not Send + Sync, so anyhow's `?` cannot convert it.
/// Flatten to a message rather than changing the whole error strategy for a spike.
macro_rules! ort_try {
    ($e:expr) => {
        $e.map_err(|e| anyhow::anyhow!("ort: {e}"))?
    };
}

/// Queries shaped like the ones §4 and Phase 5 actually promise.
const QUERIES: &[&str] = &[
    "slow films about loneliness",
    "like Wong Kar-wai but Korean",
    "films about grief that aren't depressing",
    "1970s political thrillers with an ambiguous ending",
    "quiet japanese drama, nothing happens, very beautiful",
    "heist movie where the plan goes wrong immediately",
    "anime films about growing up in a small town",
    "documentaries about people obsessed with a hopeless project",
];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: spike-c-ort <model.onnx> <tokenizer.json>");
        std::process::exit(2);
    }

    let rss_before = resident_mb();

    // ── Load ─────────────────────────────────────────────────────────────────
    let t_load = Instant::now();
    let builder = ort_try!(Session::builder());
    let builder = ort_try!(builder.with_optimization_level(GraphOptimizationLevel::Level3));
    // Tier 0 is >= 2 cores; leave headroom rather than grabbing everything.
    let mut builder = ort_try!(builder.with_intra_threads(2));
    let mut session = ort_try!(builder.commit_from_file(&args[1]));
    let load_ms = t_load.elapsed().as_secs_f64() * 1000.0;

    let mut tokenizer = Tokenizer::from_file(&args[2])
        .map_err(|e| anyhow::anyhow!("loading tokenizer: {e}"))?;
    // This tokenizer.json configures padding to a fixed 128 tokens. Left alone,
    // every query is padded to 128 and the model does ~4x the work a real short
    // query needs — measuring the wrong thing. ADR-0015 asks specifically for a
    // ~30-token query, so padding is disabled and both are reported.
    let padded = std::env::var("SPIKE_PAD").is_ok();
    if !padded {
        tokenizer.with_padding(None);
    }
    println!("padding          {}", if padded { "ON (128 tokens, worst case)" } else { "OFF (true query length)" });

    let rss_after_load = resident_mb();

    println!("model            {}", args[1]);
    println!("load             {load_ms:>8.1} ms");
    println!(
        "resident         {:>8.1} MB  (+{:.1} MB for model + runtime)",
        rss_after_load,
        rss_after_load - rss_before
    );
    println!();

    // ── Warm up: first inference includes lazy allocation, and would otherwise
    //    pollute p50/p95 with a one-off cost the user never sees again. ────────
    let warm = embed(&mut session, &tokenizer, QUERIES[0])?;
    println!("embedding dim    {}", warm.len());
    println!();

    // ── The measurement ──────────────────────────────────────────────────────
    const ROUNDS: usize = 40;
    let mut samples: Vec<f64> = Vec::with_capacity(ROUNDS * QUERIES.len());
    let mut token_counts: Vec<usize> = Vec::new();

    for _ in 0..ROUNDS {
        for q in QUERIES {
            let enc = tokenizer
                .encode(*q, true)
                .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
            token_counts.push(enc.len());

            let t = Instant::now();
            let _v = embed(&mut session, &tokenizer, q)?;
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| samples[((samples.len() - 1) as f64 * q) as usize];
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let avg_tokens = token_counts.iter().sum::<usize>() as f64 / token_counts.len() as f64;

    println!("=== QUERY EMBEDDING LATENCY ===");
    println!("  samples        {} ({} queries x {ROUNDS} rounds)", samples.len(), QUERIES.len());
    println!("  tokens/query   {avg_tokens:.1} average");
    println!("  min            {:>8.2} ms", samples[0]);
    println!("  p50            {:>8.2} ms", p(0.50));
    println!("  mean           {mean:>8.2} ms");
    println!("  p95            {:>8.2} ms   <- THE NUMBER (R3 trigger: > ~30 ms)", p(0.95));
    println!("  p99            {:>8.2} ms", p(0.99));
    println!("  max            {:>8.2} ms", samples[samples.len() - 1]);
    println!();

    let p95 = p(0.95);
    let verdict = if p95 <= 30.0 { "PASS" } else { "ESCALATE under §10.9" };
    println!("  budget context §2.3 allows 80 ms p95 for the WHOLE keystroke-to-results");
    println!("                 path, including ANN search, BM25, fusion and render.");
    println!("  query embedding uses {:.1}% of that budget", p95 / 80.0 * 100.0);
    println!();
    println!("=== SPIKE C VERDICT: {verdict} ===");

    Ok(())
}

/// Tokenize, run the model, mean-pool over the attention mask, L2-normalise.
///
/// Mean pooling then normalising is what sentence-transformers does for this
/// model; skipping either gives vectors that are not comparable by cosine.
fn embed(session: &mut Session, tokenizer: &Tokenizer, text: &str) -> Result<Vec<f32>> {
    let enc = tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("tokenize: {e}"))?;
    let len = enc.len();

    let ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
    let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&i| i as i64).collect();
    let types: Vec<i64> = vec![0; len];

    let shape = [1_i64, len as i64];
    let input_ids = ort_try!(TensorRef::from_array_view((shape, ids.as_slice())));
    let attn = ort_try!(TensorRef::from_array_view((shape, mask.as_slice())));
    let ttype = ort_try!(TensorRef::from_array_view((shape, types.as_slice())));
    let outputs = ort_try!(session.run(ort::inputs![
        "input_ids" => input_ids,
        "attention_mask" => attn,
        "token_type_ids" => ttype,
    ]));

    let (out_shape, data) = ort_try!(outputs[0].try_extract_tensor::<f32>());
    let hidden = *out_shape.last().unwrap() as usize;

    // Mean-pool over non-padding tokens.
    let mut pooled = vec![0f32; hidden];
    let mut n = 0f32;
    for t in 0..len {
        if mask[t] == 0 {
            continue;
        }
        n += 1.0;
        for h in 0..hidden {
            pooled[h] += data[t * hidden + h];
        }
    }
    for v in pooled.iter_mut() {
        *v /= n.max(1.0);
    }

    let norm = pooled.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
    for v in pooled.iter_mut() {
        *v /= norm;
    }
    Ok(pooled)
}

/// Resident set size in MB, via the Windows working-set counter.
fn resident_mb() -> f64 {
    #[cfg(windows)]
    {
        use std::process::Command;
        let pid = std::process::id();
        if let Ok(o) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).WorkingSet64"),
            ])
            .output()
        {
            if let Ok(s) = String::from_utf8(o.stdout) {
                if let Ok(b) = s.trim().parse::<f64>() {
                    return b / 1048576.0;
                }
            }
        }
    }
    0.0
}
