//! Phase 15 E5: the model updates within 2 s of a watch event, without blocking the UI.
//! On the full catalogue with the vector half, for a profile with a real imported history
//! (MovieLens user 3, 650 films, as `eval taste` builds it): a film is watched to the end,
//! and the time is taken from the viewing's write to the rebuilt model, through the same
//! `ModelCache` the app keeps. Five watches; each must show up in the model it times.
//!
//! The app does this rebuild in a background task after the viewing is written
//! (`commands::home::refresh_model`), so the UI never waits for it; what this measures is
//! how long Home can show the model from before the watch.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use sinephile_persistence::repositories::{MediaRepository, NewProfile, ProfileRepository};
use sinephile_persistence::Db;

use crate::error::EvalError;

const BUDGET_MS: f64 = 2_000.0;

pub async fn run(data_dir: &Path) -> Result<bool, EvalError> {
    let (mut engine, _) = crate::search::engine(data_dir)?;
    let db = Db::open_in(data_dir).await?;
    let profiles = ProfileRepository::new(&db);
    let cache = sinephile_taste::model::ModelCache::default();
    println!("update — a watch event to an updated taste model (target under 2 s)\n");
    let me = profiles
        .create_with(&NewProfile {
            name: &format!("eval-update-{}", std::process::id()),
            ..Default::default()
        })
        .await?;
    let outcome = async {
        crate::taste::import_user(data_dir, &db, me, 3).await?;
        let semantic = engine
            .semantic_mut()
            .ok_or_else(|| EvalError::Missing("the vector index and model are needed".into()))?;
        let today = sinephile_taste::model::day_of(&sinephile_downloads::now_iso()).unwrap_or(0.0);
        let t = Instant::now();
        let (_, seen) = cache.get(&db, semantic, me, today).await?;
        println!(
            "  the model at launch: {:.1} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
        // Five well-known films the profile has not met, with vectors.
        let fresh: Vec<i64> = MediaRepository::new(&db)
            .most_voted_films(400)
            .await?
            .into_iter()
            .filter(|id| !seen.contains(id) && semantic.index.vector(*id).is_some())
            .take(5)
            .collect();
        let mut worst = 0.0f64;
        let mut all_in = true;
        for id in fresh {
            let t0 = Instant::now();
            profiles.record_viewing(me, id, 6_000, true, 6_000).await?;
            let (modes, met) = cache.get(&db, semantic, me, today).await?;
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let members: HashSet<i64> = modes.iter().flat_map(|m| m.members.clone()).collect();
            let shows = met.contains(&id) && members.contains(&id);
            all_in &= shows;
            worst = worst.max(ms);
            println!(
                "  watched {id:>9}: model updated in {ms:>6.1} ms; the film is in it: {shows}"
            );
        }
        // And with nothing new, Home pays only the fingerprint.
        let t = Instant::now();
        cache.get(&db, semantic, me, today).await?;
        println!(
            "  nothing new: {:.1} ms (the cached model)",
            t.elapsed().as_secs_f64() * 1000.0
        );
        Ok::<_, EvalError>((worst, all_in))
    }
    .await;
    profiles.delete(me).await?;
    let (worst, all_in) = outcome?;
    let ok = worst < BUDGET_MS && all_in;
    println!(
        "\n  slowest update {worst:.1} ms (target under {BUDGET_MS:.0} ms); every watch in its model: {all_in}"
    );
    println!("\n  E5 {}", if ok { "MET" } else { "NOT MET" });
    Ok(ok)
}
