//! The first run's catalogue (Phase 16, the author's brief item 3), and the catch-up after
//! every refresh.
//!
//! A fresh install ran only the hourly refresh (titles, ratings, episodes); the steps the
//! development copy got by hand (`ingest akas`, `normalise`, `search-index`) never ran, so
//! its search index was empty: search found nothing, the library identified nothing (it
//! searches the catalogue), and Films and TV Shows stayed empty.
//!
//! - **Stage A** is what search and identification need: the normalised form of every title,
//!   then the word index and the core tier's typo index. Home waits for it.
//! - **Alternate titles** (romaji, native, English) follow in the background: 862 of the
//!   2,519 seconds the whole build took on a copy of the author's install
//!   (docs/eval-results.md), so putting them first kept Home closed fourteen minutes longer.
//! - **The catch-up** normalises and indexes what is missing. The index skips titles it
//!   already holds, so the catch-up first un-marks every title that has gained a name not
//!   yet normalised: that is how alternate titles, and the refresh's new titles, become
//!   searchable. Before it, the titles the weekly refresh added were never searchable at
//!   all (the author: "catalogues are not updated").
//!
//! Each step is a resumable job, done once, and skipped where it is already complete; the
//! `ingest` CLI's `akas`, `normalise` and `search-index --trigram-core` run the same calls
//! under the same job names, so a copy built with it is already ready.

use std::path::Path;
use std::sync::Arc;

use sinephile_persistence::Db;

use crate::{Downloader, Job, JobError};

/// Stage A's steps, by job name, in the order they run.
pub const STAGE_A: [&str; 2] = ["titles-normalise", "search-index"];

/// Alternate titles' job: after stage A, in the background.
pub const AKAS: &str = "imdb-akas";

/// The catch-up's job.
pub const CATCH_UP: &str = "catalogue-catch-up";

/// The jobs that load titles: the app's refresh, or `ingest imdb` for a development copy.
pub const LOAD: [&str; 2] = ["refresh", "imdb"];

/// Each part's share of the wait before search works, in seconds, as measured on a copy
/// of the author's install (docs/eval-results.md, 2026-11-17): the refresh's titles and
/// episodes, then stage A.
const SHARE: [(&str, f64); 3] = [
    ("load", 1407.0),
    ("titles-normalise", 123.0),
    ("search-index", 127.0),
];

/// What a step is called on screen.
pub fn label(step: &str) -> &'static str {
    match step {
        "load" => "Downloading the catalogue",
        "titles-normalise" => "Preparing titles",
        "search-index" => "Building search",
        AKAS => "Adding alternate titles",
        CATCH_UP => "Making new titles searchable",
        _ => "Getting ready",
    }
}

/// The id of the last finished job of this name.
async fn last_done(db: &Db, job: &str) -> Result<Option<i64>, JobError> {
    Ok(
        sqlx::query_scalar(
            "SELECT MAX(id) FROM ingest_jobs WHERE name = ? AND status = 'complete'",
        )
        .bind(job)
        .fetch_one(db.pool())
        .await?,
    )
}

/// Whether a job of this name has ever finished.
pub async fn done(db: &Db, job: &str) -> Result<bool, JobError> {
    Ok(last_done(db, job).await?.is_some())
}

/// Whether a load of the catalogue has finished. Not "are there titles": a load stopped
/// halfway leaves titles behind, and stage A over half a catalogue would mark itself done.
pub async fn loaded(db: &Db) -> Result<bool, JobError> {
    for job in LOAD {
        if done(db, job).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Stage A's steps not yet done, in order.
pub async fn stage_a_left(db: &Db) -> Result<Vec<&'static str>, JobError> {
    let mut left = Vec::new();
    for step in STAGE_A {
        if !done(db, step).await? {
            left.push(step);
        }
    }
    Ok(left)
}

/// Whether stage A should run now: a catalogue is loaded and stage A is not done.
pub async fn needs_stage_a(db: &Db) -> Result<bool, JobError> {
    Ok(loaded(db).await? && !stage_a_left(db).await?.is_empty())
}

/// Whether search and identification can work: a catalogue loaded and stage A done.
pub async fn ready(db: &Db) -> Result<bool, JobError> {
    Ok(loaded(db).await? && stage_a_left(db).await?.is_empty())
}

/// Whether a catch-up is owed: one stopped part-way, or titles arrived (a load, or
/// alternate titles) after the last one finished. Done by job order rather than by
/// looking for unindexed titles, which would read six million rows every hour.
pub async fn catch_up_due(db: &Db) -> Result<bool, JobError> {
    let unfinished: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ingest_jobs WHERE name = ? AND status IN ('running', 'failed')",
    )
    .bind(CATCH_UP)
    .fetch_one(db.pool())
    .await?;
    if unfinished > 0 {
        return Ok(true);
    }
    let last = last_done(db, CATCH_UP).await?.unwrap_or(0);
    for job in LOAD.into_iter().chain([AKAS]) {
        if last_done(db, job).await?.unwrap_or(0) > last {
            return Ok(true);
        }
    }
    Ok(false)
}

