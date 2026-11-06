//! Which copy to play — SPEC.md Phase 9: "The user picks a language and a quality. The
//! app does everything else, invisibly and well."
//!
//! [`score`] is a pure function: a candidate and a [`Context`] in, a [`Score`] out, with
//! every term that made it up. [`rank`] orders a list by those scores. Nothing here
//! reads a clock, a disk or the network, so every decision can be reproduced in a test
//! and shown, term by term, in the expert panel.
//!
//! The order is lexicographic, not one sum, because some things are not trade-offs:
//!
//! 1. A copy that is **sunk** — a fake, a mislabel, not a video — comes last, whatever
//!    else it has going for it ([`plausibility`]).
//! 2. A copy known **not to meet the viewer's explicit choice** (they picked Hindi and it
//!    carries Japanese) comes after every copy that does or might.
//! 3. A **file on this machine** comes first among the rest. SPEC.md: "a local file
//!    always wins."
//! 4. Then the weighted sum of the terms, highest first.
//! 5. Then the key, so equal scores still order the same way every time.
//!
//! Every weight and its reason is in `docs/specs/source-scoring.md`.

pub mod plausibility;

use std::cmp::Ordering;

use serde::Serialize;
use source_protocol::{Health, Resolution, SourceCandidate, Transport, VideoCodec};

/// What this machine plays well. Built from `tiers.rs`, never by probing here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    /// The highest resolution it plays smoothly by default (§8: 1080p on Tier 0).
    pub max_resolution: Resolution,
    /// Hardware decode for H.265 and AV1. Without it they decode in software, which a
    /// modest CPU cannot keep up with at high resolutions.
    pub hevc: bool,
    pub av1: bool,
    /// An HDR display. Without one, HDR is tone-mapped: it plays, a little flatter.
    pub hdr_display: bool,
}

impl Default for Machine {
    /// The safe assumption: a Tier 0 machine.
    fn default() -> Self {
        Self {
            max_resolution: Resolution::FullHd,
            hevc: false,
            av1: false,
            hdr_display: false,
        }
    }
}

/// Everything the score depends on besides the candidate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// The catalogue's runtime for the film or episode.
    pub runtime_minutes: Option<u32>,
    pub machine: Machine,
    /// The audio language the viewer chose, BCP 47. `None`: no preference.
    pub audio_language: Option<String>,
    /// The subtitle language they want, if any.
    pub subtitle_language: Option<String>,
    /// The quality the viewer chose. `None`: the best this machine plays.
    pub wanted: Option<Resolution>,
    /// Measured download speed, bytes per second, if there is a measurement.
    pub throughput: Option<u64>,
}

/// One part of a score. `value` is in −1..=1; its contribution is `value × weight`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Term {
    pub name: &'static str,
    pub value: f64,
    pub weight: f64,
    /// In words, for the expert panel and the "why this source" line.
    pub why: String,
}

impl Term {
    fn new(name: &'static str, value: f64, weight: f64, why: impl Into<String>) -> Self {
        Self {
            name,
            value: value.clamp(-1.0, 1.0),
            weight,
            why: why.into(),
        }
    }

