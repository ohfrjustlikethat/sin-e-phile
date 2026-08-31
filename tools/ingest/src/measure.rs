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

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use crate::download::{verify_gzip, Downloader};
use crate::imdb::{self, CatalogueScope, Dataset, Scope, VoteHistogram, KEPT_TYPES};
use crate::job::JobError;
use crate::tsv::TsvReader;

/// What the scan found.
///
/// `Default` so a test can set the two or three fields it cares about. Adding a
/// field to this struct should not mean editing every test that constructs one.
#[derive(Debug, Clone, Default)]
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

    // --- the two tiers, under CatalogueScope::DEFAULT ---
    /// Titles entering the catalogue at all.
    pub index_titles: u64,
    /// Titles additionally getting cast, crew, akas and embeddings.
    pub core_titles: u64,

    // --- what the core tier actually costs, which is the R4 question ---
    /// Every row in `title.principals`, whatever it points at.
    pub principals_total: u64,
    /// Rows pointing at a CORE title — the ones that would be stored.
    pub principals_for_core: u64,
    pub akas_total: u64,
    pub akas_for_core: u64,
    /// Whether the two large datasets were scanned. `--quick` skips them.
    pub deep: bool,
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
    /// # This projection was wrong once, by roughly half
    ///
    /// The first version used 2.4x the `media_items` field bytes — the ratio from
    /// the Phase 3 benchmark, where 500,000 synthetic rows produced a 145.4 MB
    /// database. It projected 450 MB for the index tier. **The real load passed
    /// 900 MB.**
    ///
    /// The Phase 3 benchmark inserted into `media_items` and `titles` and nothing
    /// else. A real load also writes `external_ids` (with a UNIQUE index),
    /// `media_genres`, and a second `titles` row wherever the original title differs
    /// — none of which the 2.4x captured, because none of them existed in the
    /// fixture it was derived from.
    ///
    /// The lesson is narrow and worth keeping: **a multiplier is only valid for the
    /// shape it was measured on.** Reusing it across a different set of tables is
    /// not a projection, it is a guess wearing a number's clothes.
    ///
    /// `ROW_MULTIPLIER` is now derived from the real load recorded in
    /// `docs/eval-results.md`, and a projection is labelled as such wherever it is
    /// printed.
    pub fn projected_bytes(&self, kept: u64) -> u64 {
        // Measured: the real index-tier load — 2,701,195 titles producing a
        // 1,153 MB database — divided by the field bytes counted here. Covers
        // media_items, titles, external_ids, media_genres and every index on them.
        // The projection it replaces said 450 MB, so it was 2.56x low.
        const ROW_MULTIPLIER: f64 = 6.2;
        (kept as f64 * self.mean_row_bytes * ROW_MULTIPLIER) as u64
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
pub async fn run(dir: &Path, deep: bool) -> Result<Measurement, JobError> {
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

    // The two tiers, and the core ids the expensive datasets get counted against.
    let scope = CatalogueScope::DEFAULT;
    let mut index_titles = 0u64;
    let mut core_titles = 0u64;
    let mut core_ids: HashSet<u32> = HashSet::with_capacity(300_000);

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
        let title_votes = id.and_then(|id| votes.get(&id)).copied();
        histogram.add(title_votes);

        let adult = imdb::is_adult(row.get("isAdult"));
        let year = row.parse::<i64>("startYear");
        if scope.in_index(title_type, title_votes, adult) {
            index_titles += 1;
        }
        if scope.in_core(title_type, title_votes, adult, year) {
            core_titles += 1;
            if let Some(id) = id {
                core_ids.insert(id);
            }
        }

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

    // The datasets that actually carry the R4 risk. Both scale with the number of
    // titles kept, which is the whole reason the core tier exists — so counting
    // their rows against the core set is the measurement that decides whether the
    // two-tier design works.
    let mut principals_total = 0u64;
    let mut principals_for_core = 0u64;
    let mut akas_total = 0u64;
    let mut akas_for_core = 0u64;

    if deep {
        for (dataset, id_col, total, for_core) in [
            (
                &imdb::TITLE_PRINCIPALS,
                "tconst",
                &mut principals_total,
                &mut principals_for_core,
            ),
            (
                &imdb::TITLE_AKAS,
                "titleId",
                &mut akas_total,
                &mut akas_for_core,
            ),
        ] {
            ensure(&downloader, dataset, dir).await?;
            tracing::info!("scanning {}", dataset.name);
            let mut reader = TsvReader::open(&dir.join(dataset.filename))?;
            while reader.advance()? {
                let Some(row) = reader.current_row() else {
                    continue;
                };
                *total += 1;
                if row
                    .get(id_col)
                    .and_then(tconst_id)
                    .is_some_and(|id| core_ids.contains(&id))
                {
                    *for_core += 1;
                }
            }
            tracing::info!("  {} rows, {} for core titles", total, for_core);
        }
    }

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
        index_titles,
        core_titles,
        principals_total,
        principals_for_core,
        akas_total,
        akas_for_core,
        deep,
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
            unrated_recent_years: None,
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
        unrated_recent_years: None,
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
    println!("  THE SHIPPED TWO-TIER SCOPE (CatalogueScope::DEFAULT)");
    println!(
        "    index — everything of a kept type, non-adult:  {:>10}",
        m.index_titles
    );
    println!(
        "    core  — >=10 votes, or unrated from <=2y ago:  {:>10}",
        m.core_titles
    );
    println!(
        "    the index projects to {:.0} MB of media_items",
        m.projected_bytes(m.index_titles) as f64 / 1_048_576.0
    );

    if m.deep {
        println!();
        println!("  WHAT THE CORE TIER SAVES — this is the R4 question");
        let pct = |part: u64, whole: u64| {
            if whole == 0 {
                0.0
            } else {
                part as f64 * 100.0 / whole as f64
            }
        };
        println!(
            "    title.principals   {:>11} rows total, {:>10} for core ({:.1}%)",
            m.principals_total,
            m.principals_for_core,
            pct(m.principals_for_core, m.principals_total)
        );
        println!(
            "    title.akas         {:>11} rows total, {:>10} for core ({:.1}%)",
            m.akas_total,
            m.akas_for_core,
            pct(m.akas_for_core, m.akas_total)
        );
        // ~80 bytes per credit row and ~60 per aka, times the same 2.4x overhead.
        let credits = (m.principals_for_core as f64 * 80.0 * 2.4) as u64;
        let akas = (m.akas_for_core as f64 * 60.0 * 2.4) as u64;
        let titles = m.projected_bytes(m.index_titles);
        println!(
            "    projected: {:.0} MB titles + {:.0} MB credits + {:.0} MB akas = {:.2} GB",
            titles as f64 / 1_048_576.0,
            credits as f64 / 1_048_576.0,
            akas as f64 / 1_048_576.0,
            (titles + credits + akas) as f64 / 1_073_741_824.0
        );
        let unscoped = (m.principals_total as f64 * 80.0 * 2.4) as u64
            + (m.akas_total as f64 * 60.0 * 2.4) as u64
            + titles;
        println!(
            "    without the core tier it would be {:.2} GB",
            unscoped as f64 / 1_073_741_824.0
        );
    } else {
        println!();
        println!("  (--quick: title.principals and title.akas were NOT scanned, so this");
        println!("   says nothing about the sizes that actually carry R4.)");
    }

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
            ..Default::default()
        };

        assert_eq!(
            m.kept_at(Scope {
                min_votes: 100,
                keep_unrated: false,
                unrated_recent_years: None,
                keep_adult: false
            }),
            3
        );
        assert_eq!(
            m.kept_at(Scope {
                min_votes: 100,
                keep_unrated: true,
                unrated_recent_years: None,
                keep_adult: false
            }),
            5,
            "the two unrated titles are added"
        );
    }

    #[test]
    fn the_projection_uses_the_multiplier_measured_on_a_real_load() {
        // Was 2.4, taken from the Phase 3 benchmark — which only ever inserted into
        // media_items and titles. A real load also writes external_ids with its
        // UNIQUE index, media_genres, and a second titles row where the original
        // name differs, and it came in at roughly twice the projection.
        //
        // A multiplier is only valid for the shape it was measured on.
        let m = Measurement {
            mean_row_bytes: 100.0,
            ..Default::default()
        };
        assert_eq!(m.projected_bytes(1_000_000), 620_000_000);
        assert!(
            m.projected_bytes(1_000_000) > (1_000_000.0 * 100.0 * 2.4) as u64,
            "the corrected multiplier must not be smaller than the one it replaced"
        );
    }
}
