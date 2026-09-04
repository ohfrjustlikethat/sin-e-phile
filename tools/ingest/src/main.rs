//! `ingest` — the offline dataset pipeline (`SPEC.md` Phase 4).
//!
//! Phase 4 subtask 4.1 builds the resumable runner; the datasets themselves arrive
//! in 4.2 onward. Until then this binary can show job state and reset a job, which
//! is what makes the resume behaviour inspectable by hand rather than only by test.

use std::path::{Path, PathBuf};

use sinephile_ingest::{Job, JobError};
use sinephile_persistence::{paths, DataLocation, Db};

const USAGE: &str = "\
ingest — offline dataset ingestion (SPEC.md Phase 4)

  ingest measure [--quick] report the catalogue's shape so the scope is chosen
                           from evidence (SPEC.md R4). Writes nothing. --quick
                           skips title.principals and title.akas, which are the
                           large ones and the whole point.
  ingest imdb              download and load the IMDb catalogue. Resumable —
                           re-run it after an interruption and it carries on.
  ingest credits           load cast and crew for core-tier titles. Needs
                           `ingest imdb` to have run first.
  ingest akas              load alternative titles (romaji/native/english) for
                           core-tier titles. Needs `ingest imdb` first.
  ingest normalise         backfill titles.normalised, which anime matching and
                           Phase 5's exact-title search both need
  ingest anime [--pages N] match the AniList catalogue onto ours and promote what
                           matches to anime_film/anime_series. Needs `ingest
                           normalise` first. --pages bounds the run; pages come
                           most-popular-first, so a bounded run is the useful
                           part of the catalogue rather than an arbitrary slice.
  ingest episodes          load episodes: media_items, episodes, seasons and
                           episode_numbering. --measure reports what the dataset
                           holds and writes nothing. --min-votes N also loads
                           non-anime series with at least N votes; anime is
                           always loaded in full (SPEC.md 6.2).
  ingest movielens [--set S] download MovieLens and join it onto the catalogue by
                           IMDb id. S is ml-25m (default) or ml-latest-small.
                           Ratings are NOT stored — they are an input to the
                           on-device matrix (ADR-0019), and Phase 4's job is to
                           measure what that will cost.
  ingest repair-variants   recompute titles.variant for rows the region-over-
                           language bug labelled english. Narrow and re-runnable.
  ingest refresh           ADR-0030 layer 1: re-download title.basics and
                           title.ratings, insert only titles newer than the
                           highest id already held, and re-apply every rating.
                           Weekly is ample.
  ingest verify-anime      check the catalogue against
                           fixtures/anime/e5-hand-checked.tsv — exit criterion
                           E5's evidence. Exits non-zero on any mismatch.
  ingest status            show every job and its steps
  ingest reset <name>      discard a job's progress so the next run starts clean

Options:
  --data-dir <path>        override the data directory (or set SINEPHILE_DATA_DIR)

Datasets arrive in Phase 4 subtasks 4.2 onward. Today this inspects job state.
";

