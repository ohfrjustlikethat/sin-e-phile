//! MovieLens: the collaborative-filtering signal (Phase 4, subtask 4.3).
//!
//! # What is stored, and what deliberately is not
//!
//! **The ratings are never stored.** ml-25m is 25 million of them, and R4's headroom
//! after episodes is 461 MB — they would not fit, and they do not need to. They are an
//! *input* to the item-item matrix, not catalogue data: Phase 16 streams them from the
//! archive on disk, writes a matrix, and the ratings are never read again.
//!
//! What Phase 4 stores is the **join**: one `external_ids` row per MovieLens film that
//! maps onto a catalogue item, via `links.csv`'s `imdbId` column. That is 60,000-odd
//! rows, it is what makes the matrix addressable by internal id, and nothing else in
//! the dataset is catalogue data at all.
//!
//! # Why the matrix is not built here
//!
//! ADR-0019: GroupLens does not generally permit redistribution, so a derived matrix
//! is never shipped — it is computed on the user's machine. Phase 4's job is to
//! **measure what that will cost** before Phase 16 commits to a dataset size, because
//! the cost lands on first run and lands hardest on Tier 0.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::time::Instant;

use sinephile_persistence::Db;

use crate::job::{Batch, Job, JobError, SqliteTx};

/// GroupLens' download host. Covered by `grouplens.org` in the guard allowlist, which
/// matches subdomains.
const HOST: &str = "https://files.grouplens.org/datasets/movielens";

/// Which MovieLens release to ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    /// 25 million ratings. **The default**, and stable: GroupLens versions it and does
    /// not change it, so an eval number computed against it today is reproducible next
    /// year. `ml-latest` is explicitly documented as changing over time and as
    /// unsuitable for reporting results, which makes it the wrong choice for a project
    /// whose Phase 16 exit criteria are measured numbers.
    Ml25m,
    /// 100,000 ratings. For tests, and for Tier 0 if the full set proves too slow —
    /// the mitigation ADR-0019 names.
    Small,
}

impl Release {
    pub fn name(self) -> &'static str {
        match self {
            Release::Ml25m => "ml-25m",
            Release::Small => "ml-latest-small",
        }
    }

    pub fn filename(self) -> String {
        format!("{}.zip", self.name())
    }

    pub fn url(self) -> String {
        format!("{HOST}/{}", self.filename())
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "ml-25m" | "25m" => Some(Release::Ml25m),
            "ml-latest-small" | "small" => Some(Release::Small),
            _ => None,
        }
    }
}

/// The MD5 GroupLens publishes for `ml-25m.zip`.
///
/// **Read from `grouplens.org`, which has a VALID certificate, for a file served from
/// `files.grouplens.org`, which does not** (expired 2026-08-28, still expired on
/// 2026-09-06). That asymmetry is the whole point: the transport carrying the archive
/// cannot currently be authenticated, but the checksum describing it can be. A file
/// fetched by hand over the broken link and verified against this is as trustworthy as
/// one fetched over a working one — and considerably more trustworthy than disabling
/// certificate verification in the application, which would degrade every future
/// download for every user because a university let a certificate lapse.
pub const ML25M_MD5: &str = "544c4d86ea9f05e056d8075398539b34";

/// Verify a manually-placed archive.
///
/// MD5 because that is what GroupLens publishes. It is not a security hash and is not
/// used as one: the guarantee here is integrity against a truncated or corrupted
/// download, and the authenticity comes from where the checksum was read, not from the
/// algorithm.
pub fn verify_md5(path: &Path, expected: &str) -> Result<(), JobError> {
    use md5::{Digest, Md5};
    let bytes = std::fs::read(path)
        .map_err(|e| JobError::step("movielens", format!("{}: {e}", path.display())))?;
    let actual: String = Md5::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        return Err(JobError::step(
            "movielens",
            format!(
                "{} has md5 {actual}, expected {expected} — the download is incomplete or                  is not the file GroupLens published",
                path.display()
            ),
        ));
    }
    Ok(())
}

/// One row of `links.csv`.
#[derive(Debug, Clone, Copy)]
pub struct Link {
    pub movielens_id: i64,
    /// Numeric part of the IMDb id. `links.csv` stores it WITHOUT the `tt` prefix and
    /// without zero padding, so `0114709` arrives as `114709`.
    pub imdb_id: u32,
}

/// What the join actually achieved.
#[derive(Debug, Default, Clone)]
pub struct Measurement {
    pub links: i64,
    pub matched: i64,
    pub unmatched: i64,
    pub ratings: i64,
    /// Seconds to stream every rating. This is the floor on what the on-device matrix
    /// costs (ADR-0019), and the number R4's two-hour trigger is checked against.
    pub ratings_scan_seconds: f64,
}

impl Measurement {
    pub fn report(&self, release: Release) {
        println!();
        println!("  {} ", release.name());
        println!("    {:>9}  films in links.csv", self.links);
        println!(
            "    {:>9}  matched onto the catalogue ({:.1}%)",
            self.matched,
            if self.links == 0 {
                0.0
            } else {
                100.0 * self.matched as f64 / self.links as f64
            }
        );
        println!("    {:>9}  not in the catalogue", self.unmatched);
        println!(
            "    {:>9}  ratings, streamed in {:.0}s",
            self.ratings, self.ratings_scan_seconds
        );
        println!();
    }
}

