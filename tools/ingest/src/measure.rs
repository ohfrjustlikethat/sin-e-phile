//! Measure the shape of the IMDb catalogue before committing to it.
//!
//! `SPEC.md` R4 — *"catalogue ingestion is far larger or slower than expected"* — is
//! this phase's named risk, and its mitigation is explicit: **measure in Phase 4
//! before committing to a shape, and scope by a vote/popularity threshold rather
//! than ingesting everything.**
//!
//! So this runs before the loader exists. It downloads the two smallest datasets
//! that determine scoping, scans them without writing anything, and reports how many
//! titles survive each candidate threshold. The threshold is then chosen from that
//! table rather than from an intuition about how many films there are.
//!
//! It writes nothing to the database on purpose. A measurement that mutates state
//! cannot be re-run to check itself.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use crate::download::{verify_gzip, Downloader};
use crate::imdb::{self, Dataset, Scope, VoteHistogram, KEPT_TYPES};
use crate::job::JobError;
use crate::tsv::TsvReader;

/// What the scan found.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub titles_total: u64,
    /// Rows whose `titleType` is one this catalogue would keep.
    pub titles_kept_type: u64,
    pub by_type: Vec<(String, u64)>,
    pub histogram: VoteHistogram,
    /// Mean bytes of the fields that would be stored, over kept titles. Used to
    /// project database size honestly rather than by guessing a row width.
    pub mean_row_bytes: f64,
    pub download_seconds: f64,
    pub scan_seconds: f64,
    pub compressed_bytes: u64,
    pub decompressed_bytes: u64,
}

impl Measurement {
    /// Titles kept at a candidate threshold.
    pub fn kept_at(&self, scope: Scope) -> u64 {
        let rated = self.histogram.at_least(scope.min_votes);
        let unrated = if scope.keep_unrated {
            self.histogram.unrated
        } else {
            0
        };
        // Both counts are over rows of a kept TYPE already, so this is the answer
        // for the whole filter rather than for the vote clause alone.
        rated + unrated
    }

    /// A projected database size, stated as a projection.
    ///
    /// SQLite overhead per row plus the indexes in migration 0001 come to roughly
    /// 2.4x the raw field bytes, measured against the Phase 3 benchmark: 500,000
    /// synthetic rows produced a 145.4 MB database. That ratio is the honest
    /// multiplier, and it is recorded here rather than hidden in a constant.
    pub fn projected_bytes(&self, kept: u64) -> u64 {
        const SQLITE_OVERHEAD: f64 = 2.4;
        (kept as f64 * self.mean_row_bytes * SQLITE_OVERHEAD) as u64
    }
}

/// `tt0000001` → `1`. Storing the numeric part turns a 1.5-million-entry lookup
/// from roughly 90 MB of `String` keys into 12 MB of integers, which matters
/// because the real loader needs the same map.
fn tconst_id(raw: &str) -> Option<u32> {
    raw.strip_prefix("tt")?.parse().ok()
}

async fn ensure(
    downloader: &Downloader,
    dataset: &Dataset,
    dir: &Path,
) -> Result<(u64, f64), JobError> {
    let path = dir.join(dataset.filename);
    let mut last_report = 0u64;

    let result = downloader
        .fetch(&dataset.url(), &path, |progress| {
            // One line per 25 MB. Enough to see it moving, quiet enough to leave
            // running.
            if progress.downloaded - last_report > 25 << 20 {
                last_report = progress.downloaded;
                match progress.percent() {
                    Some(pct) => tracing::info!(
                        "  {} {:.0}% ({:.0} MB)",
                        dataset.name,
                        pct,
                        progress.downloaded as f64 / 1_048_576.0
                    ),
                    None => tracing::info!(
                        "  {} {:.0} MB",
                        dataset.name,
                        progress.downloaded as f64 / 1_048_576.0
                    ),
                }
            }
        })
        .await?;

    if result.fetched {
        tracing::info!(
            "{}: {:.1} MB in {:.1}s ({:.1} MB/s)",
            dataset.name,
            result.bytes as f64 / 1_048_576.0,
            result.elapsed.as_secs_f64(),
            result.bytes_per_second() / 1_048_576.0
        );
    } else {
        tracing::info!(
            "{}: already have {:.1} MB",
            dataset.name,
            result.bytes as f64 / 1_048_576.0
        );
    }

    Ok((result.bytes, result.elapsed.as_secs_f64()))
}