#[tokio::main]
async fn main() -> Result<(), JobError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    let mut args = std::env::args().skip(1).collect::<Vec<_>>();

    let mut data_dir: Option<PathBuf> = None;
    if let Some(i) = args.iter().position(|a| a == "--data-dir") {
        data_dir = args.get(i + 1).map(PathBuf::from);
        args.drain(i..=(i + 1).min(args.len() - 1));
    }

    let command = args.first().map(String::as_str).unwrap_or("");
    if command.is_empty() || command == "--help" || command == "-h" {
        print!("{USAGE}");
        return Ok(());
    }

    // Development by default: a `cargo run` that wrote into target/ would lose the
    // database to the next `cargo clean` (see crates/persistence/src/paths.rs).
    let dir = match data_dir {
        Some(dir) => dir,
        None => paths::data_dir(DataLocation::Development)
            .map_err(|e| JobError::step("startup", e.to_string()))?,
    };
    let db = Db::open_in(&dir).await?;

    match command {
        "measure" => {
            // Deliberately does not touch the database it was handed — a
            // measurement that mutates state cannot be re-run to check itself.
            let deep = !args.iter().any(|a| a == "--quick");
            let measurement = sinephile_ingest::measure::run(&dir.join("datasets"), deep).await?;
            sinephile_ingest::measure::report(&measurement);
            Ok(())
        }
        "imdb" => imdb(&db, &dir.join("datasets")).await,
        "credits" => credits(&db, &dir.join("datasets")).await,
        "akas" => akas(&db, &dir.join("datasets")).await,
        "normalise" => normalise(&db).await,
        "anime" => {
            let pages = args
                .iter()
                .position(|a| a == "--pages")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse::<i64>().ok());
            anime(&db, pages).await
        }
        "episodes" => {
            let min_votes = args
                .iter()
                .position(|a| a == "--min-votes")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse::<i64>().ok());
            episodes(
                &db,
                &dir.join("datasets"),
                args.iter().any(|a| a == "--measure"),
                min_votes,
            )
            .await
        }
        "movielens" => {
            let set = args
                .iter()
                .position(|a| a == "--set")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| sinephile_ingest::movielens::Release::parse(s))
                .unwrap_or(sinephile_ingest::movielens::Release::Ml25m);
            movielens(&db, &dir.join("datasets"), set).await
        }
        "repair-variants" => repair_variants(&db).await,
        "refresh" => refresh(&db, &dir.join("datasets")).await,
        "verify-anime" => verify_anime(&db).await,
        "status" => status(&db).await,
        "reset" => match args.get(1) {
            Some(name) => {
                let removed = Job::reset(&db, name).await?;
                println!("ingest: cleared {removed} job(s) named {name:?}");
                Ok(())
            }
            None => {
                eprintln!("ingest: reset needs a job name");
                std::process::exit(2);
            }
        },
        other => {
            eprintln!("ingest: unknown command {other:?}\n");
            print!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// Download and load the IMDb catalogue.
async fn imdb(db: &Db, datasets: &Path) -> Result<(), JobError> {
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let downloader = sinephile_ingest::Downloader::new();

    for dataset in [
        &sinephile_ingest::imdb::TITLE_RATINGS,
        &sinephile_ingest::imdb::TITLE_BASICS,
    ] {
        let path = datasets.join(dataset.filename);
        let result = downloader.fetch(&dataset.url(), &path, |_| {}).await?;
        tracing::info!(
            "{}: {:.1} MB{}",
            dataset.name,
            result.bytes as f64 / 1_048_576.0,
            if result.fetched {
                ""
            } else {
                " (already had it)"
            }
        );
        sinephile_ingest::download::verify_gzip(&path)?;
    }

    let ratings_path = datasets.join(sinephile_ingest::imdb::TITLE_RATINGS.filename);
    tracing::info!("reading ratings");
    let votes = Arc::new(sinephile_ingest::load::load_votes(&ratings_path)?);
    let averages = Arc::new(sinephile_ingest::load::load_average_ratings(&ratings_path)?);
    tracing::info!("  {} rated titles", votes.len());

    let mut job = Job::begin(db, "imdb").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }

    tracing::info!("loading titles");
    sinephile_ingest::load::load_titles(
        &mut job,
        datasets.join(sinephile_ingest::imdb::TITLE_BASICS.filename),
        votes,
        averages,
        sinephile_ingest::imdb::CatalogueScope::DEFAULT,
        // A first ingestion starts at the top of the file; `ingest refresh` is the
        // same call with a watermark.
        None,
    )
    .await?;
    job.finish().await?;

    let (total, core) = sinephile_ingest::load::counts(db).await?;
    let bytes = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    println!();
    println!("  {total} titles indexed, {core} in the core tier");
    println!(
        "  database {:.0} MB · {:.0}s",
        bytes as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    println!();
    Ok(())
}

/// Cast and crew for core-tier titles — the R4 measurement that decides whether the
/// >=10 vote threshold survives.
async fn credits(db: &Db, datasets: &Path) -> Result<(), JobError> {
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let before = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    let downloader = sinephile_ingest::Downloader::new();

    for dataset in [
        &sinephile_ingest::imdb::TITLE_PRINCIPALS,
        &sinephile_ingest::imdb::NAME_BASICS,
    ] {
        let path = datasets.join(dataset.filename);
        let result = downloader.fetch(&dataset.url(), &path, |_| {}).await?;
        tracing::info!(
            "{}: {:.1} MB{}",
            dataset.name,
            result.bytes as f64 / 1_048_576.0,
            if result.fetched {
                ""
            } else {
                " (already had it)"
            }
        );
        sinephile_ingest::download::verify_gzip(&path)?;
    }

    let principals = datasets.join(sinephile_ingest::imdb::TITLE_PRINCIPALS.filename);
    let names = datasets.join(sinephile_ingest::imdb::NAME_BASICS.filename);

    tracing::info!("reading core title ids");
    let core = Arc::new(sinephile_ingest::credits::core_title_ids(db).await?);
    if core.is_empty() {
        eprintln!("ingest: no core titles — run `ingest imdb` first");
        std::process::exit(2);
    }
    tracing::info!("  {} core titles", core.len());

    tracing::info!("scanning title.principals for the people they reference");
    let needed = Arc::new(sinephile_ingest::credits::scan_needed_people(
        &principals,
        &core,
    )?);
    let needed_count = needed.len();
    tracing::info!("  {needed_count} people needed");

    let mut job = Job::begin(db, "imdb-credits").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }

    tracing::info!("loading people");
    sinephile_ingest::credits::load_people(&mut job, names, needed).await?;
    // Read back which people actually landed. principals references nconsts that
    // name.basics does not have, and a credit cannot exist without its person.
    let loaded = Arc::new(sinephile_ingest::credits::loaded_people(db).await?);
    let missing = needed_count.saturating_sub(loaded.len());
    if missing > 0 {
        tracing::warn!(
            "{missing} people referenced by title.principals are absent from              name.basics; their credits are dropped"
        );
    }

    tracing::info!("loading credits");
    sinephile_ingest::credits::load_credits(&mut job, principals, core, loaded).await?;
    job.finish().await?;

    let (people, credits) = sinephile_ingest::credits::counts(db).await?;
    let after = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    println!();
    println!("  {people} people, {credits} credits");
    println!(
        "  database {:.0} MB (+{:.0} MB) · {:.0}s",
        after as f64 / 1_048_576.0,
        (after.saturating_sub(before)) as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    println!();
    Ok(())
}

/// Alternative titles for core-tier titles — the last unmeasured R4 component.
async fn akas(db: &Db, datasets: &Path) -> Result<(), JobError> {
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let before = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);

    let dataset = &sinephile_ingest::imdb::TITLE_AKAS;
    let path = datasets.join(dataset.filename);
    let result = sinephile_ingest::Downloader::new()
        .fetch(&dataset.url(), &path, |_| {})
        .await?;
    tracing::info!(
        "{}: {:.1} MB{}",
        dataset.name,
        result.bytes as f64 / 1_048_576.0,
        if result.fetched {
            ""
        } else {
            " (already had it)"
        }
    );
    sinephile_ingest::download::verify_gzip(&path)?;

    let core = Arc::new(sinephile_ingest::credits::core_title_ids(db).await?);
    if core.is_empty() {
        eprintln!("ingest: no core titles — run `ingest imdb` first");
        std::process::exit(2);
    }
    tracing::info!("{} core titles", core.len());

    let mut job = Job::begin(db, "imdb-akas").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }
    sinephile_ingest::akas::load_akas(&mut job, path, core).await?;
    job.finish().await?;

    let titles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM titles")
        .fetch_one(db.pool())
        .await?;
    let after = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    println!();
    println!("  {titles} title rows");
    println!(
        "  database {:.0} MB (+{:.0} MB) · {:.0}s",
        after as f64 / 1_048_576.0,
        (after.saturating_sub(before)) as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    println!();
    Ok(())
}

