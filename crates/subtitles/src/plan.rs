//! What subtitle to show when a film opens, ready to hand to the player (SPEC.md Phase 10:
//! "Subtitles are in sync on the first frame, always, with no user action").
//!
//! 1. **Whether at all.** With the film's audio in one of the viewer's languages, none —
//!    unless they asked for subtitles always. A foreign film gets them without asking.
//! 2. **Which** — [`crate::fetch::pick`]: embedded first, then files, then a download.
//! 3. **In step** — an embedded track already is. Anything else is aligned to the film's
//!    speech where the tier allows it (§8: Tier 1+), else scaled by the film's length
//!    (Tier 0), and written to `cache/subtitles/aligned/` so the original is untouched.
//! 4. **The viewer's nudge** for this file and this subtitle, from last time.

use std::path::{Path, PathBuf};

use sinephile_persistence::repositories::SubtitleRepository;
use sinephile_persistence::Db;
use subtitle_align::{align_text, cue, scale_from_duration, Options};

use crate::fetch::{pick, FetchError, Provider};
use crate::{files, hash, Found, Origin};

/// An embedded track, as the player lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub language: Option<String>,
    pub forced: bool,
}

/// What the viewer wants.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wants {
    /// Subtitle languages, most preferred first (BCP 47 primary subtags).
    pub languages: Vec<String>,
    /// Show subtitles even when the audio is in one of `languages`.
    pub always: bool,
}

/// How a subtitle that is not embedded is put in step.
pub enum Alignment<'a> {
    /// Against the film's speech (Tier 1+).
    Speech(&'a dyn Speech),
    /// By the film's length, in milliseconds (Tier 0).
    Duration(Option<i64>),
}

