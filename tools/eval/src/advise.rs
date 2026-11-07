//! `eval advise`: Phase 13 E1 — the advisor's recommendation matches what actually
//! happens in at least 8 of 10 trials across varied swarm conditions.
//!
//! Each trial builds a loopback swarm (one to four seeders, each capped at a chosen rate),
//! measures it the way the app does (a few seconds of a fresh download's rate), asks the
//! advisor, and then plays the film from a second fresh engine the way the player does:
//! a 3-second buffer, then the film's bitrate in quarter-second slots. A slot that arrives
//! more than half a second late is a pause to buffer. The advice was right when it said
//! stream and nothing paused, or said download and something did.

use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{create_torrent, CreateTorrentOptions};
use sinephile_downloads::advise::{advise, Choice, Inputs, Swarm, Taste};
use sinephile_torrent::{AddOptions, Engine, EngineConfig, SeedingPolicy, Source};
use tempfile::TempDir;
use tokio::io::AsyncReadExt;

use crate::error::EvalError;

const MB: f64 = 1024.0 * 1024.0;
/// Each trial's film runs this long.
const RUNTIME: f64 = 20.0;

struct Trial {
    bitrate: f64,
    seeders: usize,
    per_seeder: f64,
}

fn offline(dir: &Path) -> EngineConfig {
    let mut c = EngineConfig::new(dir);
    c.dht = false;
    c.trackers = false;
    c.lsd = false;
    c.loopback_only = true;
    c.dead_swarm_after = Duration::from_secs(10);
    c
}

fn io(path: &Path, e: std::io::Error) -> EvalError {
    EvalError::Io {
        path: path.display().to_string(),
        source: e,
    }
}

fn engine_error(e: impl std::fmt::Display) -> EvalError {
    EvalError::Missing(format!("torrent engine: {e}"))
}

async fn fresh(
    dir: &Path,
    torrent: &[u8],
    peers: &[SocketAddr],
) -> Result<(Engine, String), EvalError> {
    let mut config = offline(dir);
    // As the app's streaming torrent does while it plays: a finished download is not
    // paused under the reader.
    config.seeding = SeedingPolicy::KeepSeeding;
    let engine = Engine::start(config).await.map_err(engine_error)?;
    let id = engine
        .add(
            Source::File(torrent.to_vec()),
            AddOptions {
                initial_peers: peers.to_vec(),
                ..Default::default()
            },
        )
        .await
        .map_err(engine_error)?;
    Ok((engine, id))
}

/// What the app measures: a fresh download's rate, the later half of five seconds (the
/// first seconds are peers still connecting, which understates a swarm).
async fn measure(dir: &Path, torrent: &[u8], peers: &[SocketAddr]) -> Result<Swarm, EvalError> {
    let (engine, id) = fresh(dir, torrent, peers).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut rates = Vec::new();
    let mut live = 0;
    for i in 0..8 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let s = engine.status(&id).map_err(engine_error)?;
        if s.finished {
            break;
        }
        if i >= 4 {
            rates.push(s.download_bps);
        }
        live = live.max(s.live_peers);
    }
    engine.stop().await;
    let n = rates.len().max(1) as f64;
    let mean = rates.iter().sum::<f64>() / n;
    let var = rates.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    Ok(Swarm {
        throughput: mean,
        spread: var.sqrt(),
        peers: live,
    })
}