/// Match the AniList catalogue onto ours (subtask 4.4).
async fn anime(db: &Db, max_pages: Option<i64>) -> Result<(), JobError> {
    use std::sync::Arc;
    use std::time::Instant;

    let unnormalised = sinephile_ingest::normalise::remaining(db).await?;
    if unnormalised > 0 {
        // Failing loudly beats matching against a catalogue that is only partly
        // searchable: a NULL normalised form is invisible to the matcher, so the run
        // would "succeed" with a silently depressed match rate.
        eprintln!(
            "ingest: {unnormalised} titles have no normalised form. Run `ingest normalise` first."
        );
        std::process::exit(2);
    }

    let unmatched = db
        .path()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("anilist-unmatched.tsv");

    let started = Instant::now();
    let transport: Arc<dyn sinephile_metadata_api::Transport> =
        Arc::new(sinephile_metadata_api::HttpTransport::new());
    let client = Arc::new(sinephile_metadata_api::AniList::owned(transport).await);

    let mut job = Job::begin(db, "anilist").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }
    let report =
        sinephile_ingest::anime::ingest(&mut job, client, max_pages, Some(&unmatched)).await?;
    job.finish().await?;

    println!();
    println!("  {} AniList entries seen", report.seen);
    println!(
        "  {} matched ({:.1}%)",
        report.matched,
        report.match_rate() * 100.0
    );
    println!("      {} exact title and year", report.exact_title_and_year);
    println!(
        "      {} title only, year unknown",
        report.exact_title_year_unknown
    );
    println!("      {} season-aware", report.season_aware);
    println!(
        "  {} already claimed by an earlier entry",
        report.already_claimed
    );
    println!("  {} not in catalogue", report.not_in_catalogue);
    println!("  {} ambiguous (refused)", report.ambiguous);
    println!("  {} year conflict", report.year_conflict);
    println!("  {:.0}s", started.elapsed().as_secs_f64());

    // The unmatched list is the deliverable for exit criterion E5, which hand-checks
    // fifty. The file is the real artefact; this is a legible summary of it.
    //
    // SPREAD, not the first twenty-five. The sweep is year-ascending, so the first
    // unmatched entries are all obscure shorts from the 1950s — a sample of the sweep
    // order rather than of the catalogue, and useless for the cases E5 actually names.
    println!(
        "
  what did NOT match, spread across the whole sweep:"
    );
    for entry in report.unmatched_spread(25) {
        let name = if entry.romaji.is_empty() {
            &entry.english
        } else {
            &entry.romaji
        };
        println!("    {name:<48}  {}", entry.reason);
    }
    println!(
        "
  all {} written to {}",
        report.unmatched.len(),
        unmatched.display()
    );
    println!(
        "
  a sample of what DID match:"
    );
    for (title, form, id) in report.matched_samples.iter().take(25) {
        println!("    {title:<48}  on {form:?} -> item {id}");
    }
    println!();
    Ok(())
}