    pub fn contribution(&self) -> f64 {
        self.value * self.weight
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Score {
    /// The sum of every term's contribution.
    pub total: f64,
    pub terms: Vec<Term>,
    /// Why this copy is last whatever its total: a fake, a mislabel, not a video.
    pub sunk: Option<String>,
    /// Why this copy does not meet what the viewer explicitly chose.
    pub unmet: Option<String>,
    /// On this machine.
    pub local: bool,
}

/// The weights. Each is justified in `docs/specs/source-scoring.md`; a change here
/// without a change there fails review.
pub mod weight {
    pub const LANGUAGE: f64 = 4.0;
    pub const QUALITY: f64 = 3.0;
    pub const PLAUSIBILITY: f64 = 3.0;
    pub const HEALTH: f64 = 2.0;
    pub const DURATION: f64 = 2.0;
    pub const COMPATIBILITY: f64 = 2.0;
    pub const THROUGHPUT: f64 = 2.0;
    pub const ENCODING: f64 = 1.0;
    pub const SUBTITLES: f64 = 0.5;
}

/// Score one candidate. Pure: the same arguments always give the same score.
pub fn score(c: &SourceCandidate, ctx: &Context) -> Score {
    let mut sunk = None;
    let mut unmet = None;
    let mut terms = Vec::with_capacity(9);

    let (t, why) = language(c, ctx);
    unmet = unmet.or(why);
    terms.push(t);
    let (t, why) = quality(c, ctx);
    unmet = unmet.or(why);
    terms.push(t);
    for check in [
        plausibility::bitrate,
        plausibility::duration,
        plausibility::encoding,
    ] {
        let (t, why) = check(c, ctx);
        sunk = sunk.or(why);
        terms.push(t);
    }
    terms.push(health(c));
    terms.push(compatibility(c, ctx));
    terms.push(throughput(c, ctx));
    terms.push(subtitles(c, ctx));

    Score {
        total: terms.iter().map(Term::contribution).sum(),
        terms,
        sunk,
        unmet,
        local: c.transport == Transport::Local,
    }
}

/// The order described in the module note, best first.
pub fn compare(a: (&SourceCandidate, &Score), b: (&SourceCandidate, &Score)) -> Ordering {
    let (ca, sa) = a;
    let (cb, sb) = b;
    sa.sunk
        .is_some()
        .cmp(&sb.sunk.is_some())
        .then(sa.unmet.is_some().cmp(&sb.unmet.is_some()))
        .then(sb.local.cmp(&sa.local))
        .then(sb.total.total_cmp(&sa.total))
        .then(ca.key.cmp(&cb.key))
}

/// Score every candidate and order them, best first.
pub fn rank(candidates: Vec<SourceCandidate>, ctx: &Context) -> Vec<(SourceCandidate, Score)> {
    let mut scored: Vec<_> = candidates
        .into_iter()
        .map(|c| {
            let s = score(&c, ctx);
            (c, s)
        })
        .collect();
    scored.sort_by(|a, b| compare((&a.0, &a.1), (&b.0, &b.1)));
    scored
}

/// `hi` matches `hi-IN`; case does not matter. BCP 47's primary subtag is the language.
pub fn same_language(a: &str, b: &str) -> bool {
    let primary = |s: &str| {
        s.split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    !a.is_empty() && primary(a) == primary(b)
}

pub fn resolution_label(r: Resolution) -> &'static str {
    match r {
        Resolution::Sd => "SD",
        Resolution::Hd => "720p",
        Resolution::FullHd => "1080p",
        Resolution::Uhd => "4K",
        Resolution::Unknown => "unknown quality",
    }
}

fn language(c: &SourceCandidate, ctx: &Context) -> (Term, Option<String>) {
    let w = weight::LANGUAGE;
    let Some(want) = &ctx.audio_language else {
        return (Term::new("language", 0.0, w, "no language chosen"), None);
    };
    if c.languages.iter().any(|l| same_language(l, want)) {
        return (Term::new("language", 1.0, w, format!("{want} audio")), None);
    }
    if c.languages.is_empty() {
        return (
            Term::new("language", 0.0, w, "audio languages not stated"),
            None,
        );
    }
    let why = format!("carries {}, not {want}", c.languages.join(", "));
    (Term::new("language", -1.0, w, why.clone()), Some(why))
}

fn quality(c: &SourceCandidate, ctx: &Context) -> (Term, Option<String>) {
    let w = weight::QUALITY;
    let have = c.quality.resolution;
    let target = ctx.wanted.unwrap_or(ctx.machine.max_resolution);
    let label = resolution_label(have);
    if have == Resolution::Unknown {
        let unmet = ctx
            .wanted
            .map(|w| format!("quality not stated, {} chosen", resolution_label(w)));
        return (Term::new("quality", 0.0, w, "quality not stated"), unmet);
    }
    let unmet = ctx
        .wanted
        .filter(|&want| want != have)
        .map(|want| format!("{label}, not {}", resolution_label(want)));
    let (have, target_rank, max) = (
        f64::from(have.rank()),
        f64::from(target.rank()),
        f64::from(ctx.machine.max_resolution.rank()),
    );
    let term = if ctx.wanted.is_none() && have > max {
        Term::new(
            "quality",
            -1.0,
            w,
            format!("{label} is beyond what this machine plays smoothly"),
        )
    } else if have == target_rank {
        Term::new("quality", 1.0, w, label)
    } else if have < target_rank {
        // Each step down costs half the term: 1080p for a 4K choice is still good.
        Term::new(
            "quality",
            1.0 - 0.5 * (target_rank - have),
            w,
            format!("{label}, below the {} wanted", resolution_label(target)),
        )
    } else {
        Term::new(
            "quality",
            0.5,
            w,
            format!("{label}, above the {} wanted", resolution_label(target)),
        )
    };
    (term, unmet)
}

fn health(c: &SourceCandidate) -> Term {
    let w = weight::HEALTH;
    match c.health {
        Health::Direct if c.transport == Transport::Local => {
            Term::new("health", 1.0, w, "on this machine")
        }
        Health::Direct => Term::new("health", 1.0, w, "served directly"),
        // −1 with no seeders, 0 at five, approaching 1 beyond about fifty.
        Health::Swarm { seeders, .. } => {
            let s = f64::from(seeders);
            Term::new(
                "health",
                (s - 5.0) / (s + 5.0),
                w,
                format!("{seeders} seeders"),
            )
        }
        Health::Unknown => Term::new("health", 0.0, w, "availability unknown"),
    }
}

fn compatibility(c: &SourceCandidate, ctx: &Context) -> Term {
    let w = weight::COMPATIBILITY;
    let m = ctx.machine;
    let (mut value, mut why) = match c.codec {
        VideoCodec::H264 | VideoCodec::Mpeg4 => (1.0, "plays everywhere".to_string()),
        VideoCodec::H265 if m.hevc => (1.0, "H.265, decoded in hardware".into()),
        VideoCodec::Av1 if m.av1 => (1.0, "AV1, decoded in hardware".into()),
        VideoCodec::H265 => (-0.5, "H.265 without hardware decode".into()),
        VideoCodec::Av1 => (-0.5, "AV1 without hardware decode".into()),
        VideoCodec::Vp9 => (0.5, "VP9".into()),
        VideoCodec::Unknown => (0.0, "codec not stated".into()),
    };
    if c.quality.hdr && !m.hdr_display {
        value -= 0.5;
        why.push_str("; HDR, tone-mapped on this display");
    }
    Term::new("compatibility", value, w, why)
}

fn throughput(c: &SourceCandidate, ctx: &Context) -> Term {
    let w = weight::THROUGHPUT;
    if c.transport == Transport::Local {
        return Term::new("throughput", 1.0, w, "no download needed");
    }
    let (Some(speed), Some(rate)) = (ctx.throughput, plausibility::bytes_per_second(c, ctx)) else {
        return Term::new("throughput", 0.0, w, "speed not measured");
    };
    let headroom = speed as f64 / rate;
    let mbit = |b: f64| b * 8.0 / 1e6;
    let why = format!(
        "needs {:.1} Mbit/s, measured {:.1}",
        mbit(rate),
        mbit(speed as f64)
    );
    let value = if headroom >= 1.5 {
        1.0
    } else if headroom >= 1.0 {
        0.0
    } else {
        -1.0
    };
    Term::new("throughput", value, w, why)
}

fn subtitles(c: &SourceCandidate, ctx: &Context) -> Term {
    let w = weight::SUBTITLES;
    match &ctx.subtitle_language {
        Some(want) if c.subtitles.iter().any(|l| same_language(l, want)) => {
            Term::new("subtitles", 1.0, w, format!("{want} subtitles included"))
        }
        Some(want) => Term::new("subtitles", 0.0, w, format!("no {want} subtitles included")),
        None => Term::new("subtitles", 0.0, w, "no subtitles wanted"),
    }
}
