//! The score explains itself, and the documentation explains the score (Phase 9 E5, E6).

mod common;

use common::*;
use sinephile_selection::{rank, score, weight, why, Context};
use source_protocol::{Resolution::*, VideoCodec};

fn film() -> Context {
    Context {
        runtime_minutes: Some(140),
        ..Context::default()
    }
}

#[test]
fn why_names_what_most_recommends_a_copy() {
    let c = swarm("x", 120).res(FullHd).codec(VideoCodec::H264).done();
    // Compatibility (+2) outranks health (2 × 0.92); the zero terms say nothing.
    assert_eq!(
        why(&c, &score(&c, &film())),
        "1080p · plays everywhere · 120 seeders"
    );
    let checked = http("y").res(Hd).size(1_000_000_000).minutes(139).done();
    assert_eq!(
        why(&checked, &score(&checked, &film())),
        "720p · served directly · size and length check out"
    );
    let ctx = Context {
        audio_language: Some("hi".into()),
        ..film()
    };
    let hindi = http("z").res(FullHd).audio(&["hi"]).done();
    let line = why(&hindi, &score(&hindi, &ctx));
    assert!(line.starts_with("1080p · hi audio · "), "{line}");
}

#[test]
fn a_local_copy_says_so_first() {
    let c = local("mine").res(Hd).done();
    let line = why(&c, &score(&c, &film()));
    assert!(line.starts_with("on this machine · 720p"), "{line}");
}

#[test]
fn an_unmet_choice_leads_the_line() {
    let ctx = Context {
        audio_language: Some("hi".into()),
        ..film()
    };
    let c = http("x").res(FullHd).audio(&["en"]).done();
    let line = why(&c, &score(&c, &ctx));
    assert!(line.starts_with("carries en, not hi · 1080p"), "{line}");
}

#[test]
fn a_sunk_copy_that_plays_anyway_says_why_it_is_last() {
    let ranked = rank(vec![http("x").res(FullHd).minutes(20).done()], &film());
    assert_eq!(
        why(&ranked[0].0, &ranked[0].1),
        "the best left, though plays 20 minutes; this runs 140 · 1080p · served directly"
    );
}

/// `docs/specs/source-scoring.md` §2's table is the documentation of every weight. It
/// must agree with the code, or one of them is lying (E6).
#[test]
fn the_specs_weights_are_the_codes_weights() {
    let spec = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/specs/source-scoring.md"
    ))
    .unwrap();
    let code = [
        ("language", weight::LANGUAGE),
        ("quality", weight::QUALITY),
        ("plausibility", weight::PLAUSIBILITY),
        ("duration", weight::DURATION),
        ("health", weight::HEALTH),
        ("compatibility", weight::COMPATIBILITY),
        ("throughput", weight::THROUGHPUT),
        ("encoding", weight::ENCODING),
        ("subtitles", weight::SUBTITLES),
    ];
    let documented: Vec<(String, f64)> = spec
        .lines()
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            let name = cells.get(1)?.strip_prefix('`')?.strip_suffix('`')?;
            Some((name.to_string(), cells.get(2)?.parse().ok()?))
        })
        .collect();
    assert_eq!(documented.len(), code.len(), "{documented:?}");
    for (name, w) in code {
        let doc = documented.iter().find(|(n, _)| n == name);
        assert_eq!(doc.map(|d| d.1), Some(w), "{name}");
    }
    // And every term the score produces is one of them.
    let s = score(&http("x").done(), &film());
    for t in &s.terms {
        assert!(
            code.iter().any(|(n, w)| *n == t.name && *w == t.weight),
            "{}",
            t.name
        );
    }
}

#[test]
fn the_quality_term_says_whose_target_it_is() {
    let c = http("x").res(Hd).done();
    let t = |ctx: &Context| {
        score(&c, ctx)
            .terms
            .into_iter()
            .find(|t| t.name == "quality")
            .unwrap()
            .why
    };
    assert_eq!(t(&film()), "720p, below the 1080p this machine plays");
    let chosen = Context {
        wanted: Some(Uhd),
        ..film()
    };
    assert_eq!(t(&chosen), "720p, below the 4K chosen");
}