/// Play it as the player would; returns the number of pauses to buffer.
async fn play(
    dir: &Path,
    torrent: &[u8],
    peers: &[SocketAddr],
    bitrate: f64,
) -> Result<u32, EvalError> {
    let (engine, id) = fresh(dir, torrent, peers).await?;
    // As the player's stream source does: a live peer first, then the reader.
    engine.wait_for_peers(&id).await.map_err(engine_error)?;
    let mut reader = engine.stream(&id, 0).await.map_err(engine_error)?;
    // The player reads in 256 KiB chunks; a quarter-second of a 1 MB/s film is one.
    let chunk = 256 * 1024;
    let slot = Duration::from_secs_f64(chunk as f64 / bitrate);
    let mut buf = vec![0u8; chunk];
    let startup = (bitrate * 3.0) as usize / chunk;
    let mut got = 0;
    for _ in 0..startup {
        reader.read_exact(&mut buf).await.map_err(|e| {
            io(
                dir,
                std::io::Error::other(format!("{e} after {got} bytes of the start-up buffer")),
            )
        })?;
        got += chunk;
    }
    let total = (bitrate * RUNTIME) as usize - got;
    let mut read = 0;
    let mut stalls = 0;
    let mut clock = Instant::now();
    let mut k: u32 = 0;
    while read + chunk <= total {
        k += 1;
        reader.read_exact(&mut buf).await.map_err(|e| {
            io(
                dir,
                std::io::Error::other(format!("{e} at {} bytes", got + read)),
            )
        })?;
        read += chunk;
        let due = clock + slot * k;
        let now = Instant::now();
        if now > due + Duration::from_millis(500) {
            stalls += 1;
            // Rebuffered: playback resumes from here.
            clock = now;
            k = 0;
        } else if now < due {
            tokio::time::sleep(due - now).await;
        }
    }
    engine.stop().await;
    Ok(stalls)
}