/// Episodes: measure first, because title.episode is ~8.5M rows and R4 has 670 MB.
async fn episodes(
    db: &Db,
    datasets: &Path,
    measure_only: bool,
    min_votes: Option<i64>,
) -> Result<(), JobError> {
    let downloader = sinephile_ingest::Downloader::new();
    let dataset = &sinephile_ingest::imdb::TITLE_EPISODE;
    let path = datasets.join(dataset.filename);
    let result = downloader.fetch(&dataset.url(), &path, |_| {}).await?;
    tracing::info!(
        "{}: {:.1} MB{}",
        dataset.name,
        result.bytes as f64 / 1_048_576.0,
        if result.fetched {
            ""
        } else {
            " (already had it)"
        }
    );
    sinephile_ingest::download::verify_gzip(&path)?;

    if measure_only {
        let measurement = sinephile_ingest::episodes::measure(db, datasets).await?;
        measurement.report();
        return Ok(());
    }

    use sinephile_ingest::episodes_load as ep;
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let before = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    let (episodes_before, _) = ep::count_episodes(db).await?;

    let scope = ep::EpisodeScope {
        all_anime: true,
        min_votes: min_votes.unwrap_or(i64::MAX),
    };
    let series = ep::series(db).await?;
    let skip = ep::already_loaded(db).await?;
    tracing::info!(
        "{} series in the catalogue, {} episodes already loaded",
        series.len(),
        skip.len()
    );

    let wanted = ep::collect(&path, &series, scope, &skip)?;
    tracing::info!("{} episodes in scope", wanted.len());
    if wanted.is_empty() {
        println!(
            "
  nothing new in scope
"
        );
        return Ok(());
    }

    let by_id: std::collections::HashMap<u32, ep::Wanted> =
        wanted.iter().map(|w| (w.tconst, *w)).collect();
    let wanted = Arc::new(wanted);

    let mut job = Job::begin(db, "episodes").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }
    ep::load_series_rows(&mut job, Arc::clone(&wanted)).await?;
    ep::load_episodes(
        &mut job,
        datasets.join(sinephile_ingest::imdb::TITLE_BASICS.filename),
        Arc::new(by_id),
    )
    .await?;
    ep::load_seasons(&mut job).await?;
    job.finish().await?;

    let (episodes, seasons) = ep::count_episodes(db).await?;
    let after = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    let added = after.saturating_sub(before);
    println!();
    println!("  {episodes} episodes, {seasons} seasons");
    println!(
        "  database {:.0} MB (+{:.0} MB) · {:.0}s",
        after as f64 / 1_048_576.0,
        added as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    // Per NEW episode, not per total. Dividing the delta by the whole table made an
    // incremental run report 244 bytes against the 405 the same data cost when it was
    // first loaded — a number that gets better every time you widen the scope, which
    // is exactly the direction a cost estimate must never drift.
    let new_episodes = episodes - episodes_before;
    if new_episodes > 0 {
        println!(
            "  {new_episodes} new · {:.0} bytes per new episode, measured",
            added as f64 / new_episodes as f64
        );
    }
    println!();
    Ok(())
}

