//! The Phase 3 performance criterion (E3).
//!
//! "A database populated with 500,000 synthetic media items answers **indexed
//! lookups in under 100 ms**. (The 100 ms budget is the lookup alone; bulk
//! insertion of the 500,000 rows has no time budget and is measured and recorded
//! separately in `docs/PERFORMANCE.md`.)" — `SPEC.md` Phase 3, amendment 15.
//!
//! Ignored by default: it writes a multi-hundred-megabyte database and takes
//! minutes. Run it deliberately:
//!
//! ```text
//! cargo test -p sinephile-persistence --release --test benchmark -- --ignored --nocapture
//! ```
//!
//! Release mode matters. A debug build measures rustc's lack of optimisation
//! rather than SQLite's index, and reporting a debug number as the budget would be
//! the same dishonesty as timing cold start to the window handle instead of to the
//! first painted frame.

use std::time::{Duration, Instant};

use sinephile_persistence::repositories::MediaRepository;
use sinephile_persistence::{Db, IdSource, NewMediaItem};

const TARGET_ROWS: usize = 500_000;
const BATCH: usize = 10_000;
/// SPEC.md Phase 3. The lookup budget, and nothing else.
const LOOKUP_BUDGET: Duration = Duration::from_millis(100);

/// Deterministic pseudo-random, so a rerun measures the same database.
///
/// A real RNG would make the numbers unreproducible, and the point of recording a
/// measurement is that a later phase can compare against it.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        // Numerical Recipes constants. Adequate for spreading titles and years.
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 16
    }

    fn in_range(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const WORDS: [&str; 24] = [
    "Autumn",
    "Iron",
    "Silent",
    "Crimson",
    "Distant",
    "Hollow",
    "Northern",
    "Bitter",
    "Glass",
    "Winter",
    "Paper",
    "電車",
    "Midnight",
    "Salt",
    "Amber",
    "Quiet",
    "Broken",
    "Seventh",
    "Pale",
    "Восход",
    "Wandering",
    "Lantern",
    "Ash",
    "Marigold",
];

fn synthetic_title(rng: &mut Lcg, index: usize) -> String {
    let a = WORDS[rng.in_range(WORDS.len() as u64) as usize];
    let b = WORDS[rng.in_range(WORDS.len() as u64) as usize];
    // The index keeps titles unique, which is what makes an exact-title lookup a
    // fair test rather than a scan over thousands of duplicates.
    format!("{a} {b} {index}")
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
    sorted[idx]
}

/// Time `f` `iterations` times and return (p50, p95, p99, worst).
async fn measure<F, Fut>(iterations: usize, mut f: F) -> (Duration, Duration, Duration, Duration)
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut samples = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let start = Instant::now();
        f(i).await;
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    (
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        percentile(&samples, 0.99),
        *samples.last().expect("at least one sample"),
    )
}

