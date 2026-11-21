//! Evaluation harnesses (`SPEC.md` §12.2).
//!
//! **Every number in `docs/eval-results.md` comes from here**, which is the point: a
//! measurement taken by hand once is a claim, and a measurement a command reproduces is
//! evidence. §10.12 compares each phase's numbers against the previous phase's before a
//! merge, and that comparison is only possible if the numbers are re-derivable.
//!
//! Subcommands arrive with the phases that need them. Today: `search`, `vector`, `embed`,
//! `subs` and `identify`.

use std::path::Path;

mod advise;
mod discovery;
mod embed;
mod error;
mod identify;
mod imports;
mod modes;
mod recommend;
mod relevance;
mod scan;
mod search;
mod subs;
mod surprise;
mod switching;
mod taste;
mod tvlayouts;
mod update;
mod vector;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    let report = args.iter().any(|a| a == "--report");
    let prove = args.iter().any(|a| a == "--prove");
    let expansion = args
        .iter()
        .position(|a| a == "--ef")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse::<usize>().ok());

    let data_dir = std::env::var("SINEPHILE_DATA_DIR").unwrap_or_else(|_| "data".into());

    let outcome = match command {
        "search" => match args
            .iter()
            .position(|a| a == "--query")
            .and_then(|i| args.get(i + 1))
        {
            Some(query) => search::one(Path::new(&data_dir), query).await,
            // 18.4: `--tune-meaning` sweeps the popularity prior on the tuning set;
            // `--prior W` runs E1 to E3 with it at W.
            None if args.iter().any(|a| a == "--tune-meaning") => {
                search::tune_meaning(Path::new(&data_dir), &[0.0, 0.05, 0.1, 0.2, 0.4]).await
            }
            None => {
                let prior = args
                    .iter()
                    .position(|a| a == "--prior")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|n| n.parse::<f64>().ok());
                search::run_with(Path::new(&data_dir), report, prior).await
            }
        },
        "vector" if args.iter().any(|a| a == "--memory") => vector::memory(Path::new(&data_dir)),
        "vector" => vector::run(Path::new(&data_dir), report, prove, expansion).await,
        "embed" if args.iter().any(|a| a == "--compare") => {
            let i = args.iter().position(|a| a == "--compare").expect("present");
            let query = args.get(i + 1).cloned().unwrap_or_default();
            let ids: Vec<i64> = args
                .get(i + 2)
                .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
                .unwrap_or_default();
            embed::compare(Path::new(&data_dir), &query, &ids).await
        }
        "embed" => embed::run(Path::new(&data_dir), report).await,
        "subs" if args.iter().any(|a| a == "--trace") => {
            let i = args.iter().position(|a| a == "--trace").expect("present");
            match (args.get(i + 1), args.get(i + 2)) {
                (Some(wav), Some(out)) => subs::trace(Path::new(wav), Path::new(out)),
                _ => Err(error::EvalError::Missing(
                    "eval subs --trace <wav> <out>".into(),
                )),
            }
        }
        "subs" => subs::run(
            report,
            args.iter()
                .position(|a| a == "--only")
                .and_then(|i| args.get(i + 1))
                .map(String::as_str),
        ),
        "identify" if args.iter().any(|a| a == "--timing") => {
            identify::timing(Path::new(&data_dir)).await
        }
        "advise" => {
            let set = if args.iter().any(|a| a == "--tuned") {
                "tuned"
            } else {
                "fresh"
            };
            advise::run(set).await
        }
        "scan" => {
            let files = args
                .iter()
                .position(|a| a == "--files")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse().ok())
                .unwrap_or(10_000);
            scan::run(Path::new(&data_dir), files).await
        }
        "identify" => identify::run(Path::new(&data_dir), report).await,
        "taste" => taste::run(Path::new(&data_dir), args.iter().any(|a| a == "--tune")).await,
        "switch" => switching::run(Path::new(&data_dir)).await,
        "update" => update::run(Path::new(&data_dir)).await,
        "surprise" => surprise::run(Path::new(&data_dir)).await,
        "tvlayouts" if args.iter().any(|a| a == "--folder") => {
            let at = args.iter().position(|a| a == "--folder").unwrap_or(0) + 1;
            let root = args.get(at).cloned().unwrap_or_default();
            tvlayouts::folder(Path::new(&data_dir), Path::new(&root)).await
        }
        "tvlayouts" => {
            tvlayouts::run(
                Path::new(&data_dir),
                args.iter().any(|a| a == "--before"),
                args.iter().any(|a| a == "--report"),
            )
            .await
        }
        "recommend" => {
            // `--alpha 0.6,0.7 --shrink 0,2` narrows the tuning grid.
            let list = |flag: &str, default: &[f32]| -> Vec<f32> {
                args.iter()
                    .position(|a| a == flag)
                    .and_then(|i| args.get(i + 1))
                    .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
                    .unwrap_or_else(|| default.to_vec())
            };
            // `--rail 1`: profile 1's taste rail with every card's reason.
            let rail = args
                .iter()
                .position(|a| a == "--rail")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse().ok());
            if let Some(profile) = rail {
                recommend::rail(Path::new(&data_dir), profile).await
            } else if args.iter().any(|a| a == "--whole") {
                let limit = args
                    .iter()
                    .position(|a| a == "--limit")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|n| n.parse().ok());
                let value = |flag: &str| {
                    args.iter()
                        .position(|a| a == flag)
                        .and_then(|i| args.get(i + 1))
                        .cloned()
                };
                let asked = value("--asked").and_then(|n| n.parse().ok());
                // `--caps 2,1,3`: per director, per franchise, per year.
                let caps = value("--caps").and_then(|c| {
                    let n: Vec<usize> = c.split(',').filter_map(|x| x.parse().ok()).collect();
                    (n.len() == 3).then(|| sinephile_recommend::rank::Caps {
                        per_director: n[0],
                        per_franchise: n[1],
                        per_year: n[2],
                    })
                });
                recommend::whole(
                    Path::new(&data_dir),
                    args.iter().any(|a| a == "--tune"),
                    args.iter().any(|a| a == "--check"),
                    limit,
                    asked,
                    caps,
                    !args.iter().any(|a| a == "--uncalibrated"),
                    args.iter()
                        .any(|a| a == "--people")
                        .then(|| list("--people", &[0.0])),
                )
                .await
            } else {
                recommend::run(
                    Path::new(&data_dir),
                    args.iter().any(|a| a == "--tune"),
                    &list("--alpha", &[0.3, 0.5, 0.7, 0.9]),
                    &list("--shrink", &[0.0, 2.0, 5.0, 10.0]),
                )
            }
        }
        // Phase 18. `--simulate`: E1 and E2 on simulated viewers (`--no-floor` and `--frozen`
        // show each verdict can fail).
        "discovery" if args.iter().any(|a| a == "--simulate") => discovery::simulate(
            args.iter().any(|a| a == "--no-floor"),
            args.iter().any(|a| a == "--frozen"),
        ),
        "discovery" => {
            let limit = args
                .iter()
                .position(|a| a == "--limit")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse().ok());
            // `--tune`: the tuning users (B17's prior is chosen on them); `--prior-weight K`: the
            // app's prior means at another weight (0 for none).
            let weight = args
                .iter()
                .position(|a| a == "--prior-weight")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse::<f64>().ok());
            let prior = match weight {
                Some(0.0) => sinephile_recommend::bandit::Prior::NONE,
                Some(w) => sinephile_recommend::bandit::PRIOR.weighed(w),
                None => sinephile_recommend::bandit::PRIOR,
            };
            discovery::novelty_run(
                Path::new(&data_dir),
                limit,
                args.iter().any(|a| a == "--tune"),
                prior,
            )
            .await
        }
        "modes" => modes::run(Path::new(&data_dir), args.iter().any(|a| a == "--prove")).await,
        "import" => {
            let from = args
                .iter()
                .position(|a| a == "--from")
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            imports::run(Path::new(&data_dir), from).await
        }
        "all" => match search::run(Path::new(&data_dir), report).await {
            // Every harness runs even when an earlier one misses its target — stopping
            // at the first failure hides the rest, and §10.12 compares ALL the numbers
            // against the previous phase before a merge.
            Ok(searched) => match vector::run(Path::new(&data_dir), report, false, None).await {
                Ok(vectored) => match subs::run(report, None) {
                    Ok(subbed) => identify::run(Path::new(&data_dir), report)
                        .await
                        .map(|identified| searched && vectored && subbed && identified),
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        },
        _ => {
            eprintln!(
                "eval — evaluation harnesses (SPEC.md 12.2)\n\
                 \n  eval search [--report]   exact-title top-1 (E2) and nDCG@10 (E3)\
                 \n  eval vector [--report]   HNSW recall@10 against brute force\
                 \n       [--prove]           shift the id mapping and confirm recall collapses\
                 \n       [--memory]          what mmap saves over reading the index in\
                 \n                           full — the measurement behind P11\
                 \n  eval embed [--report]    query-embedding latency, and whether a\
                 \n                           re-embedded document still matches the artefact\
                 \n  eval subs [--report]     subtitle alignment over fixtures/subtitles\
                 \n       --trace <wav> <out> the speech trace of a 16-bit mono WAV\
                 \n  eval identify [--report] what each name in fixtures/filenames is (Phase 12)\
                 \n  eval scan [--files N]    scan N distinct catalogue films (Phase 12 E3)\
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