/// Open one file inside the archive by its suffix.
///
/// MovieLens nests everything under a directory named after the release, so entries
/// are `ml-25m/links.csv` rather than `links.csv`. Matching on the suffix means the
/// directory name is not a second thing to keep in sync with the release enum.
fn open_entry<'a>(
    archive: &'a mut zip::ZipArchive<std::fs::File>,
    suffix: &str,
) -> Result<zip::read::ZipFile<'a, std::fs::File>, JobError> {
    let index = (0..archive.len())
        .find(|i| {
            archive
                .name_for_index(*i)
                .is_some_and(|name| name.ends_with(suffix))
        })
        .ok_or_else(|| JobError::step("movielens", format!("no {suffix} in the archive")))?;
    archive
        .by_index(index)
        .map_err(|e| JobError::step("movielens", format!("{suffix}: {e}")))
}

fn archive(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, JobError> {
    let file = std::fs::File::open(path)
        .map_err(|e| JobError::step("movielens", format!("{}: {e}", path.display())))?;
    zip::ZipArchive::new(file).map_err(|e| JobError::step("movielens", e.to_string()))
}

/// Read `links.csv`.
pub fn links(path: &Path) -> Result<Vec<Link>, JobError> {
    let mut zip = archive(path)?;
    let entry = open_entry(&mut zip, "links.csv")?;
    let mut reader = BufReader::new(entry);

    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|e| JobError::step("movielens", e.to_string()))?;
    if !header.starts_with("movieId,imdbId,tmdbId") {
        return Err(JobError::step(
            "movielens",
            format!("links.csv header changed: {}", header.trim()),
        ));
    }

    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| JobError::step("movielens", e.to_string()))?;
        let mut fields = line.split(',');
        let (Some(movielens), Some(imdb)) = (fields.next(), fields.next()) else {
            continue;
        };
        // A blank imdbId is a real thing in links.csv, and a film we cannot join is
        // simply skipped — there is nothing to guess at.
        let (Ok(movielens_id), Ok(imdb_id)) = (movielens.parse::<i64>(), imdb.parse::<u32>())
        else {
            continue;
        };
        out.push(Link {
            movielens_id,
            imdb_id,
        });
    }
    Ok(out)
}

/// Count the ratings, and time the read.
///
/// The timing is the deliverable, not the count: ADR-0019 puts the item-item matrix on
/// the user's machine and requires Phase 4 to measure the cost before Phase 16 commits
/// to a dataset size. Streaming the file is the floor on that cost.
pub fn scan_ratings(path: &Path) -> Result<(i64, f64), JobError> {
    let started = Instant::now();
    let mut zip = archive(path)?;
    let entry = open_entry(&mut zip, "ratings.csv")?;

    // Counting newlines over a large buffer rather than allocating a String per line:
    // 25 million allocations would measure the allocator, not the file.
    let mut reader = BufReader::with_capacity(1 << 20, entry);
    let mut buf = vec![0u8; 1 << 20];
    let mut lines: i64 = 0;
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|e| JobError::step("movielens", e.to_string()))?;
        if read == 0 {
            break;
        }
        lines += buf[..read].iter().filter(|b| **b == b'\n').count() as i64;
    }
    // Minus the header.
    Ok(((lines - 1).max(0), started.elapsed().as_secs_f64()))
}

/// Catalogue items by the numeric part of their IMDb id.
pub async fn catalogue_by_imdb(db: &Db) -> Result<HashMap<u32, i64>, JobError> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT external_id, media_item_id FROM external_ids WHERE source = 'imdb'")
            .fetch_all(db.pool())
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(tconst, id)| {
            tconst
                .strip_prefix("tt")
                .and_then(|n| n.parse::<u32>().ok())
                .map(|n| (n, id))
        })
        .collect())
}

/// Write the MovieLens ids for films we hold.
pub async fn load(
    job: &mut Job<'_>,
    links: std::sync::Arc<Vec<(i64, i64)>>,
) -> Result<(), JobError> {
    job.run_step("movielens.links", move |tx, cursor| {
        let links = std::sync::Arc::clone(&links);
        Box::pin(async move {
            let from: usize = cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
            if from >= links.len() {
                return Ok(Batch::finished(0));
            }
            let end = (from + 5_000).min(links.len());
            insert(tx, &links[from..end]).await?;

            Ok(if end >= links.len() {
                Batch::finished((end - from) as i64)
            } else {
                Batch::more(end.to_string(), (end - from) as i64)
            })
        })
    })
    .await?;
    Ok(())
}

async fn insert(tx: &mut SqliteTx<'_>, pairs: &[(i64, i64)]) -> Result<(), JobError> {
    for chunk in pairs.chunks(200) {
        // ON CONFLICT DO NOTHING on BOTH keys: (media_item_id, source) is the primary
        // key and (source, external_id) is unique, so two MovieLens ids pointing at one
        // catalogue item — which happens where IMDb merged two entries — must not fail
        // the batch. The first one wins, consistently, because links.csv is ordered.
        let sql = format!(
            "INSERT INTO external_ids (media_item_id, source, external_id)
             VALUES {} ON CONFLICT DO NOTHING",
            vec!["(?, 'movielens', ?)"; chunk.len()].join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (media_item_id, movielens_id) in chunk {
            query = query.bind(media_item_id).bind(movielens_id.to_string());
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| JobError::step("movielens.links", e.to_string()))?;
    }
    Ok(())
}

/// How many catalogue items carry a MovieLens id.
pub async fn mapped(db: &Db) -> Result<i64, JobError> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM external_ids WHERE source = 'movielens'")
            .fetch_one(db.pool())
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_resolve_to_grouplens_urls() {
        assert_eq!(
            Release::Ml25m.url(),
            "https://files.grouplens.org/datasets/movielens/ml-25m.zip"
        );
        assert!(Release::Small.url().ends_with("ml-latest-small.zip"));
        assert_eq!(Release::parse("25m"), Some(Release::Ml25m));
        assert_eq!(Release::parse("nonsense"), None);
    }
}