/// How far the wait for search has come, 0 to 100, for the page. Each part counts by its
/// measured share; normalising and indexing count their own rows. 100 only when `ready`.
pub async fn percent(db: &Db) -> Result<i32, JobError> {
    if ready(db).await? {
        return Ok(100);
    }
    let all: f64 = SHARE.iter().map(|(_, s)| s).sum();
    let mut have = 0.0;
    for (part, share) in SHARE {
        let fraction = if part == "load" {
            f64::from(u8::from(loaded(db).await?))
        } else if done(db, part).await? {
            1.0
        } else if part == "titles-normalise" {
            share_of(
                db,
                "SELECT COUNT(*) FROM titles WHERE normalised IS NOT NULL",
                "SELECT COUNT(*) FROM titles",
            )
            .await?
        } else {
            share_of(
                db,
                "SELECT COUNT(*) FROM search_indexed",
                "SELECT COUNT(*) FROM media_items WHERE kind <> 'episode'",
            )
            .await?
        };
        have += share * fraction;
    }
    Ok(((have / all * 100.0).floor() as i32).min(99))
}

async fn share_of(db: &Db, part: &str, whole: &str) -> Result<f64, JobError> {
    let part: i64 = sqlx::query_scalar(part).fetch_one(db.pool()).await?;
    let whole: i64 = sqlx::query_scalar(whole).fetch_one(db.pool()).await?;
    Ok(if whole > 0 {
        (part as f64 / whole as f64).clamp(0.0, 1.0)
    } else {
        0.0
    })
}

/// Alternate titles (romaji, native, English) for the core tier: how anime and foreign
/// films are found under the names people use. Searchable after the next catch-up.
pub async fn akas(db: &Db, datasets: &Path) -> Result<(), JobError> {
    let dataset = &crate::imdb::TITLE_AKAS;
    let path = datasets.join(dataset.filename);
    Downloader::new()
        .fetch(&dataset.url(), &path, |_| {})
        .await?;
    crate::download::verify_gzip(&path)?;
    let core = Arc::new(crate::credits::core_title_ids(db).await?);
    let mut job = Job::begin(db, AKAS).await?;
    if !core.is_empty() {
        crate::akas::load_akas(&mut job, path, core).await?;
    }
    job.finish().await?;
    Ok(())
}

/// The normalised form of every title, which exact-title search and anime matching need.
pub async fn normalise(db: &Db) -> Result<(), JobError> {
    let mut job = Job::begin(db, "titles-normalise").await?;
    crate::normalise::backfill(&mut job).await?;
    job.finish().await?;
    Ok(())
}

/// The word index over every title, and the typo-tolerant index over the core tier.
pub async fn search_index(
    db: &Db,
    trigram: bool,
    trigram_core: bool,
) -> Result<(i64, i64), JobError> {
    let mut job = Job::begin(db, "search-index").await?;
    let (main, trigrams) = index_into(&mut job, db, trigram, trigram_core).await?;
    job.finish().await?;
    Ok((main, trigrams))
}

async fn index_into(
    job: &mut Job<'_>,
    db: &Db,
    trigram: bool,
    trigram_core: bool,
) -> Result<(i64, i64), JobError> {
    let main = crate::search_index::build_main(job, db).await?;
    let trigrams = if trigram {
        crate::search_index::build_trigram(job, db, trigram_core).await?
    } else {
        0
    };
    Ok((main, trigrams))
}

/// Normalise and index what is missing; returns how many titles reached the word index.
///
/// A title that gained a name since it was indexed (its new name has no normalised form
/// yet) is un-marked first, so the index takes it again with the new name. The job is
/// begun before anything changes, so a catch-up stopped anywhere is unfinished, and
/// `catch_up_due` runs it again.
pub async fn catch_up(db: &Db) -> Result<i64, JobError> {
    let mut job = Job::begin(db, CATCH_UP).await?;
    sqlx::query(
        "DELETE FROM search_indexed WHERE media_item_id IN
           (SELECT media_item_id FROM titles WHERE normalised IS NULL)",
    )
    .execute(db.pool())
    .await?;
    crate::normalise::fill_missing(&mut job).await?;
    let (main, _) = index_into(&mut job, db, true, true).await?;
    job.finish().await?;
    Ok(main)
}

/// What a run of [`run`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Stage A ran (and finished): search works now where it did not.
    pub built: bool,
    /// Alternate titles loaded: the library's unmatched files are worth another look.
    pub akas: bool,
    /// Titles that reached the word index in the catch-up.
    pub indexed: i64,
}