/// MovieLens: join by IMDb id, and measure what the on-device matrix will cost.
async fn movielens(
    db: &Db,
    datasets: &Path,
    release: sinephile_ingest::movielens::Release,
) -> Result<(), JobError> {
    use sinephile_ingest::movielens as ml;
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let before = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);

    let path = datasets.join(release.filename());
    let downloader = sinephile_ingest::Downloader::new();
    let result = downloader.fetch(&release.url(), &path, |_| {}).await?;
    tracing::info!(
        "{}: {:.1} MB{}",
        release.name(),
        result.bytes as f64 / 1_048_576.0,
        if result.fetched {
            ""
        } else {
            " (already had it)"
        }
    );

    let links = ml::links(&path)?;
    tracing::info!("{} films in links.csv", links.len());
    let catalogue = ml::catalogue_by_imdb(db).await?;

    let mut measurement = ml::Measurement {
        links: links.len() as i64,
        ..Default::default()
    };
    let mut pairs: Vec<(i64, i64)> = Vec::new();
    for link in &links {
        match catalogue.get(&link.imdb_id) {
            Some(media_item_id) => {
                measurement.matched += 1;
                pairs.push((*media_item_id, link.movielens_id));
            }
            None => measurement.unmatched += 1,
        }
    }

    let mut job = Job::begin(db, "movielens").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }
    ml::load(&mut job, Arc::new(pairs)).await?;
    job.finish().await?;

    // The number ADR-0019 requires Phase 4 to produce: streaming every rating is the
    // floor on what the on-device item-item matrix costs on first run.
    let (ratings, seconds) = ml::scan_ratings(&path)?;
    measurement.ratings = ratings;
    measurement.ratings_scan_seconds = seconds;
    measurement.report(release);

    let after = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    println!(
        "  {} catalogue items carry a MovieLens id",
        ml::mapped(db).await?
    );
    println!(
        "  database {:.0} MB (+{:.0} MB) · {:.0}s total",
        after as f64 / 1_048_576.0,
        after.saturating_sub(before) as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    println!();
    Ok(())
}

/// ADR-0030 layer 1 — incremental bulk refresh.
async fn refresh(db: &Db, datasets: &Path) -> Result<(), JobError> {
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let before = sinephile_ingest::refresh::title_count(db).await?;
    let watermark = sinephile_ingest::refresh::watermark(db).await?;
    match &watermark {
        Some(w) => tracing::info!("{before} titles held; refreshing past {w}"),
        None => tracing::info!("empty catalogue — this is a first ingestion"),
    }

    // Both files are re-fetched: gzip cannot be seeked and IMDb publishes no
    // changelog, so the download is the unavoidable cost of layer 1.
    let downloader = sinephile_ingest::Downloader::new();
    for dataset in [
        &sinephile_ingest::imdb::TITLE_RATINGS,
        &sinephile_ingest::imdb::TITLE_BASICS,
    ] {
        let path = datasets.join(dataset.filename);
        // Deleting first, because the downloader skips a file it already has — which
        // is right for resumption and exactly wrong for a refresh.
        let _ = std::fs::remove_file(&path);
        let result = downloader.fetch(&dataset.url(), &path, |_| {}).await?;
        tracing::info!(
            "{}: {:.1} MB",
            dataset.name,
            result.bytes as f64 / 1_048_576.0
        );
        sinephile_ingest::download::verify_gzip(&path)?;
    }

    let ratings_path = datasets.join(sinephile_ingest::imdb::TITLE_RATINGS.filename);
    let votes = Arc::new(sinephile_ingest::load::load_votes(&ratings_path)?);
    let averages = Arc::new(sinephile_ingest::load::load_average_ratings(&ratings_path)?);

    let mut job = Job::begin(db, "refresh").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous refresh");
    }
    sinephile_ingest::load::load_titles(
        &mut job,
        datasets.join(sinephile_ingest::imdb::TITLE_BASICS.filename),
        Arc::clone(&votes),
        Arc::clone(&averages),
        sinephile_ingest::imdb::CatalogueScope::DEFAULT,
        watermark,
    )
    .await?;
    let rerated = sinephile_ingest::refresh::ratings(&mut job, votes, averages).await?;
    job.finish().await?;

    let after = sinephile_ingest::refresh::title_count(db).await?;
    println!();
    println!("  {} titles added", after - before);
    println!("  {rerated} ratings re-applied");
    println!("  {:.0}s", started.elapsed().as_secs_f64());
    println!();
    println!("  Not covered, deliberately: a REVISION to an existing title other than");
    println!("  its rating — a corrected year, a changed runtime. Those stay stale");
    println!("  until a full re-ingest (219 s). See docs/specs/catalogue-freshness.md.");
    println!();
    Ok(())
}

