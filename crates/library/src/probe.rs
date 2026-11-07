//! What FFprobe says about a file: duration, codecs, size, audio languages, and the
//! container's own title. Run only for the files identification is not sure about.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use sinephile_persistence::repositories::Probe;

#[derive(Deserialize)]
struct Output {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Format>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    #[serde(default)]
    tags: Tags,
}

#[derive(Deserialize, Default)]
struct Tags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
    #[serde(default)]
    tags: Tags,
}

/// Run FFprobe on `path`. `None` when it is missing or cannot read the file.
pub fn ffprobe(path: &Path) -> Option<Probe> {
    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(path);
    no_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&out.stdout)
}

/// A console program started from a windowed app opens a console window of its own
/// unless told not to; the release build has no console to share.
pub fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// FFprobe's JSON to a `Probe`.
pub fn parse(json: &[u8]) -> Option<Probe> {
    let o: Output = serde_json::from_slice(json).ok()?;
    let video = o
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));
    let audio: Vec<&Stream> = o
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .collect();
    let mut languages: Vec<String> = Vec::new();
    for a in &audio {
        if let Some(code) = a.tags.language.as_deref().and_then(two_letter) {
            if !languages.iter().any(|l| l == code) {
                languages.push(code.to_string());
            }
        }
    }
    let format = o.format.as_ref();
    Some(Probe {
        duration_seconds: format
            .and_then(|f| f.duration.as_deref())
            .and_then(|d| d.parse::<f64>().ok())
            .map(|d| d.round() as i64),
        video_codec: video.and_then(|v| v.codec_name.clone()),
        audio_codec: audio.first().and_then(|a| a.codec_name.clone()),
        width: video.and_then(|v| v.width),
        height: video.and_then(|v| v.height),
        audio_languages: languages,
        container_title: format
            .and_then(|f| f.tags.title.clone())
            .filter(|t| !t.trim().is_empty()),
    })
}

/// ISO 639-2 (what containers carry) to 639-1 (what the catalogue stores). Unknown and
/// undetermined ("und") give nothing.
fn two_letter(code: &str) -> Option<&'static str> {
    Some(match code.to_ascii_lowercase().as_str() {
        "eng" | "en" => "en",
        "fre" | "fra" | "fr" => "fr",
        "ger" | "deu" | "de" => "de",
        "ita" | "it" => "it",
        "spa" | "es" => "es",
        "por" | "pt" => "pt",
        "rus" | "ru" => "ru",
        "jpn" | "ja" => "ja",
        "kor" | "ko" => "ko",
        "chi" | "zho" | "zh" => "zh",
        "hin" | "hi" => "hi",
        "dut" | "nld" | "nl" => "nl",
        "swe" | "sv" => "sv",
        "dan" | "da" => "da",
        "nor" | "nob" | "no" => "no",
        "fin" | "fi" => "fi",
        "pol" | "pl" => "pl",
        "tur" | "tr" => "tr",
        "ara" | "ar" => "ar",
        "heb" | "he" => "he",
        "gre" | "ell" | "el" => "el",
        "cze" | "ces" | "cs" => "cs",
        "hun" | "hu" => "hu",
        "tha" | "th" => "th",
        "vie" | "vi" => "vi",
        "ind" | "id" => "id",
        "ukr" | "uk" => "uk",
        "per" | "fas" | "fa" => "fa",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffprobe_json_becomes_a_probe() {
        let json = br#"{
          "streams": [
            {"codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080},
            {"codec_type": "audio", "codec_name": "aac", "tags": {"language": "jpn"}},
            {"codec_type": "audio", "codec_name": "ac3", "tags": {"language": "eng"}},
            {"codec_type": "audio", "codec_name": "ac3", "tags": {"language": "und"}},
            {"codec_type": "subtitle", "codec_name": "ass", "tags": {"language": "eng"}}
          ],
          "format": {"duration": "5432.512", "tags": {"title": "Heat"}}
        }"#;
        let p = parse(json).expect("parses");
        assert_eq!(p.duration_seconds, Some(5433));
        assert_eq!((p.width, p.height), (Some(1920), Some(1080)));
        assert_eq!(p.video_codec.as_deref(), Some("h264"));
        assert_eq!(p.audio_codec.as_deref(), Some("aac"));
        assert_eq!(p.audio_languages, vec!["ja", "en"]);
        assert_eq!(p.container_title.as_deref(), Some("Heat"));
    }

    #[test]
    fn nonsense_is_none() {
        assert_eq!(parse(b"not json"), None);
    }
}