/// Download what is needed and scan it. Writes nothing to the database.
pub async fn run(dir: &Path) -> Result<Measurement, JobError> {
    let downloader = Downloader::new();
    std::fs::create_dir_all(dir)?;

    // Ratings first: it is small, and it is what the threshold is computed from.
    let (ratings_bytes, ratings_seconds) = ensure(&downloader, &imdb::TITLE_RATINGS, dir).await?;
    let (basics_bytes, basics_seconds) = ensure(&downloader, &imdb::TITLE_BASICS, dir).await?;

    let ratings_path = dir.join(imdb::TITLE_RATINGS.filename);
    let basics_path = dir.join(imdb::TITLE_BASICS.filename);

    tracing::info!("verifying the archives decompress cleanly");
    let decompressed = verify_gzip(&ratings_path)? + verify_gzip(&basics_path)?;

    let scan_started = Instant::now();

    // Votes by title id.
    tracing::info!("scanning {}", imdb::TITLE_RATINGS.name);
    let mut votes: HashMap<u32, i64> = HashMap::with_capacity(1_600_000);
    let mut reader = TsvReader::open(&ratings_path)?;
    imdb::check_columns(&reader, &imdb::TITLE_RATINGS)?;
    while reader.advance()? {
        let Some(row) = reader.current_row() else {
            continue;
        };
        let (Some(id), Some(count)) = (
            row.get("tconst").and_then(tconst_id),
            row.parse::<i64>("numVotes"),
        ) else {
            continue;
        };
        votes.insert(id, count);
    }
    tracing::info!("  {} rated titles", votes.len());

    // Titles, joined against those votes.
    tracing::info!("scanning {}", imdb::TITLE_BASICS.name);
    let mut reader = TsvReader::open(&basics_path)?;
    imdb::check_columns(&reader, &imdb::TITLE_BASICS)?;

    let mut by_type: HashMap<String, u64> = HashMap::new();
    let mut histogram = VoteHistogram::new();
    let mut titles_total = 0u64;
    let mut titles_kept_type = 0u64;
    let mut field_bytes = 0u64;

    while reader.advance()? {
        let Some(row) = reader.current_row() else {
            continue;
        };
        titles_total += 1;

        let Some(title_type) = row.get("titleType") else {
            continue;
        };
        *by_type.entry(title_type.to_string()).or_default() += 1;

        if !KEPT_TYPES.contains(&title_type) {
            continue;
        }
        titles_kept_type += 1;

        let id = row.get("tconst").and_then(tconst_id);
        histogram.add(id.and_then(|id| votes.get(&id)).copied());

        // What would actually be stored, so the size projection is grounded.
        field_bytes += ["primaryTitle", "originalTitle", "genres"]
            .iter()
            .filter_map(|c| row.get(c))
            .map(|v| v.len() as u64)
            .sum::<u64>()
            + 24; // the fixed numeric columns
    }

    let mut by_type: Vec<(String, u64)> = by_type.into_iter().collect();
    by_type.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

    Ok(Measurement {
        titles_total,
        titles_kept_type,
        by_type,
        histogram,
        mean_row_bytes: if titles_kept_type > 0 {
            field_bytes as f64 / titles_kept_type as f64
        } else {
            0.0
        },
        download_seconds: ratings_seconds + basics_seconds,
        scan_seconds: scan_started.elapsed().as_secs_f64(),
        compressed_bytes: ratings_bytes + basics_bytes,
        decompressed_bytes: decompressed,
    })
}

