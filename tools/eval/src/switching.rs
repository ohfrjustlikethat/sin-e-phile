//! Phase 14 E5: profile switching is instant. A switch is one write (`touch`) and the new
//! profile's Home: its rails, with the taste seed built from its history. Timed on the full
//! catalogue with the vector half, for a profile with nothing and for one with a real
//! imported history (MovieLens user 3, as `eval taste` builds it), each five times.

use std::path::Path;
use std::time::Instant;

use sinephile_persistence::repositories::{NewProfile, ProfileRepository};
use sinephile_persistence::Db;

use crate::error::EvalError;

fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(v.len() - 1)]
}

pub async fn run(data_dir: &Path) -> Result<bool, EvalError> {
    let (mut engine, _) = crate::search::engine(data_dir)?;
    let db = Db::open_in(data_dir).await?;
    let profiles = ProfileRepository::new(&db);
    println!("switch — a profile switch: one write and the new profile's Home\n");

    let empty = profiles
        .create_with(&NewProfile {
            name: &format!("eval-switch-empty-{}", std::process::id()),
            ..Default::default()
        })
        .await?;
    let full = profiles
        .create_with(&NewProfile {
            name: &format!("eval-switch-full-{}", std::process::id()),
            ..Default::default()
        })
        .await?;
    let outcome = async {
        crate::taste::import_user(data_dir, &db, full, 3).await?;
        let mut worst = 0.0f64;
        for (name, id) in [("nothing yet", empty), ("650 films imported", full)] {
            let mut times = Vec::new();
            for _ in 0..5 {
                let t0 = Instant::now();
                profiles.touch(id).await?;
                let rails =
                    sinephile_taste::home::rails(&db, engine.semantic_mut(), id, false, None)
                        .await?;
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
                assert!(!rails.is_empty());
            }
            let (first, p) = (times[0], p95(times.clone()));
            worst = worst.max(p);
            println!(
                "  {name:<20} first {first:>6.1} ms   p95 {p:>6.1} ms   (target under 100 ms)"
            );
        }
        // Where the time goes, for the full profile.
        let t = Instant::now();
        let liked = profiles.liked(full).await?;
        let t_liked = t.elapsed().as_secs_f64() * 1000.0;
        let semantic = engine.semantic_mut().expect("vector half");
        let t = Instant::now();
        let seed = sinephile_taste::seed(semantic, &liked, None).expect("seed");
        let t_seed = t.elapsed().as_secs_f64() * 1000.0;
        let seen: std::collections::HashSet<i64> = liked.iter().map(|(id, _)| *id).collect();
        let t = Instant::now();
        let _ = sinephile_taste::nearest(semantic, &seed, 20, &seen);
        let t_near = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let _ = sinephile_taste::home::rails(&db, None, full, false, None).await?;
        let t_plain = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "    (650 films: liked() {t_liked:.1} ms, seed of {} {t_seed:.1} ms, nearest {t_near:.1} ms, rails without taste {t_plain:.1} ms)",
            liked.len()
        );
        Ok::<_, EvalError>(worst)
    }
    .await;
    profiles.delete(empty).await?;
    profiles.delete(full).await?;
    let worst = outcome?;
    let ok = worst < 100.0;
    println!("\n  E5 (instant) {}", if ok { "MET" } else { "NOT MET" });
    Ok(ok)
}