/// Where the film's speech comes from: FFmpeg in the app ([`ffmpeg_speech`]), a
/// synthetic trace in tests. Called only when a subtitle needs aligning.
pub trait Speech: Sync {
    fn speech(&self) -> Option<Vec<bool>>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Show {
    Off {
        why: String,
    },
    Embedded {
        track_id: i64,
    },
    File {
        path: PathBuf,
        language: Option<String>,
        /// What alignment did, in words, for the log and the expert eye.
        note: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub show: Show,
    /// The file's identity for nudges: its hash, else its address.
    pub file_key: String,
    /// Which subtitle, for nudges.
    pub subtitle_key: Option<String>,
    /// The saved nudge, in milliseconds (positive: later).
    pub nudge_ms: i64,
}

/// Everything known about the film when it opens.
pub struct Film<'a> {
    /// What the player opened: a path or a URL.
    pub target: &'a str,
    /// The file on disk, if it is one (for its folder and its hash).
    pub local: Option<&'a Path>,
    /// The selected audio track's language.
    pub audio_language: Option<&'a str>,
    pub tracks: Vec<Track>,
    /// Subtitle files in its torrent: (index, path), already fetched to `local_copy`.
    pub torrent: Vec<(u32, String)>,
}

#[allow(clippy::too_many_arguments)]
pub async fn plan<P: Provider>(
    db: &Db,
    data: &Path,
    provider: Option<&P>,
    film: &Film<'_>,
    wants: &Wants,
    alignment: Alignment<'_>,
    today: &str,
) -> Result<Plan, FetchError> {
    let hash = film.local.and_then(|p| hash::of_file(p).ok().flatten());
    let file_key = hash.clone().unwrap_or_else(|| film.target.to_string());
    let off = |why: &str| Plan {
        show: Show::Off { why: why.into() },
        file_key: file_key.clone(),
        subtitle_key: None,
        nudge_ms: 0,
    };

    if let Some(audio) = film.audio_language {
        if !wants.always && wants.languages.iter().any(|l| same(l, audio)) {
            return Ok(off("the audio is in your language"));
        }
    }

    let mut local: Vec<Found> = film
        .tracks
        .iter()
        .map(|t| Found {
            origin: Origin::Embedded { track_id: t.id },
            language: t.language.as_deref().map(primary),
            forced: t.forced,
        })
        .collect();
    if let Some(path) = film.local {
        let names: Vec<String> = path
            .parent()
            .and_then(|d| std::fs::read_dir(d).ok())
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .collect();
        local.extend(files::beside(&path.to_string_lossy(), &names));
    }
    let Some(chosen) = pick(
        db,
        provider,
        data,
        hash.as_deref(),
        local,
        &wants.languages,
        today,
    )
    .await?
    else {
        return Ok(off("no subtitles in your languages"));
    };
    let subtitle_key = chosen.origin.key();
    let nudge_ms = SubtitleRepository::new(db)
        .nudge(&file_key, &subtitle_key)
        .await?
        .unwrap_or(0);

    let show = match &chosen.origin {
        Origin::Embedded { track_id } => Show::Embedded {
            track_id: *track_id,
        },
        Origin::Sidecar { path } => in_step(data, Path::new(path), &chosen, &file_key, alignment),
        Origin::Downloaded { relative_path, .. } => in_step(
            data,
            &data.join(relative_path),
            &chosen,
            &file_key,
            alignment,
        ),
        Origin::Torrent { .. } => return Ok(off("subtitles in the torrent are not fetched yet")),
    };
    Ok(Plan {
        show,
        file_key,
        subtitle_key: Some(subtitle_key),
        nudge_ms,
    })
}

/// The subtitle at `path`, aligned if it can be, as a file the player can load.
fn in_step(
    data: &Path,
    path: &Path,
    found: &Found,
    file_key: &str,
    alignment: Alignment<'_>,
) -> Show {
    let language = found.language.clone();
    let original = |note: &str| Show::File {
        path: path.to_path_buf(),
        language: language.clone(),
        note: note.into(),
    };
    let Ok(bytes) = std::fs::read(path) else {
        return original("could not be read; loaded as it is");
    };
    let text = decode(&bytes);
    let Some(format) = cue::detect(&text) else {
        return original("format not recognised; loaded as it is");
    };
    let (fixed, note) = match alignment {
        Alignment::Speech(source) => match source.speech() {
            None => return original("no audio to align to; loaded as it is"),
            Some(speech) => match align_text(&text, &speech, &Options::default()) {
                Ok((fixed, a)) => {
                    let split = a
                        .split
                        .map(|s| format!(", then {:+} ms from {} ms", s.offset_ms, s.at_ms))
                        .unwrap_or_default();
                    (
                        fixed,
                        format!(
                            "aligned: scale {:.5}, offset {:+} ms{split}, confidence {:.2}",
                            a.scale, a.offset_ms, a.confidence
                        ),
                    )
                }
                // Refusing is the point: an unconfident shift is worse than none.
                Err(why) => return original(&format!("not aligned ({why}); loaded as it is")),
            },
        },
        Alignment::Duration(media_ms) => {
            let cues = cue::cues(&text, format);
            match media_ms.and_then(|ms| scale_from_duration(&cues, ms)) {
                Some(scale) => (
                    cue::retime(&text, format, |t| (t as f64 * scale).round() as i64),
                    format!("scaled by {scale:.5} to fit the film's length"),
                ),
                None => return original("fits the film's length; loaded as it is"),
            }
        }
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("srt");
    let key = found.origin.key();
    let key = key.strip_suffix(&format!(".{ext}")).unwrap_or(&key);
    let name: String = format!("{file_key}-{key}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let out = data
        .join("cache")
        .join("subtitles")
        .join("aligned")
        .join(format!("{name}.{ext}"));
    let written = out
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&out, fixed));
    match written {
        Ok(()) => Show::File {
            path: out,
            language,
            note,
        },
        Err(e) => original(&format!(
            "could not save the aligned copy ({e}); loaded as it is"
        )),
    }
}

/// Subtitle files are UTF-8 (with or without a BOM), UTF-16 from some editors, or an old
/// single-byte code page; the last is read as Windows-1252's Latin-1 subset, which is right
/// for most Western files and never fails.
fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

fn primary(l: &str) -> String {
    // mpv reports ISO 639-2 ("eng"); files and prefs use 639-1 ("en").
    let l = l.to_ascii_lowercase();
    let two = match l.as_str() {
        "eng" => "en",
        "fre" | "fra" => "fr",
        "ger" | "deu" => "de",
        "spa" => "es",
        "ita" => "it",
        "jpn" => "ja",
        "hin" => "hi",
        "por" => "pt",
        "dut" | "nld" => "nl",
        "kor" => "ko",
        "chi" | "zho" => "zh",
        "rus" => "ru",
        other => other.split(['-', '_']).next().unwrap_or(other),
    };
    two.to_string()
}

fn same(a: &str, b: &str) -> bool {
    primary(a) == primary(b)
}

/// The film's speech from FFmpeg: the first `seconds` of the default audio track, 16 kHz
/// mono, through the speech detector. `None` if FFmpeg cannot read it.
pub fn ffmpeg_speech(target: &str, seconds: u32) -> Option<Vec<bool>> {
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args([
        "-nostdin",
        "-v",
        "error",
        "-i",
        target,
        "-t",
        &seconds.to_string(),
    ])
    .args(["-vn", "-ac", "1", "-ar", "16000", "-f", "s16le", "-"]);
    // A console program started from the windowed release build opens a console window
    // of its own unless told not to (found in Phase 12; thumbs.rs already did this).
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let samples: Vec<f32> = out
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| f32::from(i16::from_le_bytes(*c)) / 32768.0)
        .collect();
    Some(subtitle_align::vad::speech(&samples, 16_000))
}

/// [`ffmpeg_speech`] as a [`Speech`] source.
pub struct Ffmpeg<'a> {
    pub target: &'a str,
    pub seconds: u32,
}

impl Speech for Ffmpeg<'_> {
    fn speech(&self) -> Option<Vec<bool>> {
        ffmpeg_speech(self.target, self.seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_read_whatever_its_encoding() {
        assert_eq!(decode(b"\xEF\xBB\xBFhi"), "hi");
        assert_eq!(decode(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
        assert_eq!(decode(b"caf\xE9"), "café", "Latin-1 never fails");
        assert_eq!(decode("café".as_bytes()), "café");
    }

    #[test]
    fn mpv_languages_meet_file_languages() {
        assert!(same("eng", "en"));
        assert!(same("fra", "fr-CA"));
        assert!(!same("eng", "fr"));
    }
}
