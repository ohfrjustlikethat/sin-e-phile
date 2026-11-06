//! Phase 8 E2: "Playback of 4K HEVC HDR uses hardware decode (verified in logs and by
//! CPU usage)." Plays a file twice, headless — hardware decoding allowed, then forced
//! off — and reports the decoder mpv chose and this process's CPU time per second of
//! playback for each.
//!
//! ```text
//! cargo run --release -p sinephile-player --example hwdec -- <4k-hevc-hdr-file> [seconds]
//! ```

use std::time::{Duration, Instant};

use sinephile_player::{dll_path, Options, Player};

/// CPU time used by this process so far (user + kernel).
fn cpu_time() -> Duration {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u)
            .expect("GetProcessTimes");
    }
    let t = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    Duration::from_nanos((t(k) + t(u)) * 100)
}

fn run(file: &str, hwdec: bool, seconds: u64) {
    let p = Player::new(
        &dll_path(),
        &Options {
            headless: true,
            hwdec,
            ..Default::default()
        },
    )
    .expect("player");
    p.load(file, None).expect("load");
    p.wait_first_frame(Duration::from_secs(30))
        .expect("first frame");
    std::thread::sleep(Duration::from_secs(1));
    let s = p.state();
    let (cpu0, t0) = (cpu_time(), Instant::now());
    let pos0 = s.position.unwrap_or(0.0);
    std::thread::sleep(Duration::from_secs(seconds));
    let (cpu, wall) = (cpu_time() - cpu0, t0.elapsed());
    let played = p.state().position.unwrap_or(0.0) - pos0;
    println!(
        "hwdec {:>5}: decoder {:<12} cpu {:>6.1}% of one core   played {:.1} s in {:.1} s",
        if hwdec { "on" } else { "off" },
        s.hwdec.clone().unwrap_or_else(|| "software".into()),
        cpu.as_secs_f64() / wall.as_secs_f64() * 100.0,
        played,
        wall.as_secs_f64()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let file = args.first().expect("a file");
    let seconds = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    run(file, true, seconds);
    run(file, false, seconds);
}
