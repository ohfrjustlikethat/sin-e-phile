//! Is this copy what it says it is? SPEC.md Phase 9: "a 700 MB '4K' file is a lie; a
//! 60-minute file for a 140-minute film is a lie."
//!
//! Each check returns a term and, when the copy is a lie, the reason it is sunk.
//!
//! "Release-group reputation" is done with **structural signals only**: whether a name
//! is internally consistent and states how it was made. No group, site or indexer is
//! named anywhere (SPEC.md §2.1): a list of names would be a catalogue of where to find
//! things, and it would rot in a month anyway.

use source_protocol::{Resolution, SourceCandidate, VideoCodec};

use crate::{resolution_label, weight, Context, Term};

/// The least an honest encode needs, in megabytes per minute, for H.264 and MPEG-4.
/// Deliberately low — the point is to catch lies, not to judge thin encodes. 1 MB/min is
/// about 130 kbit/s, below anything watchable even at SD; 12 MB/min is about 1.6 Mbit/s,
/// a third of what the leanest real 4K streams use.
fn floor_mb_per_minute(r: Resolution) -> Option<f64> {
    Some(match r {
        Resolution::Sd => 1.0,
        Resolution::Hd => 3.0,
        Resolution::FullHd => 5.0,
        Resolution::Uhd => 12.0,
        Resolution::Unknown => return None,
    })
}

/// H.265, AV1 and VP9 need roughly 60% of H.264's bits for the same picture. An unknown
/// codec gets the same allowance: the lenient assumption, so it is never a false fake.
fn codec_factor(c: VideoCodec) -> f64 {
    match c {
        VideoCodec::H264 | VideoCodec::Mpeg4 => 1.0,
        _ => 0.6,
    }
}

/// How long it plays, in seconds: its own duration if stated, else the catalogue's.
fn seconds(c: &SourceCandidate, ctx: &Context) -> Option<f64> {
    c.duration_seconds
        .map(f64::from)
        .or(ctx.runtime_minutes.map(|m| f64::from(m) * 60.0))
        .filter(|&s| s > 0.0)
}

/// The average rate the file must arrive at to play without stopping.
pub fn bytes_per_second(c: &SourceCandidate, ctx: &Context) -> Option<f64> {
    Some(c.size_bytes? as f64 / seconds(c, ctx)?)
}

/// Size against claimed quality: too few bytes for the resolution is a lie.
pub fn bitrate(c: &SourceCandidate, ctx: &Context) -> (Term, Option<String>) {
    let w = weight::PLAUSIBILITY;
    let res = c.quality.resolution;
    let (Some(floor), Some(rate)) = (floor_mb_per_minute(res), bytes_per_second(c, ctx)) else {
        return (
            Term::new("plausibility", 0.0, w, "size or length not stated"),
            None,
        );
    };
    let floor = floor * codec_factor(c.codec);
    let mb_per_minute = rate * 60.0 / 1e6;
    let label = resolution_label(res);
    if mb_per_minute < floor {
        let why = format!(
            "{:.0} MB for a {label} file of {:.0} minutes is too small to be {label}",
            c.size_bytes.unwrap_or(0) as f64 / 1e6,
            seconds(c, ctx).unwrap_or(0.0) / 60.0
        );
        return (Term::new("plausibility", -1.0, w, why.clone()), Some(why));
    }
    let why = format!("{mb_per_minute:.1} MB a minute, plausible for {label}");
    let value = if mb_per_minute < 2.0 * floor {
        0.0
    } else {
        1.0
    };
    (Term::new("plausibility", value, w, why), None)
}