/// Check the catalogue against the E5 fixture.
async fn verify_anime(db: &Db) -> Result<(), JobError> {
    let path = Path::new("fixtures/anime/e5-hand-checked.tsv");
    let rows = sinephile_ingest::verify::load(path)?;
    let outcomes = sinephile_ingest::verify::run(db, &rows).await?;
    if !sinephile_ingest::verify::report(&outcomes) {
        // Non-zero, so this is usable as evidence in CI rather than only by eye.
        std::process::exit(1);
    }
    Ok(())
}

/// Recompute variants the region-over-language bug got wrong.
async fn repair_variants(db: &Db) -> Result<(), JobError> {
    use std::time::Instant;

    let started = Instant::now();
    let before = sinephile_ingest::repair::mislabelled_english(db).await?;
    tracing::info!("{before} rows claim to be english while carrying another language");

    let mut job = Job::begin(db, "repair-variants").await?;
    sinephile_ingest::repair::english_variants(&mut job).await?;
    job.finish().await?;

    let after = sinephile_ingest::repair::mislabelled_english(db).await?;
    println!();
    println!(
        "  {} rows repaired, {after} still mislabelled",
        before - after
    );
    println!("  {:.0}s", started.elapsed().as_secs_f64());
    println!();
    Ok(())
}

/// Backfill the normalised title form (migration 0009).
async fn normalise(db: &Db) -> Result<(), JobError> {
    use std::time::Instant;

    let started = Instant::now();
    let before = sinephile_ingest::normalise::remaining(db).await?;
    tracing::info!("{before} titles need normalising");

    let mut job = Job::begin(db, "titles-normalise").await?;
    if job.is_resuming().await? {
        tracing::info!("resuming a previous run");
    }
    sinephile_ingest::normalise::backfill(&mut job).await?;
    job.finish().await?;

    let after = sinephile_ingest::normalise::remaining(db).await?;
    println!();
    println!(
        "  {} titles normalised, {after} still without a form",
        before - after
    );
    println!("  {:.0}s", started.elapsed().as_secs_f64());
    println!();
    Ok(())
}

async fn status(db: &Db) -> Result<(), JobError> {
    let jobs: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, status, started_at, error
         FROM ingest_jobs ORDER BY started_at DESC LIMIT 20",
    )
    .fetch_all(db.pool())
    .await?;

    if jobs.is_empty() {
        println!("ingest: no jobs have been run in {}", db.path().display());
        return Ok(());
    }

    for (id, name, status, started, error) in jobs {
        println!("\n  {name}  [{status}]  started {started}  (job {id})");
        if let Some(error) = error {
            println!("    error: {error}");
        }

        let steps: Vec<(String, String, i64, Option<i64>)> = sqlx::query_as(
            "SELECT name, status, items_done, items_total
             FROM ingest_steps WHERE job_id = ? ORDER BY ordinal",
        )
        .bind(id)
        .fetch_all(db.pool())
        .await?;

        for (step, step_status, done, total) in steps {
            // An unknown total is normal for a streamed file, so it is shown as
            // unknown rather than as a fabricated percentage.
            let progress = match total {
                Some(total) if total > 0 => format!("{done}/{total} ({}%)", done * 100 / total),
                _ => format!("{done} items"),
            };
            println!("    {step_status:<9} {step:<28} {progress}");
        }
    }
    println!();
    Ok(())
}