/// Print the table the threshold gets chosen from.
pub fn report(m: &Measurement) {
    println!();
    println!("  IMDb catalogue shape — measured, not estimated (R4)");
    println!();
    println!("  titles in title.basics      {:>12}", m.titles_total);
    println!("  of a type we would keep     {:>12}", m.titles_kept_type);
    println!(
        "  archives                    {:>9.1} MB compressed, {:.1} MB raw",
        m.compressed_bytes as f64 / 1_048_576.0,
        m.decompressed_bytes as f64 / 1_048_576.0
    );
    println!(
        "  download {:.0}s · scan {:.0}s",
        m.download_seconds, m.scan_seconds
    );

    println!();
    println!("  by title type");
    for (name, count) in m.by_type.iter().take(10) {
        let kept = if KEPT_TYPES.contains(&name.as_str()) {
            "keep"
        } else {
            "drop"
        };
        println!("    {kept}  {name:<16} {count:>10}");
    }

    println!();
    println!("  votes threshold      titles kept    projected database");
    for threshold in [0i64, 10, 50, 100, 500, 1_000, 5_000, 10_000] {
        let scope = Scope {
            min_votes: threshold,
            keep_unrated: false,
            keep_adult: false,
        };
        let kept = m.kept_at(scope);
        println!(
            "    >= {threshold:<8}         {kept:>10}    {:>8.0} MB",
            m.projected_bytes(kept) as f64 / 1_048_576.0
        );
    }
    let unrated_kept = m.kept_at(Scope {
        min_votes: 0,
        keep_unrated: true,
        keep_adult: false,
    });
    println!(
        "    (+ {} unrated titles, which includes new releases)",
        m.histogram.unrated
    );
    println!(
        "    everything of a kept type: {unrated_kept} titles, {:.0} MB projected",
        m.projected_bytes(unrated_kept) as f64 / 1_048_576.0
    );

    println!();
    println!("  R4 triggers: ingestion over 2 hours, or a database over 4 GB.");
    println!("  Projections assume 2.4x SQLite overhead, the ratio measured in Phase 3");
    println!("  (500,000 synthetic rows produced a 145.4 MB database).");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tconst_parses_to_its_numeric_part() {
        assert_eq!(tconst_id("tt0000001"), Some(1));
        assert_eq!(tconst_id("tt12345678"), Some(12_345_678));
        assert_eq!(tconst_id("nm0000001"), None, "a person is not a title");
        assert_eq!(tconst_id("rubbish"), None);
    }

    #[test]
    fn kept_at_counts_rated_and_unrated_according_to_the_scope() {
        let mut histogram = VoteHistogram::new();
        for votes in [5, 200, 200, 20_000] {
            histogram.add(Some(votes));
        }
        histogram.add(None);
        histogram.add(None);

        let m = Measurement {
            titles_total: 6,
            titles_kept_type: 6,
            by_type: vec![],
            histogram,
            mean_row_bytes: 100.0,
            download_seconds: 0.0,
            scan_seconds: 0.0,
            compressed_bytes: 0,
            decompressed_bytes: 0,
        };

        assert_eq!(
            m.kept_at(Scope {
                min_votes: 100,
                keep_unrated: false,
                keep_adult: false
            }),
            3
        );
        assert_eq!(
            m.kept_at(Scope {
                min_votes: 100,
                keep_unrated: true,
                keep_adult: false
            }),
            5,
            "the two unrated titles are added"
        );
    }

    #[test]
    fn the_projection_is_grounded_in_the_phase_3_measurement() {
        let m = Measurement {
            titles_total: 0,
            titles_kept_type: 0,
            by_type: vec![],
            histogram: VoteHistogram::new(),
            mean_row_bytes: 100.0,
            download_seconds: 0.0,
            scan_seconds: 0.0,
            compressed_bytes: 0,
            decompressed_bytes: 0,
        };
        // 500,000 rows x 100 bytes x 2.4 = 120 MB, the same order as the 145.4 MB
        // the Phase 3 benchmark actually produced.
        assert_eq!(m.projected_bytes(500_000), 120_000_000);
    }
}
