//! Surprise me's speed (the author, 2026-11-21: "very slow"). On a real data folder and its
//! first profile, with the catalogue's vector half: Home's rails built cold and warm, then
//! Surprise me pressed three times, each timed from the press to the film chosen, through
//! the same `ModelCache` the app keeps.

use std::path::Path;
use std::time::Instant;

use sinephile_persistence::repositories::ProfileRepository;
use sinephile_persistence::Db;
use sinephile_taste::home::{rails_at, Ask};
use sinephile_taste::model::ModelCache;
use sinephile_taste::surprise::surprise;

use crate::error::EvalError;

/// What a press may take before it feels like waiting.
const BUDGET_MS: f64 = 300.0;

pub async fn run(data_dir: &Path) -> Result<bool, EvalError> {
    let (mut engine, _) = crate::search::engine(data_dir)?;
    let db = Db::open_in(data_dir).await?;
    let me = ProfileRepository::new(&db).ensure_default().await?;
    let cache = ModelCache::default();
    let ask = Ask { hour: Some(20) };
    println!("surprise — Surprise me, from the press to the film (target under {BUDGET_MS} ms)\n");

    // Pressed before Home was ever built (the app opened on another page): Home is built.
    let fresh = ModelCache::default();
    let t = Instant::now();
    surprise(&db, engine.semantic_mut(), me, false, None, &fresh, ask, 0).await?;
    println!(
        "  Surprise me before any Home: {:>8.1} ms (builds Home, cold)",
        ms(t)
    );
    let t = Instant::now();
    rails_at(&db, engine.semantic_mut(), me, false, None, &cache, ask).await?;
    println!("  Home's rails, cold: {:>8.1} ms", ms(t));
    let t = Instant::now();
    rails_at(&db, engine.semantic_mut(), me, false, None, &cache, ask).await?;
    println!("  Home's rails, warm: {:>8.1} ms", ms(t));

    let mut worst = 0.0f64;
    for seed in 1..=3u64 {
        let t = Instant::now();
        let chosen = surprise(
            &db,
            engine.semantic_mut(),
            me,
            false,
            None,
            &cache,
            ask,
            seed,
        )
        .await?;
        let took = ms(t);
        worst = worst.max(took);
        println!(
            "  Surprise me, press {seed}: {took:>8.1} ms ({:?})",
            chosen.map(|c| c.id)
        );
    }
    let met = worst < BUDGET_MS;
    println!(
        "\n  slowest press {worst:.1} ms: {}",
        if met { "within budget" } else { "OVER budget" }
    );
    Ok(met)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