#[tokio::test]
#[ignore = "writes ~500MB and takes minutes; run explicitly with --ignored"]
async fn indexed_lookups_over_500k_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("benchmark.db");
    let db = Db::open(&path).await.expect("open");
    let media = MediaRepository::new(&db);

    // ---- Bulk insert. No budget (amendment 15), but measured and recorded. ----
    let mut rng = Lcg(0x5EED_5EED);
    // Titles are kept as they are generated rather than regenerated later: the
    // generator is a sequence, so recovering row i's title by replay is O(i), and
    // sampling a thousand of them that way is half a billion iterations.
    let mut sampled_titles: Vec<String> = Vec::with_capacity(TARGET_ROWS / 500 + 1);
    let insert_start = Instant::now();

    for batch_start in (0..TARGET_ROWS).step_by(BATCH) {
        let batch: Vec<NewMediaItem> = (batch_start..(batch_start + BATCH).min(TARGET_ROWS))
            .map(|i| {
                let title = synthetic_title(&mut rng, i);
                if i % 500 == 0 {
                    sampled_titles.push(title.clone());
                }
                NewMediaItem {
                    primary_title: title,
                    release_year: Some(1900 + (rng.in_range(126) as i64)),
                    runtime_minutes: Some(60 + (rng.in_range(120) as i64)),
                    rating: Some(rng.in_range(101) as i64),
                    rating_votes: Some(rng.in_range(1_000_000) as i64),
                    ..NewMediaItem::film("", 0)
                }
            })
            .collect();

        media.insert_many(&batch).await.expect("bulk insert");
    }

    let insert_elapsed = insert_start.elapsed();
    let count = media.count().await.expect("count");
    assert_eq!(
        count, TARGET_ROWS as i64,
        "the fixture is the specified size"
    );

    // External ids on a sample, so by_external_id is measured against a populated
    // index rather than an empty one.
    for i in (0..TARGET_ROWS).step_by(500) {
        media
            .add_external_id(i as i64 + 1, IdSource::Imdb, &format!("tt{i:07}"), 1.0)
            .await
            .expect("external id");
    }

    // ANALYZE lets the query planner choose on statistics rather than heuristics.
    // Ingestion will run this too, so measuring without it would measure a
    // configuration the app never actually ships.
    sqlx::query("ANALYZE")
        .execute(db.pool())
        .await
        .expect("analyze");

    let db_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    // ---- The lookups the criterion is about. ----
    let mut rng = Lcg(0xC0FF_EE00);

    let ids: Vec<i64> = (0..1_000)
        .map(|_| rng.in_range(TARGET_ROWS as u64) as i64 + 1)
        .collect();
    let (id_p50, id_p95, id_p99, id_worst) = measure(ids.len(), |i| {
        let media = &media;
        let id = ids[i];
        async move {
            media.by_id(id).await.expect("by_id");
        }
    })
    .await;

    // Exact title: the lookup Phase 5's 100%-top-1 criterion depends on.
    let mut rng = Lcg(0xBEEF_0001);
    let titles: Vec<String> = (0..1_000)
        .map(|_| sampled_titles[rng.in_range(sampled_titles.len() as u64) as usize].clone())
        .collect();
    let (title_p50, title_p95, title_p99, title_worst) = measure(titles.len(), |i| {
        let media = &media;
        let title = titles[i].clone();
        async move {
            media.by_exact_title(&title).await.expect("by_exact_title");
        }
    })
    .await;

    let mut rng = Lcg(0xFACE_0002);
    let externals: Vec<String> = (0..1_000)
        .map(|_| format!("tt{:07}", rng.in_range((TARGET_ROWS / 500) as u64) * 500))
        .collect();
    let (ext_p50, ext_p95, ext_p99, ext_worst) = measure(externals.len(), |i| {
        let media = &media;
        let ext = externals[i].clone();
        async move {
            media
                .by_external_id(IdSource::Imdb, &ext)
                .await
                .expect("by_external_id");
        }
    })
    .await;

    println!();
    println!("  Phase 3 E3 — indexed lookup over {TARGET_ROWS} synthetic media items");
    println!("  database {:.1} MB", db_bytes as f64 / 1_048_576.0);
    println!(
        "  bulk insert {:.1} s ({:.0} rows/sec) — no budget, amendment 15",
        insert_elapsed.as_secs_f64(),
        TARGET_ROWS as f64 / insert_elapsed.as_secs_f64()
    );
    println!();
    println!("  lookup              p50        p95        p99      worst");
    for (name, p50, p95, p99, worst) in [
        ("by_id", id_p50, id_p95, id_p99, id_worst),
        (
            "by_exact_title",
            title_p50,
            title_p95,
            title_p99,
            title_worst,
        ),
        ("by_external_id", ext_p50, ext_p95, ext_p99, ext_worst),
    ] {
        println!(
            "  {name:<16} {:>7.3}ms {:>8.3}ms {:>8.3}ms {:>8.3}ms",
            p50.as_secs_f64() * 1000.0,
            p95.as_secs_f64() * 1000.0,
            p99.as_secs_f64() * 1000.0,
            worst.as_secs_f64() * 1000.0,
        );
    }
    println!();
    println!("  budget: {}ms per lookup", LOOKUP_BUDGET.as_millis());
    println!();

    // The criterion. p99 rather than p50, because "answers indexed lookups in
    // under 100 ms" is a promise about the slow ones too.
    for (name, p99) in [
        ("by_id", id_p99),
        ("by_exact_title", title_p99),
        ("by_external_id", ext_p99),
    ] {
        assert!(
            p99 < LOOKUP_BUDGET,
            "{name} p99 was {:.3}ms, over the {}ms budget",
            p99.as_secs_f64() * 1000.0,
            LOOKUP_BUDGET.as_millis()
        );
    }
}
