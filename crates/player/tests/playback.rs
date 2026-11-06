//! The player against a real clip, headless (no window, no sound card needed).
//!
//! Needs `lib/libmpv-2.dll` (`python tools/mpv/fetch.py`) and FFmpeg on PATH, as CI
//! provides. Missing either is a failure with instructions, never a silent skip.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use sinephile_player::{dll_path, Options, Player};

/// 6 s, 640×360 at 25 fps, two audio tracks (eng, fra) and one subtitle track (eng).
fn clip(dir: &Path) -> PathBuf {
    let srt = dir.join("subs.srt");
    std::fs::write(&srt, "1\n00:00:00,500 --> 00:00:05,000\nA test subtitle.\n").unwrap();
    let out = dir.join("clip.mkv");
    let status = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=25:duration=6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=660:duration=6",
            "-i",
        ])
        .arg(&srt)
        .args([
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-map",
            "2:a",
            "-map",
            "3:s",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "25",
            "-c:a",
            "aac",
            "-c:s",
            "srt",
            "-metadata:s:a:0",
            "language=eng",
            "-metadata:s:a:1",
            "language=fra",
            "-metadata:s:s:0",
            "language=eng",
        ])
        .arg(&out)
        .status()
        .expect("FFmpeg must be on PATH to build the test clip");
    assert!(status.success(), "ffmpeg failed");
    out
}

fn player() -> Player {
    let dll = dll_path();
    assert!(
        dll.is_file(),
        "{} is missing: run `python tools/mpv/fetch.py`",
        dll.display()
    );
    Player::new(
        &dll,
        &Options {
            headless: true,
            ..Default::default()
        },
    )
    .expect("player")
}

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_clip_loads_plays_and_reports_its_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    p.load(clip(dir.path()).to_str().unwrap(), None).unwrap();
    p.wait_loaded(Duration::from_secs(10)).unwrap();
    let first = p.wait_first_frame(Duration::from_secs(10)).unwrap();
    println!("first frame (headless, local): {first:?}");

    let s = p.state();
    assert!((s.duration.unwrap() - 6.0).abs() < 0.1, "{s:?}");
    wait_until("time to advance", || {
        p.state().position.unwrap_or(0.0) > 0.3
    });

    let tracks = p.tracks();
    let audio: Vec<_> = tracks.iter().filter(|t| t.kind == "audio").collect();
    let subs: Vec<_> = tracks.iter().filter(|t| t.kind == "sub").collect();
    assert_eq!(audio.len(), 2, "{tracks:?}");
    assert_eq!(audio[1].lang.as_deref(), Some("fra"));
    assert_eq!(subs.len(), 1);

    p.set_audio(audio[1].id).unwrap();
    wait_until("the French track to be selected", || {
        p.tracks()
            .iter()
            .any(|t| t.kind == "audio" && t.lang.as_deref() == Some("fra") && t.selected)
    });
    p.set_subtitles(None).unwrap();
    wait_until("subtitles off", || {
        !p.tracks().iter().any(|t| t.kind == "sub" && t.selected)
    });
}

#[test]
fn a_subtitle_beside_the_file_is_not_loaded_behind_the_chains_back() {
    // mpv's own sub-auto would load it unaligned; the chain must be the only one.
    let dir = tempfile::tempdir().unwrap();
    let film = clip(dir.path());
    std::fs::write(
        film.with_extension("en.srt"),
        "1
00:00:01,000 --> 00:00:02,000
Beside.
",
    )
    .unwrap();
    let p = player();
    p.load(film.to_str().unwrap(), None).unwrap();
    p.wait_first_frame(Duration::from_secs(10)).unwrap();
    let subs = p.tracks().into_iter().filter(|t| t.kind == "sub").count();
    assert_eq!(subs, 1, "only the embedded track");
}

