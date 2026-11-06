//! The subtitle-alignment harness (SPEC.md §12.2, Phase 10 E1–E2).
//!
//! `fixtures/subtitles/` holds, per synthetic film, the speech trace the app's detector
//! produced from its audio, the truly timed subtitles, and the times the lines were
//! actually spoken; and eight damaged copies of the subtitles. For each copy the aligner
//! is run against the trace, and every cue it moves is compared with where the true file
//! has it. A copy **aligns** when at least 95% of its cues land within 150 ms. The target
//! is more than 90% of copies.
//!
//! `eval subs --trace <wav> <out>` is the other half: the speech trace of a 16-bit mono WAV,
//! as `tools/subcorpus/build.py` makes the corpus.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use subtitle_align::{cue, vad, Options};

use crate::error::EvalError;

const WITHIN_MS: i64 = 150;
const CUES_THAT_MUST_LAND: f64 = 0.95;
const TARGET: f64 = 0.90;

#[derive(Deserialize)]
struct Manifest {
    films: Vec<Film>,
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Film {
    id: String,
    /// Built after the aligner was tuned, and never tuned on.
    #[serde(default)]
    holdout: bool,
}

#[derive(Deserialize)]
struct Variant {
    id: String,
    film: String,
    kind: String,
}

fn read(path: &Path) -> Result<String, EvalError> {
    std::fs::read_to_string(path).map_err(|source| EvalError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/subtitles")
}

/// A trace on disk: run lengths of alternating silence and speech, silence first.
pub fn parse_trace(text: &str) -> Vec<bool> {
    let mut out = Vec::new();
    let mut speaking = false;
    for n in text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .flat_map(str::split_whitespace)
        .filter_map(|n| n.parse::<usize>().ok())
    {
        out.extend(std::iter::repeat_n(speaking, n));
        speaking = !speaking;
    }
    out
}

fn write_trace(trace: &[bool]) -> String {
    let mut runs = Vec::new();
    let mut current = false;
    let mut n = 0usize;
    for &s in trace {
        if s == current {
            n += 1;
        } else {
            runs.push(n);
            current = s;
            n = 1;
        }
    }
    runs.push(n);
    let mut out = String::from("# speech trace: 10 ms frames, run lengths, silence first\n");
    for chunk in runs.chunks(20) {
        out.push_str(
            &chunk
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(" "),
        );
        out.push('\n');
    }
    out
}

/// `eval subs --trace in.wav out.trace`
pub fn trace(wav: &Path, out: &Path) -> Result<bool, EvalError> {
    let bytes = std::fs::read(wav).map_err(|source| EvalError::Io {
        path: wav.display().to_string(),
        source,
    })?;
    let (rate, samples) = pcm16_mono(&bytes).ok_or_else(|| {
        EvalError::Fixture(wav.display().to_string(), "not 16-bit mono PCM".into())
    })?;
    let speech = vad::speech(&samples, rate);
    std::fs::write(out, write_trace(&speech)).map_err(|source| EvalError::Io {
        path: out.display().to_string(),
        source,
    })?;
    println!(
        "{}: {} frames, {:.1}% speech",
        out.display(),
        speech.len(),
        100.0 * speech.iter().filter(|&&s| s).count() as f64 / speech.len().max(1) as f64
    );
    Ok(true)
}

/// The `fmt ` and `data` chunks of a RIFF WAV, for 16-bit mono only.
fn pcm16_mono(b: &[u8]) -> Option<(u32, Vec<f32>)> {
    if b.get(..4)? != b"RIFF" || b.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut i = 12;
    let mut rate = None;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().ok()?) as usize;
        let body = b.get(i + 8..(i + 8 + len).min(b.len()))?;
        if id == b"fmt " {
            let channels = u16::from_le_bytes(body.get(2..4)?.try_into().ok()?);
            let bits = u16::from_le_bytes(body.get(14..16)?.try_into().ok()?);
            if channels != 1 || bits != 16 {
                return None;
            }
            rate = Some(u32::from_le_bytes(body.get(4..8)?.try_into().ok()?));
        } else if id == b"data" {
            let samples = body
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| f32::from(i16::from_le_bytes(*c)) / 32768.0)
                .collect();
            return Some((rate?, samples));
        }
        i += 8 + len + (len & 1);
    }
    None
}

