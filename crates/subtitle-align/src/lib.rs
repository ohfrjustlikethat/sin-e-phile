//! Align subtitles to speech — SPEC.md Phase 10.
//!
//! Three pieces, usable on their own:
//!
//! - [`cue`]: read timings from SRT, WebVTT and ASS/SSA, and rewrite them in place.
//! - [`vad`]: which 10 ms frames of mono PCM are speech.
//! - [`align`](mod@align): the offset and framerate scale that put the subtitles over the
//!   speech, with a confidence, or a refusal.
//!
//! [`align_text`] does all three for the common case. This crate never runs FFmpeg or
//! touches a file (ADR-0017): audio comes in as samples, subtitles as text.
//!
//! The README's example is compiled and run as a test:
#![doc = include_str!("../README.md")]

pub mod align;
pub mod cue;
pub mod vad;

pub use align::{Alignment, Options, Refusal, Split, SCALES};
pub use cue::{Cue, Format};

/// Align a subtitle file's text to a speech trace and return the corrected text, with the
/// alignment applied. Unreadable text or an unconfident match leaves it unchanged, and
/// says why.
pub fn align_text(
    text: &str,
    speech: &[bool],
    opts: &Options,
) -> Result<(String, Alignment), Refusal> {
    let format = cue::detect(text).ok_or(Refusal::NoCues)?;
    let cues = cue::cues(text, format);
    let a = align::align(speech, &cues, opts)?;
    Ok((cue::retime(text, format, |t| a.map(t)), a))
}

/// Without audio (Tier 0, SPEC.md §8): the framerate scale that makes the last subtitle end
/// just before the film does. `None` unless one scale clearly fits better than none.
pub fn scale_from_duration(cues: &[Cue], media_ms: i64) -> Option<f64> {
    let last = cues.iter().map(|c| c.end).max()?;
    if last <= 0 || media_ms <= 0 {
        return None;
    }
    // Subtitles usually end within the last few minutes; credits run on after them.
    let fits = |s: f64| {
        let end = last as f64 * s;
        end <= media_ms as f64 && end >= media_ms as f64 - 8.0 * 60_000.0
    };
    if fits(1.0) {
        return None;
    }
    SCALES
        .iter()
        .copied()
        .filter(|&s| s != 1.0 && fits(s))
        .min_by(|a, b| (a - 1.0).abs().total_cmp(&(b - 1.0).abs()))
}