/// Length against the catalogue's runtime. Shorter than 80% is a different thing — a
/// trailer, an episode, a cut-down copy. Up to 30% longer is an extended cut or extras.
pub fn duration(c: &SourceCandidate, ctx: &Context) -> (Term, Option<String>) {
    let w = weight::DURATION;
    let (Some(have), Some(runtime)) = (c.duration_seconds, ctx.runtime_minutes) else {
        return (Term::new("duration", 0.0, w, "length not stated"), None);
    };
    let have = f64::from(have) / 60.0;
    let runtime = f64::from(runtime);
    if runtime <= 0.0 {
        return (Term::new("duration", 0.0, w, "runtime unknown"), None);
    }
    let ratio = have / runtime;
    if ratio < 0.8 {
        let why = format!("plays {have:.0} minutes; this runs {runtime:.0}");
        return (Term::new("duration", -1.0, w, why.clone()), Some(why));
    }
    if ratio > 1.3 {
        let why = format!("plays {have:.0} minutes, longer than the {runtime:.0} listed");
        return (Term::new("duration", 0.0, w, why), None);
    }
    (
        Term::new(
            "duration",
            1.0,
            w,
            format!("plays {have:.0} minutes, as listed"),
        ),
        None,
    )
}

/// Extensions that are never a film. A "film" named `.exe` is the oldest trick there is.
/// Not `.com`: too many titles end in it.
const NOT_VIDEO: [&str; 9] = ["exe", "scr", "bat", "cmd", "msi", "lnk", "zip", "rar", "7z"];
/// Tags meaning the picture was filmed off a cinema screen.
const CAMERA: [&str; 8] = [
    "cam", "camrip", "hdcam", "ts", "hdts", "telesync", "tc", "telecine",
];
/// Tags naming where the picture came from.
const SOURCE: [&str; 9] = [
    "bluray", "bdrip", "brrip", "remux", "webdl", "webrip", "web", "hdtv", "dvdrip",
];
/// Tags naming the encoder or codec.
const ENCODE: [&str; 6] = ["x264", "x265", "h264", "h265", "hevc", "av1"];

fn tokens(s: &str) -> Vec<String> {
    s.to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn extension(s: &str) -> Option<String> {
    let last = s.rsplit(['/', '\\']).next()?;
    let (_, ext) = last.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

fn named_resolution(t: &str) -> Option<Resolution> {
    Some(match t {
        "480p" | "576p" => Resolution::Sd,
        "720p" => Resolution::Hd,
        "1080p" | "1080i" => Resolution::FullHd,
        "2160p" | "4k" | "uhd" => Resolution::Uhd,
        _ => return None,
    })
}

/// The name's structure: is it a video, is it consistent with what is claimed, and does
/// it say how it was made?
pub fn encoding(c: &SourceCandidate, _ctx: &Context) -> (Term, Option<String>) {
    let w = weight::ENCODING;
    for name in [&c.title, &c.key] {
        if let Some(ext) = extension(name).filter(|e| NOT_VIDEO.contains(&e.as_str())) {
            let why = format!("not a video file (.{ext})");
            return (Term::new("encoding", -1.0, w, why.clone()), Some(why));
        }
    }
    // The title only: a key is opaque, and `.ts` is a real video container.
    let words = tokens(&c.title);
    let claimed = c.quality.resolution;
    if let Some(named) = words.iter().find_map(|t| named_resolution(t)) {
        if claimed != Resolution::Unknown && named != claimed {
            let why = format!(
                "named {}, listed as {}",
                resolution_label(named),
                resolution_label(claimed)
            );
            return (Term::new("encoding", -1.0, w, why.clone()), Some(why));
        }
    }
    if words.iter().any(|t| CAMERA.contains(&t.as_str())) {
        return (
            Term::new("encoding", -1.0, w, "filmed off a cinema screen"),
            None,
        );
    }
    let source = words.iter().any(|t| SOURCE.contains(&t.as_str()));
    let encode = words.iter().any(|t| ENCODE.contains(&t.as_str()));
    match (source, encode) {
        (true, true) => (
            Term::new("encoding", 1.0, w, "states its source and encode"),
            None,
        ),
        (true, false) | (false, true) => (
            Term::new("encoding", 0.5, w, "states how it was made"),
            None,
        ),
        (false, false) => (
            Term::new("encoding", 0.0, w, "no encode details in the name"),
            None,
        ),
    }
}