async fn trial(t: &Trial) -> Result<(Choice, String, f64, u32, Swarm), EvalError> {
    let dir = TempDir::new().map_err(|e| io(Path::new("temp"), e))?;
    let name = "Advisor.Trial";
    let root = dir.path().join("content").join(name);
    std::fs::create_dir_all(&root).map_err(|e| io(&root, e))?;
    let len = (t.bitrate * MB * RUNTIME) as usize;
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let bytes: Vec<u8> = (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    let file = root.join(format!("{name}.mkv"));
    std::fs::write(&file, &bytes).map_err(|e| io(&file, e))?;
    let torrent = create_torrent(
        &root,
        CreateTorrentOptions {
            name: Some(name),
            trackers: vec![],
            piece_length: Some(256 * 1024),
        },
        &BlockingSpawner::new(2),
    )
    .await
    .map_err(engine_error)?
    .as_bytes()
    .map_err(engine_error)?
    .to_vec();

    let mut seeders = Vec::new();
    let mut peers = Vec::new();
    for i in 0..t.seeders {
        let mut config = offline(&dir.path().join(format!("seeder{i}")));
        config.seeding = SeedingPolicy::KeepSeeding;
        config.upload_limit = Some((t.per_seeder * MB) as u32);
        let engine = Engine::start(config).await.map_err(engine_error)?;
        engine
            .add(
                Source::File(torrent.clone()),
                AddOptions {
                    output_folder: Some(root.clone()),
                    ..Default::default()
                },
            )
            .await
            .map_err(engine_error)?;
        peers.push(SocketAddr::from((
            [127, 0, 0, 1],
            engine.listen_addr().map_or(0, |a| a.port()),
        )));
        seeders.push(engine);
    }
    // Every seeder must have checked its files and hold the whole film before anyone asks.
    for s in &seeders {
        let id = s.list().first().map(|t| t.id.clone()).unwrap_or_default();
        let deadline = Instant::now() + Duration::from_secs(60);
        while !s.status(&id).map_err(engine_error)?.finished {
            if Instant::now() > deadline {
                return Err(EvalError::Missing(
                    "a seeder never finished checking its files".into(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    let swarm = measure(&dir.path().join("measure"), &torrent, &peers).await?;
    let advice = advise(&Inputs {
        file_bytes: len as u64,
        runtime_seconds: RUNTIME,
        buffer_seconds: 3.0,
        swarm: Some(swarm),
        local_copy: false,
        free_disk_bytes: u64::MAX / 2,
        hourly: [None; 24],
        hour_now: 20,
        taste: Taste::neutral(),
    });
    let stalls = play(&dir.path().join("play"), &torrent, &peers, t.bitrate * MB).await?;
    for s in seeders {
        s.stop().await;
    }
    Ok((
        advice.choice,
        advice.sentence,
        advice.stall.unwrap_or(1.0),
        stalls,
        swarm,
    ))
}

/// The conditions the advisor was tuned on (its first run, docs/eval-results.md).
fn tuned() -> Vec<Trial> {
    vec![
        Trial {
            bitrate: 1.0,
            seeders: 1,
            per_seeder: 4.0,
        },
        Trial {
            bitrate: 1.0,
            seeders: 1,
            per_seeder: 2.0,
        },
        Trial {
            bitrate: 1.0,
            seeders: 1,
            per_seeder: 1.4,
        },
        Trial {
            bitrate: 1.0,
            seeders: 1,
            per_seeder: 1.0,
        },
        Trial {
            bitrate: 1.0,
            seeders: 1,
            per_seeder: 0.7,
        },
        Trial {
            bitrate: 1.0,
            seeders: 1,
            per_seeder: 0.4,
        },
        Trial {
            bitrate: 1.0,
            seeders: 3,
            per_seeder: 1.5,
        },
        Trial {
            bitrate: 1.0,
            seeders: 3,
            per_seeder: 0.5,
        },
        Trial {
            bitrate: 1.0,
            seeders: 3,
            per_seeder: 0.3,
        },
        Trial {
            bitrate: 1.0,
            seeders: 4,
            per_seeder: 0.25,
        },
        Trial {
            bitrate: 1.5,
            seeders: 2,
            per_seeder: 1.0,
        },
        Trial {
            bitrate: 4.0,
            seeders: 1,
            per_seeder: 8.0,
        },
    ]
}

/// Conditions chosen after the model changed and run once: the set E1 is judged on.
fn fresh_set() -> Vec<Trial> {
    vec![
        Trial {
            bitrate: 2.0,
            seeders: 1,
            per_seeder: 5.0,
        },
        Trial {
            bitrate: 2.0,
            seeders: 1,
            per_seeder: 2.6,
        },
        Trial {
            bitrate: 2.0,
            seeders: 2,
            per_seeder: 0.9,
        },
        Trial {
            bitrate: 2.0,
            seeders: 1,
            per_seeder: 1.2,
        },
        Trial {
            bitrate: 0.8,
            seeders: 2,
            per_seeder: 1.0,
        },
        Trial {
            bitrate: 0.8,
            seeders: 4,
            per_seeder: 0.22,
        },
        Trial {
            bitrate: 0.8,
            seeders: 1,
            per_seeder: 0.85,
        },
        Trial {
            bitrate: 0.8,
            seeders: 3,
            per_seeder: 0.2,
        },
        Trial {
            bitrate: 3.0,
            seeders: 3,
            per_seeder: 2.0,
        },
        Trial {
            bitrate: 3.0,
            seeders: 2,
            per_seeder: 1.2,
        },
    ]
}

pub async fn run(set: &str) -> Result<bool, EvalError> {
    let trials = if set == "tuned" { tuned() } else { fresh_set() };
    println!(
        "advise — the advisor against what actually happens, {} trials on loopback ({} set)\n",
        trials.len(),
        if set == "tuned" { "tuned" } else { "fresh" }
    );
    println!("  film MB/s  swarm (seeders x cap)  measured MB/s (spread, peers)  stall p  advice     paused  right");
    let mut right = 0;
    for t in &trials {
        let (choice, sentence, p, stalls, swarm) = match trial(t).await {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "  {:>8.1}  {:>2} x {:>4.2}  the trial itself failed: {e}",
                    t.bitrate, t.seeders, t.per_seeder
                );
                continue;
            }
        };
        let paused = stalls > 0;
        let ok = match choice {
            Choice::Stream | Choice::PlayLocal => !paused,
            Choice::Download | Choice::DownloadTonight => paused,
        };
        right += usize::from(ok);
        println!(
            "  {:>8.1}  {:>2} x {:>4.2} = {:>4.2}        {:>5.2} ({:>4.2}, {})              {:>5.2}  {:<9}  {:>6}  {}",
            t.bitrate,
            t.seeders,
            t.per_seeder,
            t.seeders as f64 * t.per_seeder,
            swarm.throughput / MB,
            swarm.spread / MB,
            swarm.peers,
            p,
            format!("{choice:?}"),
            stalls,
            if ok { "yes" } else { "NO" }
        );
        println!("            \"{sentence}\"");
    }
    let met = right * 10 >= trials.len() * 8;
    println!(
        "\n  E1: {right}/{} right ({:.0}%; target at least 8 in 10) -> {}",
        trials.len(),
        100.0 * right as f64 / trials.len() as f64,
        if met { "met" } else { "NOT MET" }
    );
    Ok(met)
}