/// After every refresh: whatever is left of stage A, then alternate titles if they never
/// loaded, then the catch-up if one is due. `on_step` hears each step's name as it starts.
/// Nothing happens until a catalogue has loaded: the refresh loads it first.
pub async fn run(db: &Db, datasets: &Path, on_step: impl Fn(&str)) -> Result<Report, JobError> {
    let mut report = Report::default();
    if !loaded(db).await? {
        return Ok(report);
    }
    for step in stage_a_left(db).await? {
        on_step(step);
        match step {
            "titles-normalise" => normalise(db).await?,
            _ => {
                search_index(db, true, true).await?;
            }
        }
        report.built = true;
    }
    if !done(db, AKAS).await? {
        on_step(AKAS);
        akas(db, datasets).await?;
        report.akas = true;
    }
    if catch_up_due(db).await? {
        on_step(CATCH_UP);
        report.indexed = catch_up(db).await?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinephile_persistence::repositories::MediaRepository;
    use sinephile_persistence::NewMediaItem;

    async fn fresh() -> (tempfile::TempDir, Db) {
        // On disk, as the app's is: a job reads through a second connection while its
        // step's transaction is open, which an in-memory database cannot give it.
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in(dir.path()).await.unwrap();
        (dir, db)
    }

    async fn film(db: &Db, title: &str, year: i64) -> i64 {
        MediaRepository::new(db)
            .insert(&NewMediaItem::film(title, year))
            .await
            .unwrap()
    }

    async fn finish_job(db: &Db, name: &str) {
        let job = Job::begin(db, name).await.unwrap();
        job.finish().await.unwrap();
    }

    async fn hits(db: &Db, word: &str) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM search_index WHERE search_index MATCH ?")
            .bind(word)
            .fetch_one(db.pool())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn search_works_after_a_finished_load_and_stage_a() {
        let (dir, db) = fresh().await;
        film(&db, "Nosferatu", 1922).await;
        // Titles from a load that has not finished: not ready, and nothing to build on.
        assert!(!loaded(&db).await.unwrap());
        assert!(!needs_stage_a(&db).await.unwrap());
        assert_eq!(percent(&db).await.unwrap(), 0);
        let nothing = run(&db, dir.path(), |_| {}).await.unwrap();
        assert_eq!(nothing, Report::default());

        finish_job(&db, "refresh").await;
        assert!(needs_stage_a(&db).await.unwrap());
        assert_eq!(stage_a_left(&db).await.unwrap(), STAGE_A.to_vec());
        let loaded_only = percent(&db).await.unwrap();
        assert!((1..100).contains(&loaded_only), "{loaded_only}");

        // Stage A's steps need no download: run them as `run` would.
        normalise(&db).await.unwrap();
        assert!(percent(&db).await.unwrap() > loaded_only);
        let (main, _) = search_index(&db, true, true).await.unwrap();
        assert_eq!(main, 1, "the one title is in the index");
        assert_eq!(hits(&db, "nosferatu").await, 1, "search finds it");
        assert!(
            ready(&db).await.unwrap(),
            "alternate titles do not hold Home back"
        );
        assert!(!needs_stage_a(&db).await.unwrap());
        assert_eq!(percent(&db).await.unwrap(), 100);
    }

    #[tokio::test]
    async fn a_refresh_s_new_titles_are_normalised_and_searchable_after_the_catch_up() {
        let (_dir, db) = fresh().await;
        film(&db, "Nosferatu", 1922).await;
        finish_job(&db, "refresh").await;
        normalise(&db).await.unwrap();
        search_index(&db, true, true).await.unwrap();
        assert!(
            catch_up_due(&db).await.unwrap(),
            "the load came after any catch-up"
        );
        catch_up(&db).await.unwrap();
        assert!(!catch_up_due(&db).await.unwrap());

        // The refresh adds a title, with no normalised form and no index entry.
        film(&db, "Metropolis", 1927).await;
        finish_job(&db, "refresh").await;
        assert!(catch_up_due(&db).await.unwrap());
        assert_eq!(hits(&db, "metropolis").await, 0);
        assert_eq!(catch_up(&db).await.unwrap(), 1, "only the new title");
        assert_eq!(hits(&db, "metropolis").await, 1);
        let missing: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM titles WHERE normalised IS NULL")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(missing, 0);
        assert!(!catch_up_due(&db).await.unwrap());
        assert_eq!(
            catch_up(&db).await.unwrap(),
            0,
            "nothing new, nothing redone"
        );
    }

    #[tokio::test]
    async fn an_alternate_title_added_after_the_index_becomes_searchable() {
        let (_dir, db) = fresh().await;
        let id = film(&db, "Attack on Titan", 2013).await;
        finish_job(&db, "refresh").await;
        normalise(&db).await.unwrap();
        search_index(&db, true, true).await.unwrap();
        assert_eq!(hits(&db, "kyojin").await, 0);

        // What `akas` does to a title already in the index: a new name, not normalised.
        sqlx::query("INSERT INTO titles (media_item_id, title, variant) VALUES (?, ?, 'romaji')")
            .bind(id)
            .bind("Shingeki no Kyojin")
            .execute(db.pool())
            .await
            .unwrap();
        finish_job(&db, AKAS).await;
        assert!(
            catch_up_due(&db).await.unwrap(),
            "alternate titles owe a catch-up"
        );
        assert_eq!(
            catch_up(&db).await.unwrap(),
            1,
            "the title is indexed again"
        );
        assert_eq!(hits(&db, "kyojin").await, 1, "under its romaji name");
        assert_eq!(hits(&db, "titan").await, 1, "and still under its own, once");
    }
}
