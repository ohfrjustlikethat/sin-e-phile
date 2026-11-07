//! Phase 15 E1: the model finds distinct, recognisable modes for a synthetic profile with
//! two deliberately unrelated tastes. The profile loves the 30 most-voted horror films and
//! the 30 most-voted documentaries (no title in both), on the real catalogue's vectors.
//! Judged: no mode mixes the two tastes much (purity), and each taste lives in modes of
//! its own.

use std::collections::HashMap;
use std::path::Path;

use sinephile_persistence::Db;

use crate::error::EvalError;

async fn top_of(db: &Db, genre: &str, not: &str, n: i64) -> Result<Vec<i64>, EvalError> {
    Ok(sqlx::query_scalar(
        "SELECT m.id FROM media_items m
         WHERE m.kind = 'film' AND m.rating_votes IS NOT NULL AND m.is_adult = 0
           AND EXISTS (SELECT 1 FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                       WHERE mg.media_item_id = m.id AND g.name = ?1)
           AND NOT EXISTS (SELECT 1 FROM media_genres mg JOIN genres g ON g.id = mg.genre_id
                       WHERE mg.media_item_id = m.id AND g.name = ?2)
         ORDER BY m.rating_votes DESC LIMIT ?3",
    )
    .bind(genre)
    .bind(not)
    .bind(n)
    .fetch_all(db.pool())
    .await?)
}

pub async fn run(data_dir: &Path, prove: bool) -> Result<bool, EvalError> {
    let (engine, _) = crate::search::engine(data_dir)?;
    let mut engine = engine;
    let semantic = engine
        .semantic_mut()
        .ok_or_else(|| EvalError::Missing("the vector index and model are needed".into()))?;
    let db = Db::open_in(data_dir).await?;
    println!("modes — a profile with two unrelated tastes: 30 horror films and 30 documentaries\n");
    let horror = top_of(&db, "Horror", "Documentary", 30).await?;
    let docs = top_of(&db, "Documentary", "Horror", 30).await?;
    let taste: HashMap<i64, &str> = horror
        .iter()
        .map(|id| (*id, "horror"))
        .chain(docs.iter().map(|id| (*id, "documentary")))
        .collect();
    let liked: Vec<(i64, Vec<f32>, f64)> = taste
        .keys()
        .filter_map(|id| {
            let v = semantic.index.vector(*id)?;
            Some((*id, v.iter().map(|x| f32::from(*x)).collect(), 1.6))
        })
        .collect();
    // THE NEGATIVE CONTROL (`--prove`): the vectors are shuffled between titles (a fixed
    // shuffle, so the run repeats). The tastes are then noise to the model, and purity must
    // collapse; if it does not, this harness measures nothing.
    // A fixed order, as the app's (weight, then id): k-means seeds from the first.
    let mut liked = liked;
    liked.sort_by_key(|(id, _, _)| *id);
    if prove {
        liked.sort_by_key(|(id, _, _)| *id);
        let mut rng: u64 = 0x9e37_79b9_7f4a_7c15;
        for i in (1..liked.len()).rev() {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let j = (rng % (i as u64 + 1)) as usize;
            let (a, b) = (liked[i].1.clone(), liked[j].1.clone());
            liked[i].1 = b;
            liked[j].1 = a;
        }
        println!("PROVING THE HARNESS: vectors shuffled between titles; purity must collapse.\n");
    }
    let modes = sinephile_taste::modes::modes(&liked);
    let mut ok = modes.len() >= 2;
    let mut homes: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut weighted_purity = 0.0;
    for (i, m) in modes.iter().enumerate() {
        let h = m.members.iter().filter(|id| taste[*id] == "horror").count();
        let d = m.members.len() - h;
        let purity = h.max(d) as f64 / m.members.len().max(1) as f64;
        weighted_purity += purity * m.members.len() as f64 / liked.len() as f64;
        let major = if h >= d { "horror" } else { "documentary" };
        homes.entry(major).or_default().push(i);
        println!(
            "  mode {} ({:.0}% of the weight): {h} horror, {d} documentary, purity {:.0}%",
            i + 1,
            100.0 * m.weight,
            100.0 * purity
        );
    }
    ok &= homes.contains_key("horror") && homes.contains_key("documentary");
    ok &= weighted_purity >= 0.9;
    println!(
        "\n  {} modes from {} titles with vectors; weighted purity {:.1}% (target at least 90%); each taste has modes of its own: {}",
        modes.len(),
        liked.len(),
        100.0 * weighted_purity,
        homes.contains_key("horror") && homes.contains_key("documentary")
    );
    println!("\n  E1 {}", if ok { "MET" } else { "NOT MET" });
    Ok(ok)
}