/// `eval subs`
pub fn run(report: bool, only: Option<&str>) -> Result<bool, EvalError> {
    let dir = corpus();
    let manifest: Manifest = serde_json::from_str(&read(&dir.join("manifest.json"))?)
        .map_err(|e| EvalError::Fixture("manifest.json".into(), e.to_string()))?;
    println!(
        "subs — subtitle alignment over fixtures/subtitles ({} copies)",
        manifest.variants.len()
    );

    // How well the detector heard each film, against when the lines were spoken.
    for film in &manifest.films {
        let trace = parse_trace(&read(
            &dir.join("films").join(format!("{}.trace", film.id)),
        )?);
        let mut truth = vec![false; trace.len()];
        for line in read(&dir.join("films").join(format!("{}.voice.tsv", film.id)))?.lines() {
            let mut it = line.split('\t').filter_map(|n| n.parse::<usize>().ok());
            if let (Some(a), Some(b)) = (it.next(), it.next()) {
                for f in truth
                    .iter_mut()
                    .take((b / 10).min(trace.len()))
                    .skip(a / 10)
                {
                    *f = true;
                }
            }
        }
        let agree = trace.iter().zip(&truth).filter(|(a, b)| a == b).count();
        let found = trace.iter().zip(&truth).filter(|(a, b)| **a && **b).count();
        let spoken = truth.iter().filter(|&&t| t).count().max(1);
        let heard = trace.iter().filter(|&&t| t).count().max(1);
        if report {
            println!(
                "  vad {:<10} agreement {:.1}%   recall {:.1}%   precision {:.1}%",
                film.id,
                100.0 * agree as f64 / trace.len().max(1) as f64,
                100.0 * found as f64 / spoken as f64,
                100.0 * found as f64 / heard as f64,
            );
        }
    }

    let opts = Options::default();
    let mut aligned = 0usize;
    let mut refused = 0usize;
    let mut by_kind: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    // (aligned, copies) for the films tuned on and the films held out.
    let mut tuned = (0usize, 0usize);
    let mut held = (0usize, 0usize);
    let holdout = |film: &str| manifest.films.iter().any(|f| f.id == film && f.holdout);
    for v in manifest
        .variants
        .iter()
        .filter(|v| only.is_none_or(|o| v.id == o))
    {
        let trace = parse_trace(&read(&dir.join("films").join(format!("{}.trace", v.film)))?);
        let truth_text = read(&dir.join("films").join(format!("{}.srt", v.film)))?;
        let bad_text = read(&dir.join("variants").join(format!("{}.srt", v.id)))?;
        let truth = cue::cues(&truth_text, cue::Format::Srt);
        let bad = cue::cues(&bad_text, cue::Format::Srt);
        let (ok, detail) = match subtitle_align::align::align(&trace, &bad, &opts) {
            Ok(a) => {
                let landed = bad
                    .iter()
                    .zip(&truth)
                    .filter(|(b, t)| (a.map(b.start) - t.start).abs() <= WITHIN_MS)
                    .count();
                let share = landed as f64 / truth.len().max(1) as f64;
                let split = a
                    .split
                    .map(|s| format!(", split at {} ms → {:+} ms", s.at_ms, s.offset_ms))
                    .unwrap_or_default();
                (
                    share >= CUES_THAT_MUST_LAND,
                    format!(
                        "{:5.1}% of cues within {WITHIN_MS} ms   scale {:.5} offset {:+} ms{split}   confidence {:.1}",
                        100.0 * share,
                        a.scale,
                        a.offset_ms,
                        a.confidence
                    ),
                )
            }
            Err(r) => {
                refused += 1;
                (false, format!("refused: {r}"))
            }
        };
        aligned += usize::from(ok);
        let set = if holdout(&v.film) {
            &mut held
        } else {
            &mut tuned
        };
        set.0 += usize::from(ok);
        set.1 += 1;
        let e = by_kind.entry(v.kind.clone()).or_default();
        e.0 += usize::from(ok);
        e.1 += 1;
        if report || !ok {
            println!(
                "  {} {:<34} {detail}",
                if ok { "ok  " } else { "MISS" },
                v.id
            );
        }
    }
    // Negative controls: each film's subtitles against the next film's speech. There is
    // nothing to find, and anything but a refusal is a false accept.
    let mut false_accepts = 0usize;
    for (i, film) in manifest.films.iter().enumerate() {
        let other = &manifest.films[(i + 1) % manifest.films.len()];
        let trace = parse_trace(&read(
            &dir.join("films").join(format!("{}.trace", other.id)),
        )?);
        let subs = cue::cues(
            &read(&dir.join("films").join(format!("{}.srt", film.id)))?,
            cue::Format::Srt,
        );
        let outcome = subtitle_align::align::align(&trace, &subs, &opts);
        let accepted = outcome.is_ok();
        false_accepts += usize::from(accepted);
        if report || accepted {
            let detail = match outcome {
                Ok(a) => format!("ACCEPTED (confidence {:.2})", a.confidence),
                Err(r) => format!("refused: {r}"),
            };
            println!(
                "  {} control {:<10} against {:<10} {detail}",
                if accepted { "MISS" } else { "ok  " },
                film.id,
                other.id
            );
        }
    }
    let share = aligned as f64 / manifest.variants.len().max(1) as f64;
    println!();
    for (kind, (ok, n)) in &by_kind {
        println!("  {kind:<12} {ok}/{n}");
    }
    println!(
        "  aligned {aligned}/{} = {:.1}%   (target > {:.0}%; a copy aligns when {:.0}% of its cues land within {WITHIN_MS} ms)   refused {refused}",
        manifest.variants.len(),
        100.0 * share,
        100.0 * TARGET,
        100.0 * CUES_THAT_MUST_LAND
    );
    let pct = |(a, n): (usize, usize)| 100.0 * a as f64 / n.max(1) as f64;
    println!(
        "  tuned on {}/{} = {:.1}%   held out {}/{} = {:.1}%",
        tuned.0,
        tuned.1,
        pct(tuned),
        held.0,
        held.1,
        pct(held)
    );
    println!(
        "  false accepts {false_accepts}/{} (subtitles against the wrong film; target 0)",
        manifest.films.len()
    );
    Ok(share > TARGET && false_accepts == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trace_survives_the_round_trip() {
        let t = [false, false, true, true, true, false, true];
        assert_eq!(parse_trace(&write_trace(&t)), t);
        let starts_speaking = [true, true, false];
        assert_eq!(parse_trace(&write_trace(&starts_speaking)), starts_speaking);
    }
}
