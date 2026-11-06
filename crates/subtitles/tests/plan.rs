//! What opens with a film: each branch of `plan`, against real files in a temporary
//! folder and a speech trace made from known lines.

use std::path::{Path, PathBuf};

use sinephile_persistence::repositories::SubtitleRepository;
use sinephile_persistence::Db;
use sinephile_subtitles::opensubtitles::OpenSubtitles;
use sinephile_subtitles::plan::{plan, Alignment, Film, Show, Speech, Track, Wants};
use subtitle_align::cue::{cues, Format};

const DAY: &str = "2026-11-06";

/// Lines of dialogue: (start, end) ms over ten minutes, at irregular gaps as speech is.
/// (Lines every 4 s exactly would be periodic, and any 4 s shift would fit: the aligner
/// rightly refuses that.)
fn lines() -> Vec<(i64, i64)> {
    let mut seed = 7u64;
    let mut next = |n: i64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 33) as i64 % n
    };
    let mut t = 5_000;
    let mut out = Vec::new();
    while t < 570_000 {
        let len = 700 + next(3_000);
        out.push((t, t + len));
        let long = next(8) == 0;
        t += len + 300 + next(if long { 9_000 } else { 2_000 });
    }
    out
}

struct Trace(Vec<bool>);
impl Speech for Trace {
    fn speech(&self) -> Option<Vec<bool>> {
        Some(self.0.clone())
    }
}

fn trace() -> Trace {
    let mut v = vec![false; 60_000];
    for (a, b) in lines() {
        v[(a / 10) as usize..(b / 10) as usize]
            .iter_mut()
            .for_each(|x| *x = true);
    }
    Trace(v)
}

fn srt(shift: i64, scale: f64) -> String {
    let f = |ms: i64| {
        let ms = (ms as f64 * scale) as i64 + shift;
        format!(
            "{:02}:{:02}:{:02},{:03}",
            ms / 3_600_000,
            ms / 60_000 % 60,
            ms / 1000 % 60,
            ms % 1000
        )
    };
    lines()
        .iter()
        .enumerate()
        .map(|(i, &(a, b))| format!("{}\n{} --> {}\nLine {i}\n\n", i + 1, f(a - 100), f(b + 400)))
        .collect()
}

/// A folder with a "film" (bytes enough to hash) and an English subtitle beside it.
fn folder(subtitle: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let film = dir.path().join("Sunrise.1927.mkv");
    std::fs::write(
        &film,
        (0..300_000u32).map(|i| (i % 253) as u8).collect::<Vec<_>>(),
    )
    .unwrap();
    std::fs::write(dir.path().join("Sunrise.1927.en.srt"), subtitle).unwrap();
    (dir, film)
}

fn wants() -> Wants {
    Wants {
        languages: vec!["en".into()],
        always: false,
    }
}

fn film<'a>(path: &'a Path, audio: Option<&'a str>, tracks: Vec<Track>) -> Film<'a> {
    Film {
        target: path.to_str().unwrap(),
        local: Some(path),
        audio_language: audio,
        tracks,
        torrent: vec![],
    }
}

async fn run(
    film: &Film<'_>,
    wants: &Wants,
    alignment: Alignment<'_>,
    data: &Path,
    db: &Db,
) -> sinephile_subtitles::plan::Plan {
    plan::<OpenSubtitles>(db, data, None, film, wants, alignment, DAY)
        .await
        .unwrap()
}

#[tokio::test]
async fn audio_in_your_language_needs_no_subtitles_unless_asked() {
    let (dir, path) = folder(&srt(0, 1.0));
    let db = Db::in_memory().await.unwrap();
    let f = film(&path, Some("eng"), vec![]);
    let p = run(&f, &wants(), Alignment::Duration(None), dir.path(), &db).await;
    assert!(matches!(p.show, Show::Off { .. }), "{:?}", p.show);
    let always = Wants {
        always: true,
        ..wants()
    };
    let p = run(&f, &always, Alignment::Duration(None), dir.path(), &db).await;
    assert!(matches!(p.show, Show::File { .. }), "{:?}", p.show);
}

#[tokio::test]
async fn an_embedded_track_in_your_language_is_shown_as_it_is() {
    let (dir, path) = folder(&srt(0, 1.0));
    let db = Db::in_memory().await.unwrap();
    let tracks = vec![
        Track {
            id: 2,
            language: Some("fra".into()),
            forced: false,
        },
        Track {
            id: 3,
            language: Some("eng".into()),
            forced: false,
        },
    ];
    let f = film(&path, Some("ja"), tracks);
    let p = run(&f, &wants(), Alignment::Speech(&trace()), dir.path(), &db).await;
    assert_eq!(
        p.show,
        Show::Embedded { track_id: 3 },
        "embedded beats the file beside it"
    );
}

