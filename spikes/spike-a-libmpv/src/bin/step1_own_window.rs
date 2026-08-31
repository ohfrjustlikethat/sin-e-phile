//! Spike A, step 1 — can Rust drive libmpv at all on this machine?
//!
//! Baseline before any embedding is attempted. If this fails, R1 is not about
//! Tauri at all and the whole spike changes shape.
//!
//! Usage: step1-own-window <libmpv-2.dll> <video>

use spike_a_libmpv::mpv::*;
use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err("usage: step1-own-window <libmpv-2.dll> <video>".into());
    }
    let (dll, video) = (&args[1], &args[2]);

    let t0 = Instant::now();
    let mpv = Mpv::load(std::path::Path::new(dll))?;
    println!("load+create      {:>7.1} ms", t0.elapsed().as_secs_f64() * 1000.0);

    mpv.set_option("terminal", "no")?;
    mpv.set_option("hwdec", "auto-safe")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.request_log_messages("info")?;

    let t1 = Instant::now();
    mpv.initialize()?;
    println!("initialize       {:>7.1} ms", t1.elapsed().as_secs_f64() * 1000.0);

    let t2 = Instant::now();
    mpv.command(&["loadfile", video])?;

    let mut first_frame: Option<f64> = None;
    let deadline = Instant::now() + std::time::Duration::from_secs(20);

    while Instant::now() < deadline {
        let (id, text) = mpv.wait_event(0.2);
        if let Some(t) = text {
            println!("  mpv {t}");
        }
        match id {
            MPV_EVENT_FILE_LOADED => {
                println!("file-loaded      {:>7.1} ms", t2.elapsed().as_secs_f64() * 1000.0)
            }
            MPV_EVENT_PLAYBACK_RESTART if first_frame.is_none() => {
                let ms = t2.elapsed().as_secs_f64() * 1000.0;
                first_frame = Some(ms);
                println!("FIRST FRAME      {ms:>7.1} ms");
                println!("  hwdec-current  {:?}", mpv.get_property("hwdec-current"));
                println!("  video-codec    {:?}", mpv.get_property("video-codec"));
                println!("  dimensions     {:?}x{:?}",
                    mpv.get_property("width"), mpv.get_property("height"));
                // Let it run briefly to prove it is really playing, then stop.
                std::thread::sleep(std::time::Duration::from_millis(1500));
                println!("  time-pos       {:?}", mpv.get_property("time-pos"));
                break;
            }
            MPV_EVENT_SHUTDOWN => break,
            _ => {}
        }
    }

    match first_frame {
        Some(ms) => {
            println!("\nSTEP 1 PASS - libmpv renders from Rust. First frame {ms:.1} ms.");
            Ok(())
        }
        None => Err("STEP 1 FAIL - no playback within 20 s".into()),
    }
}
