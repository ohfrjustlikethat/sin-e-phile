//! Evaluation harnesses (`SPEC.md` §12.2).
//!
//! **Every number in `docs/eval-results.md` comes from here**, which is the point: a
//! measurement taken by hand once is a claim, and a measurement a command reproduces is
//! evidence. §10.12 compares each phase's numbers against the previous phase's before a
//! merge, and that comparison is only possible if the numbers are re-derivable.
//!
//! Subcommands arrive with the phases that need them. Today: `search`.

use std::path::Path;

mod search;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    let report = args.iter().any(|a| a == "--report");

    let data_dir = std::env::var("SINEPHILE_DATA_DIR").unwrap_or_else(|_| "data".into());

    let outcome = match command {
        "search" => search::run(Path::new(&data_dir), report).await,
        "all" => search::run(Path::new(&data_dir), report).await,
        _ => {
            eprintln!(
                "eval — evaluation harnesses (SPEC.md 12.2)\n\
                 \n  eval search [--report]   exact-title top-1 (E2) and nDCG@10 (E3)\
                 \n  eval all [--report]      every harness that exists\
                 \n\
                 \nReads the catalogue from $SINEPHILE_DATA_DIR, default ./data.\n"
            );
            std::process::exit(2);
        }
    };

    match outcome {
        Ok(passed) if passed => {}
        Ok(_) => {
            // Non-zero on a missed target, so a harness is usable as a gate rather than
            // only by eye. §10.12 makes a regression block a merge exactly as a failing
            // test does, and that needs an exit code.
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("eval: {e}");
            std::process::exit(2);
        }
    }
}
