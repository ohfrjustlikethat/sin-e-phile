//! Invariants that hold for every candidate list, checked over thousands of random ones
//! (SPEC.md Phase 9 E1). A small seeded generator rather than a property-testing crate:
//! the cases are reproducible from the seed, and nothing new is added to the build.

mod common;

use common::*;
use sinephile_selection::{rank, score, Context, Machine};
use source_protocol::{Resolution, SourceCandidate, VideoCodec};

/// xorshift64*: enough randomness for test cases, and the same sequence every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<T: Clone>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize].clone()
    }
    fn maybe<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> Option<T> {
        (self.below(2) == 0).then(|| f(self))
    }
}

const RESOLUTIONS: [Resolution; 5] = [
    Resolution::Sd,
    Resolution::Hd,
    Resolution::FullHd,
    Resolution::Uhd,
    Resolution::Unknown,
];
const CODECS: [VideoCodec; 6] = [
    VideoCodec::H264,
    VideoCodec::H265,
    VideoCodec::Av1,
    VideoCodec::Vp9,
    VideoCodec::Mpeg4,
    VideoCodec::Unknown,
];
const LANGS: [&str; 4] = ["en", "hi", "ja", "fr"];
const TITLES: [&str; 5] = [
    "Film",
    "Film 1080p BluRay x264",
    "Film HDCAM",
    "Film 720p",
    "Film.exe",
];

fn candidate(rng: &mut Rng, i: usize) -> SourceCandidate {
    let key = format!("c{i}");
    let mut c = match rng.below(3) {
        0 => local(&key),
        1 => http(&key),
        _ => swarm(&key, rng.below(200) as u32),
    }
    .res(rng.pick(&RESOLUTIONS))
    .codec(rng.pick(&CODECS))
    .titled(rng.pick(&TITLES));
    if rng.below(2) == 0 {
        c = c.size(rng.below(30 * GB) + 1);
    }
    if rng.below(2) == 0 {
        c = c.minutes(rng.below(200) as u32 + 1);
    }
    if rng.below(2) == 0 {
        c = c.audio(&[rng.pick(&LANGS)]);
    }
    if rng.below(4) == 0 {
        c = c.hdr();
    }
    c.done()
}

fn context(rng: &mut Rng) -> Context {
    Context {
        runtime_minutes: rng.maybe(|r| r.below(180) as u32 + 1),
        machine: Machine {
            max_resolution: rng.pick(&RESOLUTIONS[..4]),
            hevc: rng.below(2) == 0,
            av1: rng.below(2) == 0,
            hdr_display: rng.below(2) == 0,
        },
        audio_language: rng.maybe(|r| r.pick(&LANGS).to_string()),
        subtitle_language: rng.maybe(|r| r.pick(&LANGS).to_string()),
        wanted: rng.maybe(|r| r.pick(&RESOLUTIONS[..4])),
        throughput: rng.maybe(|r| r.below(10_000_000) + 1),
    }
}

/// Run `check` over `cases` random (list, context) pairs.
fn for_all(cases: usize, mut check: impl FnMut(&mut Rng, Vec<SourceCandidate>, Context)) {
    let mut rng = Rng(0x5EED_CAFE);
    for _ in 0..cases {
        let n = rng.below(8) as usize + 1;
        let list = (0..n).map(|i| candidate(&mut rng, i)).collect();
        let ctx = context(&mut rng);
        check(&mut rng, list, ctx);
    }
}

#[test]
fn ranking_is_a_permutation() {
    for_all(2000, |_, list, ctx| {
        let mut before: Vec<_> = list.iter().map(|c| c.key.clone()).collect();
        let mut after: Vec<_> = rank(list, &ctx).into_iter().map(|(c, _)| c.key).collect();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    });
}

#[test]
fn the_order_does_not_depend_on_the_order_given() {
    for_all(2000, |rng, list, ctx| {
        let mut shuffled = list.clone();
        for i in (1..shuffled.len()).rev() {
            shuffled.swap(i, rng.below(i as u64 + 1) as usize);
        }
        let a: Vec<_> = rank(list, &ctx).into_iter().map(|(c, _)| c.key).collect();
        let b: Vec<_> = rank(shuffled, &ctx)
            .into_iter()
            .map(|(c, _)| c.key)
            .collect();
        assert_eq!(a, b);
    });
}

#[test]
fn a_sunk_copy_is_never_ahead_of_an_honest_one() {
    for_all(2000, |_, list, ctx| {
        let ranked = rank(list, &ctx);
        let first_sunk = ranked.iter().position(|(_, s)| s.sunk.is_some());
        if let Some(i) = first_sunk {
            assert!(ranked[i..].iter().all(|(_, s)| s.sunk.is_some()));
        }
    });
}

#[test]
fn an_honest_local_file_that_meets_the_choice_is_first() {
    for_all(2000, |_, list, ctx| {
        let ranked = rank(list, &ctx);
        let good_local =
            |s: &sinephile_selection::Score| s.local && s.sunk.is_none() && s.unmet.is_none();
        if ranked.iter().any(|(_, s)| good_local(s)) {
            assert!(good_local(&ranked[0].1));
        }
    });
}

#[test]
fn the_total_is_the_sum_of_the_terms_and_every_value_is_bounded() {
    for_all(2000, |_, list, ctx| {
        for c in &list {
            let s = score(c, &ctx);
            let sum: f64 = s.terms.iter().map(|t| t.value * t.weight).sum();
            assert!((s.total - sum).abs() < 1e-9);
            assert!(s.terms.iter().all(|t| (-1.0..=1.0).contains(&t.value)));
        }
    });
}

#[test]
fn more_seeders_never_lower_a_score() {
    for_all(1000, |rng, list, ctx| {
        for c in &list {
            let source_protocol::Health::Swarm { seeders, peers } = c.health else {
                continue;
            };
            let mut more = c.clone();
            more.health = source_protocol::Health::Swarm {
                seeders: seeders + rng.below(500) as u32 + 1,
                peers,
            };
            assert!(score(&more, &ctx).total >= score(c, &ctx).total);
        }
    });
}

#[test]
fn carrying_the_chosen_language_never_hurts() {
    for_all(1000, |_, list, ctx| {
        let Some(want) = ctx.audio_language.clone() else {
            return;
        };
        for c in &list {
            let mut with = c.clone();
            with.languages.push(want.clone());
            let (a, b) = (score(&with, &ctx), score(c, &ctx));
            assert!(a.total >= b.total);
            assert!(a.unmet.is_none() || b.unmet.is_some());
        }
    });
}