#[test]
fn an_outside_subtitle_loads_shifts_and_keeps_its_style() {
    // SPEC.md Phase 10: a file found beside the film or downloaded, loaded and selected;
    // the nudge moves it; the style is accepted.
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    p.load(clip(dir.path()).to_str().unwrap(), None).unwrap();
    p.wait_first_frame(Duration::from_secs(10)).unwrap();
    let outside = dir.path().join("outside.fr.srt");
    std::fs::write(
        &outside,
        "1
00:00:01,000 --> 00:00:04,000
Un sous-titre.
",
    )
    .unwrap();
    p.add_subtitle(outside.to_str().unwrap(), "Downloaded", Some("fr"))
        .unwrap();
    wait_until("the outside subtitle to be selected", || {
        p.tracks()
            .iter()
            .any(|t| t.kind == "sub" && t.selected && t.lang.as_deref() == Some("fr"))
    });
    assert_eq!(p.tracks().iter().filter(|t| t.kind == "sub").count(), 2);
    let subs: Vec<_> = p.tracks().into_iter().filter(|t| t.kind == "sub").collect();
    assert_eq!(
        subs.iter().map(|t| t.external).collect::<Vec<_>>(),
        [false, true],
        "the embedded one and the added file are told apart"
    );
    p.set_subtitle_delay(-0.3).unwrap();
    assert!((p.subtitle_delay() + 0.3).abs() < 1e-6);
    p.set_subtitle_style(&sinephile_player::SubtitleStyle {
        background: true,
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn an_exact_seek_lands_on_the_frame_asked_for() {
    // Phase 8 E3: "Seeking is frame-accurate and responsive."
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    p.load(clip(dir.path()).to_str().unwrap(), None).unwrap();
    p.wait_loaded(Duration::from_secs(10)).unwrap();
    // Seeking before playback has started can be superseded by the initial start; a
    // player only offers seeking once the first frame is up, so the test does the same.
    p.wait_first_frame(Duration::from_secs(10)).unwrap();
    p.set_paused(true).unwrap();
    // 3.52 s is between keyframes (one per second), so a keyframe seek would miss it.
    for target in [3.52, 1.08, 5.2] {
        let start = Instant::now();
        p.seek_to_and_wait(target, Duration::from_secs(5)).unwrap();
        let took = start.elapsed();
        let at = p.state().position.unwrap();
        println!("seek to {target}: landed at {at:.3} in {took:?}");
        assert!(
            (at - target).abs() < 1.0 / 25.0,
            "landed at {at}, wanted {target}"
        );
        assert!(took < Duration::from_millis(500), "seek took {took:?}");
    }
    p.frame_step(true).unwrap();
    wait_until("one frame forward", || {
        p.state().position.is_some_and(|t| (t - 5.24).abs() < 0.02)
    });
}

#[test]
fn speed_volume_and_pause_take_effect() {
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    p.load(clip(dir.path()).to_str().unwrap(), None).unwrap();
    p.wait_loaded(Duration::from_secs(10)).unwrap();
    p.set_speed(1.5).unwrap();
    p.set_volume(40.0).unwrap();
    p.set_muted(true).unwrap();
    p.set_paused(true).unwrap();
    wait_until("the settings", || {
        let s = p.state();
        s.paused && s.muted && (s.speed - 1.5).abs() < 1e-6 && (s.volume - 40.0).abs() < 1e-6
    });
}

#[test]
fn resuming_starts_where_it_left_off() {
    // Phase 8 E4's mechanism: a load with a start position.
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    p.load(clip(dir.path()).to_str().unwrap(), Some(4.0))
        .unwrap();
    p.wait_loaded(Duration::from_secs(10)).unwrap();
    p.set_paused(true).unwrap();
    wait_until("the start position", || {
        p.state().position.is_some_and(|t| t >= 3.95)
    });
}

#[test]
fn a_file_that_cannot_be_opened_is_an_error_not_a_hang() {
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    let missing = dir.path().join("not-here.mkv");
    p.load(missing.to_str().unwrap(), None).unwrap();
    let err = p.wait_loaded(Duration::from_secs(10)).unwrap_err();
    assert!(err.to_string().starts_with("could not play this"), "{err}");
}

#[test]
fn a_still_frame_can_be_saved_for_the_pause_overlay() {
    let dir = tempfile::tempdir().unwrap();
    let p = player();
    p.load(clip(dir.path()).to_str().unwrap(), None).unwrap();
    p.wait_first_frame(Duration::from_secs(10)).unwrap();
    p.set_paused(true).unwrap();
    let jpeg = p
        .capture_still(&dir.path().join("still.jpg"), Duration::from_secs(5))
        .unwrap();
    assert!(sinephile_player::still::is_complete_jpeg(&jpeg));
    assert!(jpeg.len() > 1000);
}