#[tokio::test]
async fn a_file_beside_the_film_is_aligned_to_its_speech() {
    let (dir, path) = folder(&srt(6_500, 1.0));
    let db = Db::in_memory().await.unwrap();
    let f = film(&path, Some("ja"), vec![]);
    let p = run(&f, &wants(), Alignment::Speech(&trace()), dir.path(), &db).await;
    let Show::File {
        path: out,
        note,
        language,
    } = &p.show
    else {
        panic!("{:?}", p.show)
    };
    assert!(note.starts_with("aligned"), "{note}");
    assert_eq!(language.as_deref(), Some("en"));
    assert!(out.starts_with(dir.path().join("cache").join("subtitles").join("aligned")));
    let fixed = cues(&std::fs::read_to_string(out).unwrap(), Format::Srt);
    let truth = cues(&srt(0, 1.0), Format::Srt);
    let landed = fixed
        .iter()
        .zip(&truth)
        .filter(|(a, b)| (a.start - b.start).abs() <= 150)
        .count();
    assert!(
        landed * 100 >= truth.len() * 95,
        "{landed}/{} ({note})",
        truth.len()
    );
    // The original is untouched.
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Sunrise.1927.en.srt")).unwrap(),
        srt(6_500, 1.0)
    );
}

#[tokio::test]
async fn a_subtitle_for_other_speech_is_left_alone() {
    let (dir, path) = folder(&srt(0, 1.0));
    let db = Db::in_memory().await.unwrap();
    let f = film(&path, None, vec![]);
    // Speech nowhere near the subtitles' rhythm.
    let mut v = vec![false; 60_000];
    v.iter_mut()
        .enumerate()
        .for_each(|(i, x)| *x = (i / 700) % 5 == 0);
    let p = run(&f, &wants(), Alignment::Speech(&Trace(v)), dir.path(), &db).await;
    let Show::File {
        path: out, note, ..
    } = &p.show
    else {
        panic!()
    };
    assert!(note.starts_with("not aligned"), "{note}");
    assert_eq!(out, &dir.path().join("Sunrise.1927.en.srt"));
}

#[tokio::test]
async fn without_speech_the_films_length_fixes_a_framerate() {
    // Tier 0: subtitles timed 4.3% long run past the end of a 10-minute film.
    let (dir, path) = folder(&srt(0, 25.0 / 23.976));
    let db = Db::in_memory().await.unwrap();
    let f = film(&path, None, vec![]);
    let p = run(
        &f,
        &wants(),
        Alignment::Duration(Some(575_000)),
        dir.path(),
        &db,
    )
    .await;
    let Show::File { note, .. } = &p.show else {
        panic!("{:?}", p.show)
    };
    assert!(note.starts_with("scaled by 0.9"), "{note}");
}

#[tokio::test]
async fn the_nudge_from_last_time_comes_back() {
    let (dir, path) = folder(&srt(0, 1.0));
    let db = Db::in_memory().await.unwrap();
    let f = film(&path, None, vec![]);
    let first = run(&f, &wants(), Alignment::Duration(None), dir.path(), &db).await;
    assert_eq!(first.nudge_ms, 0);
    SubtitleRepository::new(&db)
        .set_nudge(
            &first.file_key,
            first.subtitle_key.as_deref().unwrap(),
            -400,
        )
        .await
        .unwrap();
    let again = run(&f, &wants(), Alignment::Duration(None), dir.path(), &db).await;
    assert_eq!(again.nudge_ms, -400);
    assert_eq!(
        first.file_key.len(),
        16,
        "keyed by the file's hash, so a move keeps it"
    );
}

#[test]
fn ffmpeg_hears_a_real_file() {
    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("tone.mkv");
    let ok = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=500:duration=4",
            "-c:a",
            "aac",
        ])
        .arg(&clip)
        .status()
        .expect("FFmpeg must be on PATH")
        .success();
    assert!(ok);
    let trace =
        sinephile_subtitles::plan::ffmpeg_speech(clip.to_str().unwrap(), 600).expect("a trace");
    assert!(
        (395..=405).contains(&trace.len()),
        "4 s is 400 frames: {}",
        trace.len()
    );
    assert!(sinephile_subtitles::plan::ffmpeg_speech("nonexistent.mkv", 10).is_none());
}
