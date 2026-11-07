//! `eval scan`: Phase 12 E3 — scanning 10,000 files completes in under three minutes and
//! does not block the UI.
//!
//! A worst case for the scanner's shared searches: N *distinct* films sampled from the real
//! catalogue (popular enough to be in a real library), each written as an empty-ish file
//! with a scene-style name, one folder per film. Then a real scan with the real FFprobe,
//! timed, while a second task keeps issuing the queries a page issues and records their
//! latency. Because each file's truth is known, accuracy over the sample is measured too.
//!
//! Everything it adds to the catalogue's database (one library folder and its files) is
//! removed at the end.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use sinephile_library::probe::ffprobe;
use sinephile_library::scan::scan;
use sinephile_persistence::repositories::{LibraryRepository, SearchRepository};
use sinephile_persistence::Db;

use crate::error::EvalError;

fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}

fn p95(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
}

/// The queries a page issues while the scan runs, timed.
async fn page_queries(db: Db, stop: Arc<AtomicBool>) -> Vec<f64> {
    let search = SearchRepository::new(&db);
    let mut times = Vec::new();
    let probes = [
        "heat",
        "the godfather",
        "spirited away",
        "breaking bad",
        "amelie",
    ];
    let mut i = 0;
    while !stop.load(Ordering::Relaxed) {
        let t = Instant::now();
        let _ = search.exact_title(probes[i % probes.len()], 20).await;
        let _ = search.keyword(probes[i % probes.len()], 20).await;
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        i += 1;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    times
}

pub async fn run(data_dir: &Path, n: usize) -> Result<bool, EvalError> {
    let db = Db::open_in(data_dir).await?;
    println!("scan — {n} distinct films from the catalogue, scanned with the real FFprobe\n");

    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT id, primary_title, release_year FROM media_items
         WHERE kind = 'film' AND release_year IS NOT NULL AND rating_votes >= 500
         ORDER BY random() LIMIT ?",
    )
    .bind(n as i64)
    .fetch_all(db.pool())
    .await?;

    let root = std::env::temp_dir().join("sinephile-eval-scan");
    let _ = std::fs::remove_dir_all(&root);
    let made = Instant::now();
    let mut truth = std::collections::HashMap::new();
    for (i, (id, title, year)) in rows.iter().enumerate() {
        let t = safe(title);
        if t.is_empty() {
            continue;
        }
        let dir = root.join(format!("{t} ({year})"));
        let name = format!("{}.{year}.1080p.BluRay.x264-GROUP.mkv", t.replace(' ', "."));
        std::fs::create_dir_all(&dir).map_err(|e| EvalError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let path = dir.join(&name);
        // Distinct sizes, as real files have.
        std::fs::write(&path, vec![0u8; 1024 + i]).map_err(|e| EvalError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        truth.insert(path.to_string_lossy().into_owned(), *id);
    }
    println!(
        "  wrote {} files in {:.1} s",
        truth.len(),
        made.elapsed().as_secs_f64()
    );

    let repo = LibraryRepository::new(&db);
    let folder = repo.add_folder(&root.to_string_lossy()).await?;

    // The page's queries, timed alone, then during the scan.
    let stop = Arc::new(AtomicBool::new(false));
    let idle_task = tokio::spawn(page_queries(db.clone(), stop.clone()));
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    stop.store(true, Ordering::Relaxed);
    let idle = idle_task.await.unwrap_or_default();

    let stop = Arc::new(AtomicBool::new(false));
    let busy_task = tokio::spawn(page_queries(db.clone(), stop.clone()));
    let started = Instant::now();
    let report = scan(&db, folder, &root, ffprobe, &|_| {}).await?;
    let elapsed = started.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let busy = busy_task.await.unwrap_or_default();

    // Accuracy over the sample.
    let mut right_accepted = 0;
    let mut wrong_accepted = 0;
    let mut right_best = 0;
    let stored = repo.files_in_folder(folder).await?;
    let seen: std::collections::HashSet<&str> = stored.iter().map(|f| f.path.as_str()).collect();
    for path in truth.keys().filter(|p| !seen.contains(p.as_str())).take(10) {
        println!("  written but not scanned: {path}");
    }
    for f in stored {
        let Some(want) = truth.get(&f.path) else {
            continue;
        };
        if let Some(m) = repo.match_of(f.id).await? {
            let right = m.media_item_id == *want;
            right_best += usize::from(right);
            if m.confidence >= sinephile_library::ACCEPT_AT {
                if right {
                    right_accepted += 1;
                } else {
                    wrong_accepted += 1;
                }
            }
        }
    }

    repo.remove_folder(folder).await?;
    let _ = std::fs::remove_dir_all(&root);

    let files = report.files.max(1) as f64;
    println!(
        "  scanned {} files in {elapsed:.1} s ({:.1} ms a file): {} accepted, {} asked, {} probed",
        report.files,
        1000.0 * elapsed / files,
        report.accepted,
        report.asked,
        report.probed
    );
    println!(
        "  E3 target: 10,000 files in under 180 s -> {}",
        if elapsed * 10_000.0 / files < 180.0 {
            "met at this rate"
        } else {
            "NOT MET"
        }
    );
    println!(
        "  a page's queries, p95: {:.1} ms idle ({} runs), {:.1} ms during the scan ({} runs)",
        p95(idle.clone()),
        idle.len(),
        p95(busy.clone()),
        busy.len()
    );
    let accepted = right_accepted + wrong_accepted;
    println!(
        "  accuracy on the sample: best right {right_best}/{} = {:.2}%; accepted wrong {wrong_accepted}/{accepted} = {:.2}%",
        truth.len(),
        100.0 * right_best as f64 / truth.len().max(1) as f64,
        100.0 * wrong_accepted as f64 / accepted.max(1) as f64
    );
    Ok(elapsed * 10_000.0 / files < 180.0)
}
