//! `ingest` — the offline dataset pipeline (`SPEC.md` Phase 4).
//!
//! Phase 4 subtask 4.1 builds the resumable runner; the datasets themselves arrive
//! in 4.2 onward. Until then this binary can show job state and reset a job, which
//! is what makes the resume behaviour inspectable by hand rather than only by test.

use std::path::PathBuf;

use sinephile_ingest::{Job, JobError};
use sinephile_persistence::{paths, DataLocation, Db};

const USAGE: &str = "\
ingest — offline dataset ingestion (SPEC.md Phase 4)

  ingest measure [--quick] report the catalogue's shape so the scope is chosen
                           from evidence (SPEC.md R4). Writes nothing. --quick
                           skips title.principals and title.akas, which are the
                           large ones and the whole point.
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
